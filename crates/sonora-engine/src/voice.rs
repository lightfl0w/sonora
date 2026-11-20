use sonora_dsp::{Biquad, FastSin};

use crate::preset::{VoicePreset, MAX_PARTIALS};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Attack,
    Sustain,
    Release,
    Done,
}

pub struct SineVoice {
    phase: [f32; MAX_PARTIALS],
    phase_inc: [f32; MAX_PARTIALS],
    gains: [f32; MAX_PARTIALS],
    detune: [f32; MAX_PARTIALS],

    amp: f32,
    amp_target: f32,
    stage: Stage,
    attack_inc: f32,
    release_dec: f32,
    hold_frames: u64,

    cutoff: f32,
    cutoff_target: f32,
    lp: Biquad,

    lfo_phase: f32,
    lfo_inc: f32,
    lfo_depth: f32,

    pan_l: f32,
    pan_r: f32,

    preset: VoicePreset,
    pub note_id: u64,
    pub active: bool,
}

impl SineVoice {
    pub fn new() -> Self {
        Self {
            phase: [0.0; MAX_PARTIALS],
            phase_inc: [0.0; MAX_PARTIALS],
            gains: [0.0; MAX_PARTIALS],
            detune: [0.0; MAX_PARTIALS],
            amp: 0.0,
            amp_target: 0.0,
            stage: Stage::Done,
            attack_inc: 0.0,
            release_dec: 0.0,
            hold_frames: 0,
            cutoff: 1000.0,
            cutoff_target: 1000.0,
            lp: Biquad::default(),
            lfo_phase: 0.0,
            lfo_inc: 0.0,
            lfo_depth: 0.0,
            pan_l: std::f32::consts::FRAC_1_SQRT_2,
            pan_r: std::f32::consts::FRAC_1_SQRT_2,
            preset: VoicePreset::silent(0.1),
            note_id: 0,
            active: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn trigger(
        &mut self,
        preset: &VoicePreset,
        freq: f32,
        dur: f32,
        level: f32,
        pan: f32,
        sr: f32,
        note_id: u64,
        seed: f32,
    ) {
        self.preset = *preset;
        self.note_id = note_id;
        self.active = true;
        self.stage = Stage::Attack;
        self.amp = 0.0;
        self.amp_target = level * preset.level;

        let attack = if preset.is_slow_attack() {
            (dur * 0.6).max(0.001)
        } else {
            preset.attack
        };
        self.attack_inc = self.amp_target / (attack * sr).max(1.0);
        self.release_dec = 1.0 / ((preset.release * 0.35 + 0.05) * sr);
        self.hold_frames = (dur * sr) as u64;

        self.cutoff = preset.cutoff;
        self.cutoff_target = preset.cutoff * (1.0 - preset.cutoff_sweep);
        self.lp.set_lowpass(self.cutoff, 0.3, sr);

        self.lfo_phase = seed.fract();
        self.lfo_inc = preset.drift_rate / sr.max(1.0);
        self.lfo_depth = preset.drift_depth;

        let pan = pan.clamp(-1.0, 1.0);
        let a = (pan + 1.0) * 0.25 * std::f32::consts::PI;
        self.pan_l = a.cos();
        self.pan_r = a.sin();

        for i in 0..MAX_PARTIALS {
            if i < preset.count {
                let partial = preset.partials[i];
                self.gains[i] = partial.gain;
                self.phase[i] = 0.0;
                self.detune[i] = partial.detune_cents;
                self.phase_inc[i] =
                    freq * partial.ratio * cents_to_ratio(partial.detune_cents) / sr;
            } else {
                self.gains[i] = 0.0;
                self.phase_inc[i] = 0.0;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], sr: f32, table: &FastSin) -> bool {
        if !self.active {
            return false;
        }
        let frames = out_l.len().min(out_r.len());

        for n in 0..frames {
            match self.stage {
                Stage::Attack => {
                    self.amp += self.attack_inc;
                    if self.amp >= self.amp_target {
                        self.amp = self.amp_target;
                        self.stage = Stage::Sustain;
                    }
                }
                Stage::Sustain => {
                    if self.hold_frames > 0 {
                        self.hold_frames -= 1;
                        if self.hold_frames == 0 {
                            self.stage = Stage::Release;
                        }
                    } else {
                        self.stage = Stage::Release;
                    }
                }
                Stage::Release => {
                    self.amp -= self.release_dec * self.amp_target;
                    if self.amp <= 0.0 {
                        self.amp = 0.0;
                        self.active = false;
                        self.stage = Stage::Done;
                        return false;
                    }
                }
                Stage::Done => return false,
            }

            self.lfo_phase += self.lfo_inc;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }

            let mut sample = 0.0;
            for i in 0..MAX_PARTIALS {
                let g = self.gains[i];
                if g == 0.0 {
                    continue;
                }
                self.phase[i] += self.phase_inc[i];
                if self.phase[i] >= 1.0 {
                    self.phase[i] -= 1.0;
                }
                sample += table.eval(self.phase[i]) * g;
            }

            sample = self.lp.process(sample) * self.amp;
            out_l[n] += sample * self.pan_l;
            out_r[n] += sample * self.pan_r;
        }

        if self.cutoff > self.cutoff_target {
            let step = 2000.0 / (sr * 0.6).max(1.0);
            self.cutoff = (self.cutoff - step).max(self.cutoff_target);
            self.lp.set_lowpass(self.cutoff, 0.3, sr);
        }

        self.active
    }
}

impl Default for SineVoice {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
pub fn cents_to_ratio(cents: f32) -> f32 {
    (cents / 1200.0 * std::f32::consts::LN_2).exp()
}

#[inline]
pub fn midi_to_freq(note: f32) -> f32 {
    440.0 * cents_to_ratio((note - 69.0) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset;
    use crate::preset::Partial;

    const SR: f32 = 44100.0;

    fn table() -> FastSin {
        FastSin::new()
    }

    #[test]
    fn cents_conversion_is_accurate() {
        assert!((cents_to_ratio(0.0) - 1.0).abs() < 1e-6);
        assert!((cents_to_ratio(1200.0) - 2.0).abs() < 1e-4);
        assert!((cents_to_ratio(-1200.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn midi_a4_is_440() {
        assert!((midi_to_freq(69.0) - 440.0).abs() < 1e-3);
        assert!((midi_to_freq(60.0) - 261.63).abs() < 0.1);
    }

    #[test]
    fn renders_audible_signal() {
        let t = table();
        let mut v = SineVoice::new();
        v.trigger(&preset::piano(), 440.0, 1.0, 1.0, 0.0, SR, 1, 0.5);

        let n = 8192;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        v.render(&mut l, &mut r, SR, &t);

        let peak = l.iter().chain(r.iter()).map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.001, "应产生信号，实际 {peak}");
    }

    #[test]
    fn voice_deactivates_after_release() {
        let t = table();
        let mut v = SineVoice::new();
        v.trigger(&preset::piano(), 440.0, 0.01, 1.0, 0.0, SR, 1, 0.0);

        let n = 512;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];

        let mut frames = 0;
        while v.is_active() && frames < SR as usize * 5 {
            v.render(&mut l, &mut r, SR, &t);
            frames += n;
        }
        assert!(!v.is_active(), "释放后声部应结束");
    }

    #[test]
    fn panning_balances_channels() {
        let t = table();

        let channel_energy = |pan: f32| {
            let mut v = SineVoice::new();
            v.trigger(&preset::piano(), 440.0, 1.0, 1.0, pan, SR, 1, 0.0);
            let n = 4096;
            let mut l = vec![0.0f32; n];
            let mut r = vec![0.0f32; n];
            v.render(&mut l, &mut r, SR, &t);
            (
                l.iter().map(|s| s * s).sum::<f32>(),
                r.iter().map(|s| s * s).sum::<f32>(),
            )
        };

        let (l, r) = channel_energy(-1.0);
        assert!(l > r * 10.0, "全左时左声道应远大于右");

        let (l, r) = channel_energy(1.0);
        assert!(r > l * 10.0, "全右时右声道应远大于左");

        let (l, r) = channel_energy(0.0);
        assert!((l - r).abs() / l.max(r) < 0.05, "居中时两声道应平衡");
    }

    #[test]
    fn multiple_partials_are_summed() {
        let t = table();
        let single = VoicePreset {
            partials: [
                Partial {
                    ratio: 1.0,
                    gain: 1.0,
                    decay_scale: 1.0,
                    detune_cents: 0.0,
                },
                Partial::default(),
                Partial::default(),
                Partial::default(),
                Partial::default(),
                Partial::default(),
            ],
            count: 1,
            ..preset::piano()
        };

        let energy = |p: &VoicePreset| {
            let mut v = SineVoice::new();
            v.trigger(p, 440.0, 1.0, 1.0, 0.0, SR, 1, 0.0);
            let n = 4096;
            let mut l = vec![0.0f32; n];
            let mut r = vec![0.0f32; n];
            v.render(&mut l, &mut r, SR, &t);
            l.iter().map(|s| s * s).sum::<f32>()
        };

        let one = energy(&single);
        let three = energy(&preset::piano());
        assert!(three > one, "3 个分音应比 1 个更响");
    }
}
