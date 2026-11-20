use sonora_dsp::FastSin;

use crate::drum::DrumVoice;
use crate::preset::VoicePreset;
use crate::voice::SineVoice;

pub struct VoicePool {
    voices: Vec<SineVoice>,
    drums: Vec<DrumVoice>,
    table: FastSin,
}

impl VoicePool {
    pub fn new(capacity: usize) -> Self {
        Self {
            voices: (0..capacity).map(|_| SineVoice::new()).collect(),
            drums: (0..capacity.min(16)).map(|_| DrumVoice::new()).collect(),
            table: FastSin::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trigger_sine(
        &mut self,
        preset: &VoicePreset,
        freq: f32,
        dur: f32,
        level: f32,
        pan: f32,
        sr: f32,
        note_id: u64,
    ) {
        let idx = self.pick_slot();
        let seed = (note_id as f32).sin().abs();
        self.voices[idx].trigger(preset, freq, dur, level, pan, sr, note_id, seed);
    }

    pub fn trigger_drum(&mut self, midi: u8, dur: f32, level: f32, pan: f32, sr: f32, note_id: u64) {
        let idx = self.pick_drum_slot();
        self.drums[idx].trigger(midi, dur, level, pan, sr, note_id);
    }

    fn pick_slot(&self) -> usize {
        for (i, v) in self.voices.iter().enumerate() {
            if !v.is_active() {
                return i;
            }
        }
        let mut oldest = 0;
        for (i, v) in self.voices.iter().enumerate() {
            if v.note_id < self.voices[oldest].note_id {
                oldest = i;
            }
        }
        oldest
    }

    fn pick_drum_slot(&self) -> usize {
        for (i, v) in self.drums.iter().enumerate() {
            if !v.is_active() {
                return i;
            }
        }
        let mut oldest = 0;
        for (i, v) in self.drums.iter().enumerate() {
            if v.note_id < self.drums[oldest].note_id {
                oldest = i;
            }
        }
        oldest
    }

    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], sr: f32) {
        for v in self.voices.iter_mut() {
            if v.is_active() {
                v.render(out_l, out_r, sr, &self.table);
            }
        }
        for v in self.drums.iter_mut() {
            if v.is_active() {
                v.render(out_l, out_r, sr, &self.table);
            }
        }
    }

    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
            + self.drums.iter().filter(|v| v.is_active()).count()
    }

    pub fn all_off(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
        }
        for v in self.drums.iter_mut() {
            v.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset;

    const SR: f32 = 44100.0;

    #[test]
    fn pool_reuses_idle_voices() {
        let mut pool = VoicePool::new(4);
        pool.trigger_sine(&preset::piano(), 440.0, 0.05, 1.0, 0.0, SR, 1);

        let n = 256;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        for _ in 0..200 {
            pool.render(&mut l, &mut r, SR);
            l.fill(0.0);
            r.fill(0.0);
        }

        pool.trigger_sine(&preset::piano(), 440.0, 1.0, 1.0, 0.0, SR, 2);
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn pool_never_exceeds_capacity() {
        let mut pool = VoicePool::new(3);
        for i in 0..10 {
            pool.trigger_sine(&preset::piano(), 440.0, 2.0, 1.0, 0.0, SR, i);
        }
        assert!(pool.active_count() <= 3);
    }

    #[test]
    fn all_off_silences_everything() {
        let mut pool = VoicePool::new(8);
        pool.trigger_sine(&preset::piano(), 440.0, 2.0, 1.0, 0.0, SR, 1);
        pool.trigger_drum(36, 0.3, 1.0, 0.0, SR, 2);
        assert!(pool.active_count() > 0);

        pool.all_off();
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn mixed_sine_and_drums_render() {
        let mut pool = VoicePool::new(8);
        pool.trigger_sine(&preset::piano(), 440.0, 1.0, 1.0, 0.0, SR, 1);
        pool.trigger_drum(36, 0.3, 1.0, 0.0, SR, 2);

        let n = 4096;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        pool.render(&mut l, &mut r, SR);

        let peak = l.iter().chain(r.iter()).map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.001);
    }
}
