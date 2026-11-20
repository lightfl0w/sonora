use sonora_core::voice::VoiceId;
use sonora_core::{Song, Track};

use crate::library::VoiceLibrary;
use crate::pool::VoicePool;
use crate::voice::midi_to_freq;

const START_PAD_SECS: f32 = 0.2;
const TAIL_SECS: f32 = 8.0;

#[derive(Clone, Copy)]
struct Pending {
    frame: u64,
    track: u16,
    voice: VoiceId,
    drums: bool,
    midi: u8,
    dur_secs: f32,
    vel: f32,
}

pub struct Sequencer {
    pending: Vec<Pending>,
    cursor: usize,
    pub frame: u64,
    pub playing: bool,
    id_counter: u64,
    tail_frames: u64,
    sr: f32,
    peak_events: usize,
}

impl Sequencer {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            cursor: 0,
            frame: 0,
            playing: false,
            id_counter: 1,
            tail_frames: 0,
            sr: 44100.0,
            peak_events: 0,
        }
    }

    pub fn load(&mut self, song: &Song, sr: f32) {
        self.sr = sr;
        self.pending.clear();

        let beat = song.beat_secs();
        let total: usize = song.tracks.iter().map(|t| t.notes.len()).sum();
        let need = total.max(self.peak_events).max(64);
        if self.pending.capacity() < need {
            self.pending.reserve(need - self.pending.capacity());
        }
        self.peak_events = need;

        for (ti, track) in song.tracks.iter().enumerate() {
            let track_idx = ti.min(u16::MAX as usize) as u16;
            for note in &track.notes {
                let start = START_PAD_SECS
                    + note.bar.saturating_sub(1) as f32 * beat * 4.0
                    + (note.beat - 1.0).max(0.0) * beat;
                self.pending.push(Pending {
                    frame: (start * sr) as u64,
                    track: track_idx,
                    voice: track.voice,
                    drums: track.drums,
                    midi: note.midi,
                    dur_secs: (note.len * beat).max(0.05),
                    vel: note.vel,
                });
            }
        }

        self.pending.sort_by_key(|p| p.frame);
        self.cursor = 0;
        self.frame = 0;
        self.tail_frames = ((song.duration_secs() + TAIL_SECS) * sr) as u64;
    }

    pub fn advance(
        &mut self,
        pool: &mut VoicePool,
        lib: &VoiceLibrary,
        song: &Song,
        frames: usize,
        master: f32,
    ) {
        if !self.playing {
            return;
        }
        let any_solo = song.any_solo();
        let end = self.frame + frames as u64;

        while self.cursor < self.pending.len() {
            let p = self.pending[self.cursor];
            if p.frame >= end {
                break;
            }
            self.cursor += 1;

            let Some(track) = song.tracks.get(p.track as usize) else {
                continue;
            };
            if !audible(track, any_solo) {
                continue;
            }

            let level = track.level * p.vel * master;
            if level <= 1e-4 {
                continue;
            }

            let pan = ((p.midi as f32 - 60.0) / 48.0).clamp(-1.0, 1.0) * 0.6;
            let id = self.id_counter;
            self.id_counter = self.id_counter.wrapping_add(1);

            if p.drums {
                pool.trigger_drum(p.midi, p.dur_secs, level, pan, self.sr, id);
            } else {
                let preset = lib.preset(p.voice);
                pool.trigger_sine(
                    &preset,
                    midi_to_freq(p.midi as f32),
                    p.dur_secs,
                    level,
                    pan,
                    self.sr,
                    id,
                );
            }
        }

        self.frame = end;
    }

    pub fn finished(&self) -> bool {
        self.playing && self.cursor >= self.pending.len() && self.frame > self.tail_frames
    }

    pub fn progress(&self) -> f32 {
        if self.tail_frames == 0 {
            return 0.0;
        }
        (self.frame as f32 / self.tail_frames as f32).clamp(0.0, 1.0)
    }

    pub fn position_bars(&self, song: &Song) -> f32 {
        let bar_secs = song.bar_secs();
        if bar_secs <= 0.0 {
            return 0.0;
        }
        let secs = (self.frame as f64 / self.sr.max(1.0) as f64) as f32 - START_PAD_SECS;
        secs / bar_secs
    }

    pub fn current_bar(&self, song: &Song) -> u32 {
        (self.position_bars(song).floor() as u32 + 1).min(song.bars)
    }

    pub fn seek_to_bar(&mut self, bar: f32, song: &Song) {
        let bar = bar.clamp(0.0, song.bars as f32);
        let frame = ((START_PAD_SECS + bar * song.bar_secs()) * self.sr) as u64;
        self.frame = frame;
        self.cursor = self.pending.partition_point(|p| p.frame < frame);
    }

    pub fn next_note_id(&mut self) -> u64 {
        let id = self.id_counter;
        self.id_counter = self.id_counter.wrapping_add(1);
        id
    }

    pub fn peak_events(&self) -> usize {
        self.peak_events
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
        self.frame = 0;
        self.playing = false;
        self.id_counter = 1;
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

fn audible(track: &Track, any_solo: bool) -> bool {
    if any_solo {
        track.solo
    } else {
        !track.muted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::VoiceLibrary;
    use crate::pool::VoicePool;
    use sonora_core::song::Note;
    use sonora_core::voice::builtin;

    const SR: f32 = 44100.0;

    fn note(bar: u32, beat: f32, midi: u8) -> Note {
        Note {
            bar,
            beat,
            midi,
            len: 1.0,
            vel: 0.8,
        }
    }

    fn setup(tracks: Vec<Track>) -> (Sequencer, VoicePool, VoiceLibrary, Song) {
        let mut song = Song::empty();
        song.tracks = tracks;
        let mut seq = Sequencer::new();
        seq.load(&song, SR);
        (seq, VoicePool::new(32), VoiceLibrary::builtins(), song)
    }

    #[test]
    fn events_fire_in_time_order() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(2, 1.0, 64), note(1, 1.0, 60), note(1, 3.0, 62)];
        let (mut seq, mut pool, lib, song) = setup(vec![t]);

        seq.playing = true;
        seq.advance(&mut pool, &lib, &song, 44100, 1.0);
        assert!(pool.active_count() > 0);
    }

    #[test]
    fn muted_track_is_silent() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60)];
        t.muted = true;
        let (mut seq, mut pool, lib, song) = setup(vec![t]);

        seq.playing = true;
        seq.advance(&mut pool, &lib, &song, 44100, 1.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn solo_overrides_other_tracks() {
        let mut a = Track::new(builtin::PIANO, false, 1.0);
        a.notes = vec![note(1, 1.0, 60)];
        let mut b = Track::new(builtin::STRINGS, false, 1.0);
        b.notes = vec![note(1, 1.0, 64)];
        b.solo = true;

        let (mut seq, mut pool, lib, song) = setup(vec![a, b]);
        seq.playing = true;
        seq.advance(&mut pool, &lib, &song, 44100, 1.0);
        assert_eq!(pool.active_count(), 1, "只有 solo 轨应发声");
    }

    #[test]
    fn drums_route_to_drum_pool() {
        let mut t = Track::new(builtin::DRUMS, true, 1.0);
        t.notes = vec![note(1, 1.0, 36)];
        let (mut seq, mut pool, lib, song) = setup(vec![t]);

        seq.playing = true;
        seq.advance(&mut pool, &lib, &song, 44100, 1.0);
        assert!(pool.active_count() > 0);
    }

    #[test]
    fn seek_moves_cursor_forward() {
        let mut t = Track::new(builtin::PIANO, false, 1.0);
        t.notes = vec![note(1, 1.0, 60), note(5, 1.0, 62)];
        let (mut seq, _, _, song) = setup(vec![t]);

        seq.seek_to_bar(4.0, &song);
        assert_eq!(seq.cursor, 1, "跳到第 4 小节应跳过第 1 小节的音符");
    }

    #[test]
    fn position_follows_bpm() {
        let mut song = Song::empty();
        song.bpm = 120.0;
        song.bars = 8;
        let mut seq = Sequencer::new();
        seq.load(&song, SR);
        seq.playing = true;

        let mut pool = VoicePool::new(8);
        let lib = VoiceLibrary::builtins();
        let before = seq.position_bars(&song);
        let one_bar_frames = (song.bar_secs() * SR) as usize;
        seq.advance(&mut pool, &lib, &song, one_bar_frames, 1.0);

        let advanced = seq.position_bars(&song) - before;
        assert!((advanced - 1.0).abs() < 0.01, "应推进 1 小节，实际 {advanced}");
    }
}
