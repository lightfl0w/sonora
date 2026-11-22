use serde::{Deserialize, Serialize};

use crate::voice::VoiceId;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub bar: u32,
    pub beat: f32,
    pub midi: u8,
    pub len: f32,
    pub vel: f32,
}

impl Note {
    pub fn start_beats(&self) -> f32 {
        self.bar.saturating_sub(1) as f32 * 4.0 + (self.beat - 1.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub voice: VoiceId,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub drums: bool,
    pub level: f32,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub notes: Vec<Note>,
}

impl Track {
    pub fn new(voice: VoiceId, drums: bool, level: f32) -> Self {
        Self {
            voice,
            name: String::new(),
            drums,
            level,
            muted: false,
            solo: false,
            notes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub bpm: f32,
    pub bars: u32,
    pub tracks: Vec<Track>,
    pub master: f32,
    pub reverb_size: f32,
    pub reverb_damp: f32,
    pub reverb_wet: f32,
}

impl Song {
    pub fn empty() -> Self {
        Self {
            bpm: 120.0,
            bars: 16,
            tracks: vec![Track::new(0, false, 0.9)],
            master: 0.8,
            reverb_size: 0.82,
            reverb_damp: 0.55,
            reverb_wet: 0.5,
        }
    }
    pub fn example() -> Self {
        use crate::voice::builtin;
        let mut piano = Track::new(builtin::PIANO, false, 0.9);
        piano.name = "钢琴".into();
        let mut strings = Track::new(builtin::STRINGS, false, 0.7);
        strings.name = "弦乐".into();
        let mut harp = Track::new(builtin::HARP, false, 0.65);
        harp.name = "竖琴".into();
        let mut ambient = Track::new(builtin::AMBIENT, false, 0.55);
        ambient.name = "环境音".into();
        let chords: [(u8, [u8; 4]); 4] = [
            (45, [57, 61, 64, 68]),
            (42, [54, 57, 61, 64]),
            (38, [50, 54, 57, 61]),
            (40, [52, 56, 59, 64]),
        ];
        let melody: [(u32, f32, u8, f32); 24] = [
            (9, 1.0, 81, 2.0),
            (9, 3.0, 80, 1.5),
            (10, 1.0, 78, 2.0),
            (10, 3.0, 76, 1.5),
            (11, 1.0, 74, 2.0),
            (11, 3.0, 73, 1.5),
            (12, 1.0, 71, 3.0),
            (13, 1.0, 81, 1.5),
            (13, 2.5, 80, 1.0),
            (13, 4.0, 78, 2.0),
            (14, 1.0, 76, 2.0),
            (14, 3.0, 73, 2.0),
            (15, 1.0, 74, 3.0),
            (15, 4.0, 71, 2.0),
            (16, 1.0, 69, 4.0),
            (17, 1.0, 81, 2.0),
            (17, 3.0, 85, 2.0),
            (18, 1.0, 84, 2.0),
            (18, 3.0, 81, 2.0),
            (19, 1.0, 78, 3.0),
            (19, 4.0, 76, 1.5),
            (20, 1.0, 74, 4.0),
            (21, 1.0, 81, 2.0),
            (25, 1.0, 85, 2.0),
        ];
        for (bar, beat, midi, len) in melody {
            piano.notes.push(Note {
                bar,
                beat,
                midi,
                len,
                vel: 0.55,
            });
        }
        for bar in 1..=32u32 {
            let (root, pad) = chords[((bar - 1) % 4) as usize];
            for n in pad {
                strings.notes.push(Note {
                    bar,
                    beat: 1.0,
                    midi: n,
                    len: 3.8,
                    vel: 0.8,
                });
            }
            push_avoiding(
                &mut piano.notes,
                Note {
                    bar,
                    beat: 1.0,
                    midi: root,
                    len: 3.6,
                    vel: 0.75,
                },
            );
            if bar % 2 == 1 {
                push_avoiding(
                    &mut piano.notes,
                    Note {
                        bar,
                        beat: 2.0,
                        midi: pad[3] + 12,
                        len: 1.8,
                        vel: 0.6,
                    },
                );
                push_avoiding(
                    &mut piano.notes,
                    Note {
                        bar,
                        beat: 4.0,
                        midi: pad[1] + 12,
                        len: 1.5,
                        vel: 0.5,
                    },
                );
            } else {
                push_avoiding(
                    &mut piano.notes,
                    Note {
                        bar,
                        beat: 3.0,
                        midi: pad[2] + 12,
                        len: 2.0,
                        vel: 0.55,
                    },
                );
            }
            if bar >= 9 && bar % 2 == 0 {
                harp.notes.push(Note {
                    bar,
                    beat: 4.0,
                    midi: pad[0] + 24,
                    len: 2.0,
                    vel: 0.5,
                });
            }
            if (bar - 1) % 4 == 0 {
                ambient.notes.push(Note {
                    bar,
                    beat: 1.0,
                    midi: pad[0] + 12,
                    len: 15.0,
                    vel: 0.6,
                });
            }
        }
        Self {
            bpm: 72.0,
            bars: 32,
            tracks: vec![piano, strings, harp, ambient],
            master: 0.8,
            reverb_size: 0.82,
            reverb_damp: 0.55,
            reverb_wet: 0.5,
        }
    }
    pub fn beat_secs(&self) -> f32 {
        60.0 / self.bpm.max(20.0)
    }
    pub fn bar_secs(&self) -> f32 {
        self.beat_secs() * 4.0
    }
    pub fn duration_secs(&self) -> f32 {
        self.bars as f32 * self.bar_secs()
    }
    pub fn total_notes(&self) -> usize {
        self.tracks.iter().map(|t| t.notes.len()).sum()
    }
    pub fn swell(&self, bar: u32) -> f32 {
        let phase = bar.saturating_sub(1) as f32 / (self.bars.max(2) - 1) as f32;
        (phase * core::f32::consts::PI).sin()
    }
    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.solo)
    }
    pub fn is_audible(&self, track: &Track) -> bool {
        if self.any_solo() {
            track.solo
        } else {
            !track.muted
        }
    }
    pub fn sanitize(&mut self) {
        self.bpm = if self.bpm.is_finite() {
            self.bpm.clamp(20.0, 300.0)
        } else {
            120.0
        };
        self.bars = self.bars.clamp(1, 512);
        self.master = sane(self.master, 0.8);
        self.reverb_size = sane(self.reverb_size, 0.82);
        self.reverb_damp = sane(self.reverb_damp, 0.55);
        self.reverb_wet = sane(self.reverb_wet, 0.5);
        self.tracks.retain(|t| t.notes.len() <= MAX_NOTES_PER_TRACK);
        if self.tracks.is_empty() {
            self.tracks.push(Track::new(0, false, 0.9));
        }
        for track in &mut self.tracks {
            track.level = sane(track.level, 0.9);
            for note in &mut track.notes {
                note.bar = note.bar.clamp(1, self.bars);
                note.beat = if note.beat.is_finite() {
                    note.beat.clamp(1.0, 4.0)
                } else {
                    1.0
                };
                note.len = if note.len.is_finite() {
                    note.len.clamp(0.0625, 16.0)
                } else {
                    1.0
                };
                note.vel = sane(note.vel, 0.8).clamp(0.05, 1.0);
                note.midi = note.midi.min(127);
            }
        }
    }
}

pub const MAX_NOTES_PER_TRACK: usize = 100_000;

fn sane(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn push_avoiding(notes: &mut Vec<Note>, mut note: Note) {
    let mut start = note.start_beats();
    for other in notes.iter().filter(|n| n.midi == note.midi) {
        let os = other.start_beats();
        let oe = os + other.len;
        if os <= start + 1e-4 && oe > start + 1e-4 {
            start = oe;
            note.len -= oe - note.start_beats();
        }
    }
    let limit = start + note.len;
    for other in notes.iter().filter(|n| n.midi == note.midi) {
        let os = other.start_beats();
        if os > start + 1e-4 && os < limit {
            note.len = os - start;
        }
    }
    if note.len <= 1e-4 {
        return;
    }
    let bar_index = (start / 4.0).floor();
    note.bar = (bar_index as u32).saturating_add(1);
    note.beat = start - bar_index * 4.0 + 1.0;
    notes.push(note);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(bar: u32, beat: f32) -> Note {
        Note {
            bar,
            beat,
            midi: 60,
            len: 1.0,
            vel: 0.8,
        }
    }
    #[test]
    fn empty_song_has_one_piano_track() {
        let s = Song::empty();
        assert_eq!(s.tracks.len(), 1);
        assert_eq!(s.tracks[0].voice, 0);
        assert!(!s.tracks[0].drums);
        assert_eq!(s.total_notes(), 0);
    }
    #[test]
    fn start_beats_is_zero_based() {
        assert_eq!(note(1, 1.0).start_beats(), 0.0);
        assert_eq!(note(2, 3.5).start_beats(), 6.5);
    }
    #[test]
    fn timing_follows_bpm() {
        let mut s = Song::empty();
        assert!((s.beat_secs() - 0.5).abs() < 1e-6);
        assert!((s.bar_secs() - 2.0).abs() < 1e-6);
        s.bpm = 60.0;
        assert!((s.beat_secs() - 1.0).abs() < 1e-6);
    }
    #[test]
    fn swell_peaks_midway() {
        let s = Song::empty();
        assert!(s.swell(1) < 0.01);
        assert!((s.swell(s.bars / 2) - 1.0).abs() < 0.1);
        assert!(s.swell(s.bars) < 0.01);
    }
    #[test]
    fn total_notes_sums_all_tracks() {
        let mut s = Song::empty();
        s.tracks[0].notes.push(note(1, 1.0));
        s.tracks.push(Track::new(1, false, 0.7));
        s.tracks[1].notes.push(note(1, 1.0));
        s.tracks[1].notes.push(note(2, 1.0));
        assert_eq!(s.total_notes(), 3);
    }
    #[test]
    fn solo_overrides_mute() {
        let mut s = Song::empty();
        s.tracks.push(Track::new(1, false, 0.7));
        s.tracks[0].muted = true;
        s.tracks[1].solo = true;
        assert!(!s.is_audible(&s.tracks[0]));
        assert!(s.is_audible(&s.tracks[1]));
    }
    #[test]
    fn mute_applies_without_solo() {
        let mut s = Song::empty();
        s.tracks[0].muted = true;
        assert!(!s.is_audible(&s.tracks[0]));
    }
    #[test]
    fn example_song_has_no_same_pitch_overlaps() {
        let song = Song::example();
        let mut problems = Vec::new();
        for (ti, t) in song.tracks.iter().enumerate() {
            let mut notes: Vec<(usize, f32, f32, u8)> = t
                .notes
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    (
                        i,
                        (n.bar.saturating_sub(1)) as f32 * 4.0 + (n.beat - 1.0),
                        n.len,
                        n.midi,
                    )
                })
                .collect();
            notes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            for w in notes.windows(2) {
                let (ia, sa, la, ma) = w[0];
                let (ib, sb, lb, mb) = w[1];
                if ma == mb && sa + la > sb + 1e-4 {
                    problems.push(format!(
                        "track{ti} ({}) #{ia} midi{ma} {sa:.2}..{:.2} 与 #{ib} {sb:.2}..{:.2} 重叠",
                        t.name,
                        sa + la,
                        sb + lb
                    ));
                }
            }
        }
        assert!(
            problems.is_empty(),
            "示例工程存在 {} 处同音高重叠:\n{}",
            problems.len(),
            problems.join("\n")
        );
    }
}
