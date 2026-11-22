mod convert;
mod error;
mod smf;
mod write;

pub use convert::to_song;
pub use error::MidiError;
pub use smf::{MidiFile, RawNote, parse, read_file};
pub use write::{write_file, write_song};

#[cfg(test)]
mod tests {
    use super::*;
    fn build_smf(tracks: &[Vec<u8>]) -> Vec<u8> {
        let mut file = Vec::new();
        file.extend_from_slice(b"MThd");
        file.extend_from_slice(&6u32.to_be_bytes());
        file.extend_from_slice(&1u16.to_be_bytes());
        file.extend_from_slice(&(tracks.len() as u16).to_be_bytes());
        file.extend_from_slice(&96u16.to_be_bytes());
        for trk in tracks {
            file.extend_from_slice(b"MTrk");
            file.extend_from_slice(&(trk.len() as u32).to_be_bytes());
            file.extend_from_slice(trk);
        }
        file
    }
    fn simple_track() -> Vec<u8> {
        let mut trk = Vec::new();
        trk.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]);
        trk.extend_from_slice(&[0x00, 0x90, 60, 80]);
        trk.extend_from_slice(&[0x60, 0x80, 60, 0]);
        trk.extend_from_slice(&[0x00, 0x90, 64, 90]);
        trk.extend_from_slice(&[0x60, 0x80, 64, 0]);
        trk.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        trk
    }
    #[test]
    fn parses_two_notes_with_tempo() {
        let smf = build_smf(&[simple_track()]);
        let file = parse(&smf).unwrap();
        assert_eq!(file.ppq, 96);
        assert_eq!(file.notes.len(), 2);
        assert!((file.bpm() - 120.0).abs() < 0.1);
        let n0 = file.notes.iter().find(|n| n.midi == 60).unwrap();
        assert_eq!(n0.start_beats, 0.0);
        assert!((n0.len_beats - 1.0).abs() < 1e-9);
    }
    #[test]
    fn rejects_non_midi() {
        assert!(matches!(parse(b"RIFF...."), Err(MidiError::NotMidi)));
        assert!(matches!(parse(b""), Err(MidiError::NotMidi)));
    }
    #[test]
    fn rejects_smpte_division() {
        let mut smf = build_smf(&[simple_track()]);
        smf[12] = 0xE7;
        smf[13] = 0x28;
        assert!(matches!(parse(&smf), Err(MidiError::Smpte)));
    }
    #[test]
    fn handles_running_status() {
        let mut trk = Vec::new();
        trk.extend_from_slice(&[0x00, 0x90, 60, 80]);
        trk.extend_from_slice(&[0x60, 60, 0]);
        trk.extend_from_slice(&[0x00, 0x90, 64, 80]);
        trk.extend_from_slice(&[0x60, 64, 0]);
        trk.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        let smf = build_smf(&[trk]);
        let file = parse(&smf).unwrap();
        assert_eq!(file.notes.len(), 2, "running status 应正确解析");
    }
    #[test]
    fn closes_dangling_note_at_track_end() {
        let mut trk = Vec::new();
        trk.extend_from_slice(&[0x00, 0x90, 60, 80]);
        trk.extend_from_slice(&[0x60, 0xFF, 0x2F, 0x00]);
        let smf = build_smf(&[trk]);
        let file = parse(&smf).unwrap();
        assert_eq!(file.notes.len(), 1, "未闭合的音符应在轨道结束时收尾");
    }
    #[test]
    fn skips_sysex_events() {
        let mut trk = Vec::new();
        trk.extend_from_slice(&[0x00, 0xF0, 0x03, 0x01, 0x02, 0x03]);
        trk.extend_from_slice(&[0x00, 0x90, 60, 80]);
        trk.extend_from_slice(&[0x60, 0x80, 60, 0]);
        trk.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        let smf = build_smf(&[trk]);
        let file = parse(&smf).unwrap();
        assert_eq!(file.notes.len(), 1);
    }
    #[test]
    fn retrigger_replaces_unclosed_note() {
        let mut trk = Vec::new();
        trk.extend_from_slice(&[0x00, 0x90, 60, 80]);
        trk.extend_from_slice(&[0x30, 0x90, 60, 100]);
        trk.extend_from_slice(&[0x30, 0x80, 60, 0]);
        trk.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        let smf = build_smf(&[trk]);
        let file = parse(&smf).unwrap();
        assert_eq!(file.notes.len(), 1, "重复触发应覆盖而非叠加");
    }
    #[test]
    fn multi_track_file_merges_notes() {
        let a = simple_track();
        let b = simple_track();
        let smf = build_smf(&[a, b]);
        let file = parse(&smf).unwrap();
        assert_eq!(file.notes.len(), 4, "两条轨道应合并");
    }
    #[test]
    fn converts_to_song_with_bars() {
        let smf = build_smf(&[simple_track()]);
        let file = parse(&smf).unwrap();
        let song = to_song(&file);
        assert!(song.bars >= 1);
        assert!(song.total_notes() >= 1);
        assert!(song.bpm > 0.0);
    }
}
