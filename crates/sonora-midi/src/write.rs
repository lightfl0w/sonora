use sonora_core::song::Song;

use crate::error::MidiError;

const PPQ: u16 = 96;
const DRUM_CHANNEL: u8 = 9;

pub fn write_song(song: &Song) -> Vec<u8> {
    let mut events: Vec<(u64, u8, Vec<u8>)> = Vec::new();
    let us_per_beat = (60_000_000.0 / song.bpm.clamp(20.0, 300.0) as f64).round() as u32;
    events.push((0, 0, tempo_event(us_per_beat)));
    events.push((0, 0, time_signature_event()));
    let mut melodic_channel = 0u8;
    for track in &song.tracks {
        let channel = if track.drums {
            DRUM_CHANNEL
        } else {
            let ch = melodic_channel.min(15);
            melodic_channel = (melodic_channel + 1) % 16;
            if ch == DRUM_CHANNEL {
                melodic_channel = 0;
                0
            } else {
                ch
            }
        };
        for note in &track.notes {
            let start = (note.start_beats().max(0.0) as f64 * PPQ as f64).round() as u64;
            let len = (note.len.max(0.03) as f64 * PPQ as f64).round() as u64;
            let end = start + len.max(1);
            let vel = (note.vel.clamp(0.05, 1.0) * 127.0)
                .round()
                .clamp(1.0, 127.0) as u8;
            events.push((start, 2, vec![0x90 | channel, note.midi.min(127), vel]));
            events.push((end, 1, vec![0x80 | channel, note.midi.min(127), 0]));
        }
    }
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut track: Vec<u8> = Vec::with_capacity(events.len() * 4 + 32);
    let mut last_tick = 0u64;
    for (tick, _, bytes) in events {
        write_varlen(&mut track, (tick - last_tick) as u32);
        last_tick = tick;
        track.extend_from_slice(&bytes);
    }
    track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
    let mut file = Vec::with_capacity(track.len() + 22);
    file.extend_from_slice(b"MThd");
    file.extend_from_slice(&6u32.to_be_bytes());
    file.extend_from_slice(&0u16.to_be_bytes());
    file.extend_from_slice(&1u16.to_be_bytes());
    file.extend_from_slice(&PPQ.to_be_bytes());
    file.extend_from_slice(b"MTrk");
    file.extend_from_slice(&(track.len() as u32).to_be_bytes());
    file.extend_from_slice(&track);
    file
}

pub fn write_file(path: &std::path::Path, song: &Song) -> Result<(), MidiError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(MidiError::Io)?;
        }
    }
    std::fs::write(path, write_song(song)).map_err(MidiError::Io)
}

fn tempo_event(us_per_beat: u32) -> Vec<u8> {
    let b = us_per_beat.to_be_bytes();
    vec![0xFF, 0x51, 0x03, b[1], b[2], b[3]]
}

fn time_signature_event() -> Vec<u8> {
    vec![0xFF, 0x58, 0x04, 4, 2, 24, 8]
}

fn write_varlen(out: &mut Vec<u8>, mut value: u32) {
    let mut buffer = [0u8; 4];
    let mut i = 3;
    buffer[i] = (value & 0x7F) as u8;
    value >>= 7;
    while value > 0 {
        i -= 1;
        buffer[i] = ((value & 0x7F) as u8) | 0x80;
        value >>= 7;
    }
    out.extend_from_slice(&buffer[i..]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use sonora_core::song::{Note, Track};
    use sonora_core::voice::builtin;
    #[test]
    fn written_file_parses_back() {
        let mut song = Song::empty();
        song.bpm = 96.0;
        song.bars = 4;
        song.tracks[0].notes.push(Note {
            bar: 1,
            beat: 1.0,
            midi: 60,
            len: 1.0,
            vel: 0.8,
        });
        song.tracks[0].notes.push(Note {
            bar: 2,
            beat: 3.0,
            midi: 64,
            len: 2.0,
            vel: 0.5,
        });
        let bytes = write_song(&song);
        let parsed = parse(&bytes).expect("写出来的 MIDI 应能解析");
        assert_eq!(parsed.notes.len(), 2);
        assert!((parsed.bpm() - 96.0).abs() < 0.5, "速度应保留");
        let mut pitches: Vec<u8> = parsed.notes.iter().map(|n| n.midi).collect();
        pitches.sort_unstable();
        assert_eq!(pitches, vec![60, 64]);
        let n = parsed.notes.iter().find(|n| n.midi == 60).expect("应有 C4");
        assert!(n.start_beats.abs() < 1e-6);
        assert!((n.len_beats - 1.0).abs() < 1e-6);
    }
    #[test]
    fn drums_go_to_channel_ten() {
        let mut song = Song::empty();
        let mut drums = Track::new(builtin::DRUMS, true, 0.8);
        drums.notes.push(Note {
            bar: 1,
            beat: 1.0,
            midi: 36,
            len: 0.5,
            vel: 0.9,
        });
        song.tracks.push(drums);
        let parsed = parse(&write_song(&song)).unwrap();
        let kick = parsed
            .notes
            .iter()
            .find(|n| n.midi == 36)
            .expect("应有底鼓");
        assert_eq!(kick.channel, 9, "鼓应写在第 10 通道");
        assert!(parsed.is_drum_channel(kick.channel));
    }
    #[test]
    fn empty_song_is_still_valid() {
        let parsed = parse(&write_song(&Song::empty())).expect("空工程也应写出合法文件");
        assert!(parsed.notes.is_empty());
    }
}
