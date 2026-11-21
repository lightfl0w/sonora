use sonora_core::song::{Note, Song, Track};
use sonora_core::voice::builtin;

use crate::smf::MidiFile;

const MIN_LEN_BEATS: f32 = 0.0625;
const MAX_LEN_BEATS: f32 = 16.0;

pub fn to_song(file: &MidiFile) -> Song {
    let mut song = Song::empty();
    song.bpm = file.bpm().clamp(20.0, 300.0);

    if file.notes.is_empty() {
        return song;
    }

    let max_end = file
        .notes
        .iter()
        .map(|n| n.start_beats + n.len_beats)
        .fold(0.0f64, f64::max);
    song.bars = ((max_end / 4.0).ceil() as u32).max(1).min(512);

    let melodic: Vec<&crate::smf::RawNote> = file
        .notes
        .iter()
        .filter(|n| !file.is_drum_channel(n.channel))
        .collect();

    let (min_pitch, max_pitch) = if melodic.is_empty() {
        (48u8, 72u8)
    } else {
        let lo = melodic.iter().map(|n| n.midi).min().unwrap_or(48);
        let hi = melodic.iter().map(|n| n.midi).max().unwrap_or(72);
        (lo, hi)
    };
    let span = max_pitch.saturating_sub(min_pitch).max(1) as f32;

    let has_drums = file.notes.iter().any(|n| file.is_drum_channel(n.channel));

    song.tracks = vec![
        Track::new(builtin::STRINGS, false, 0.7),
        Track::new(builtin::PIANO, false, 0.9),
        Track::new(builtin::HARP, false, 0.65),
        Track::new(builtin::AMBIENT, false, 0.55),
    ];
    if has_drums {
        song.tracks.push(Track::new(builtin::DRUMS, true, 0.8));
    }

    for raw in &file.notes {
        let drum = file.is_drum_channel(raw.channel);
        let track_idx = if drum {
            song.tracks.len() - 1
        } else {
            let rel = raw.midi.saturating_sub(min_pitch) as f32 / span;
            if rel < 0.25 {
                0
            } else if rel < 0.72 {
                1
            } else if rel < 0.92 {
                2
            } else {
                3
            }
        };

        let bar_f = raw.start_beats / 4.0;
        let bar = (bar_f.floor() as u32 + 1).min(song.bars);
        let beat = (bar_f.fract() * 4.0) as f32 + 1.0;

        song.tracks[track_idx].notes.push(Note {
            bar,
            beat: beat.clamp(1.0, 4.0),
            midi: raw.midi,
            len: (raw.len_beats as f32).clamp(MIN_LEN_BEATS, MAX_LEN_BEATS),
            vel: raw.vel.clamp(0.05, 1.0),
        });
    }

    song
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smf::RawNote;

    fn raw(start: f64, len: f64, midi: u8, channel: u8) -> RawNote {
        RawNote {
            start_beats: start,
            len_beats: len,
            midi,
            vel: 0.8,
            channel,
        }
    }

    #[test]
    fn empty_file_gives_empty_song() {
        let file = MidiFile::default();
        let song = to_song(&file);
        assert_eq!(song.total_notes(), 0);
    }

    #[test]
    fn bpm_comes_from_tempo() {
        let mut file = MidiFile::default();
        file.tempo_us_per_qn = 500_000;
        assert!((file.bpm() - 120.0).abs() < 0.1);

        file.tempo_us_per_qn = 1_000_000;
        assert!((file.bpm() - 60.0).abs() < 0.1);
    }

    #[test]
    fn bars_extend_to_last_note() {
        let mut file = MidiFile::default();
        file.notes = vec![raw(0.0, 4.0, 60, 0), raw(20.0, 4.0, 64, 0)];

        let song = to_song(&file);
        assert!(song.bars >= 6, "应扩展到覆盖最后一个音符，实际 {}", song.bars);
    }

    #[test]
    fn pitches_are_split_across_tracks() {
        let mut file = MidiFile::default();
        file.notes = vec![
            raw(0.0, 1.0, 40, 0),
            raw(0.0, 1.0, 60, 0),
            raw(0.0, 1.0, 80, 0),
        ];

        let song = to_song(&file);
        assert_eq!(song.total_notes(), 3);
        assert_eq!(song.tracks.len(), 4, "无鼓通道时不应加鼓轨");

        let per_track: Vec<usize> = song.tracks.iter().map(|t| t.notes.len()).collect();
        assert!(per_track.iter().filter(|c| **c > 0).count() >= 2, "应分布到多条轨道");
    }

    #[test]
    fn drum_channel_gets_own_track() {
        let mut file = MidiFile::default();
        file.notes = vec![raw(0.0, 1.0, 60, 0), raw(0.0, 0.25, 36, 9)];

        let song = to_song(&file);
        assert_eq!(song.tracks.len(), 5);

        let drums = song.tracks.last().unwrap();
        assert!(drums.drums);
        assert_eq!(drums.voice, builtin::DRUMS);
        assert_eq!(drums.notes.len(), 1);
        assert_eq!(drums.notes[0].midi, 36);
    }

    #[test]
    fn note_lengths_are_clamped() {
        let mut file = MidiFile::default();
        file.notes = vec![raw(0.0, 0.0001, 60, 0), raw(4.0, 1000.0, 64, 0)];

        let song = to_song(&file);
        for track in &song.tracks {
            for note in &track.notes {
                assert!(note.len >= MIN_LEN_BEATS);
                assert!(note.len <= MAX_LEN_BEATS);
            }
        }
    }

    #[test]
    fn beats_map_to_bar_and_beat() {
        let mut file = MidiFile::default();
        file.notes = vec![raw(0.0, 1.0, 60, 0), raw(5.0, 1.0, 62, 0)];

        let song = to_song(&file);
        let all: Vec<&Note> = song.tracks.iter().flat_map(|t| t.notes.iter()).collect();

        let first = all.iter().find(|n| n.midi == 60).unwrap();
        assert_eq!(first.bar, 1);
        assert!((first.beat - 1.0).abs() < 0.01);

        let second = all.iter().find(|n| n.midi == 62).unwrap();
        assert_eq!(second.bar, 2);
        assert!((second.beat - 2.0).abs() < 0.01);
    }
}
