use serde::{Deserialize, Serialize};

pub const MAX_PARTIALS: usize = 6;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Partial {
    pub ratio: f32,
    pub gain: f32,
    pub decay_scale: f32,
    pub detune_cents: f32,
}

impl Default for Partial {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            gain: 0.0,
            decay_scale: 1.0,
            detune_cents: 0.0,
        }
    }
}

const fn p(ratio: f32, gain: f32, decay: f32, detune: f32) -> Partial {
    Partial {
        ratio,
        gain,
        decay_scale: decay,
        detune_cents: detune,
    }
}

const SILENT: Partial = p(0.0, 0.0, 1.0, 0.0);

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct VoicePreset {
    pub partials: [Partial; MAX_PARTIALS],
    pub count: usize,
    pub attack: f32,
    pub release: f32,
    pub cutoff: f32,
    pub cutoff_sweep: f32,
    pub drift_rate: f32,
    pub drift_depth: f32,
    pub level: f32,
}

impl VoicePreset {
    pub const fn silent(level: f32) -> Self {
        Self {
            partials: [SILENT; MAX_PARTIALS],
            count: 0,
            attack: 0.01,
            release: 0.3,
            cutoff: 8000.0,
            cutoff_sweep: 0.0,
            drift_rate: 0.0,
            drift_depth: 0.0,
            level,
        }
    }
    pub fn active_partials(&self) -> &[Partial] {
        &self.partials[..self.count.min(MAX_PARTIALS)]
    }
    pub fn is_slow_attack(&self) -> bool {
        self.attack >= 0.2
    }
    pub fn clamp_params(&mut self) {
        self.count = self.count.min(MAX_PARTIALS);
        self.attack = self.attack.clamp(0.0005, 4.0);
        self.release = self.release.clamp(0.02, 8.0);
        self.cutoff = self.cutoff.clamp(80.0, 16000.0);
        self.cutoff_sweep = self.cutoff_sweep.clamp(0.0, 1.0);
        self.drift_rate = self.drift_rate.clamp(0.0, 2.0);
        self.drift_depth = self.drift_depth.clamp(0.0, 20.0);
        self.level = self.level.clamp(0.0, 0.5);
        for partial in self.partials.iter_mut() {
            partial.ratio = partial.ratio.clamp(0.25, 16.0);
            partial.gain = partial.gain.clamp(0.0, 1.0);
            partial.decay_scale = partial.decay_scale.clamp(0.05, 2.0);
            partial.detune_cents = partial.detune_cents.clamp(-50.0, 50.0);
        }
    }
}

pub fn piano() -> VoicePreset {
    VoicePreset {
        partials: [
            p(1.0, 1.00, 1.0, 0.0),
            p(2.0, 0.18, 0.6, 0.0),
            p(3.0, 0.05, 0.4, 0.0),
            SILENT,
            SILENT,
            SILENT,
        ],
        count: 3,
        attack: 0.015,
        release: 0.85,
        cutoff: 4200.0,
        cutoff_sweep: 0.72,
        drift_rate: 0.0,
        drift_depth: 0.0,
        level: 0.075,
    }
}

pub fn strings() -> VoicePreset {
    VoicePreset {
        partials: [
            p(1.0, 0.2, 1.0, -8.0),
            p(1.0, 0.2, 1.0, -3.0),
            p(1.0, 0.2, 1.0, 0.0),
            p(1.0, 0.2, 1.0, 3.0),
            p(1.0, 0.2, 1.0, 8.0),
            SILENT,
        ],
        count: 5,
        attack: 1.2,
        release: 2.0,
        cutoff: 1400.0,
        cutoff_sweep: 0.0,
        drift_rate: 0.23,
        drift_depth: 3.0,
        level: 0.018,
    }
}

pub fn harp() -> VoicePreset {
    VoicePreset {
        partials: [
            p(1.0, 1.0, 1.0, 0.0),
            p(2.0, 0.12, 1.0, 0.0),
            SILENT,
            SILENT,
            SILENT,
            SILENT,
        ],
        count: 2,
        attack: 0.008,
        release: 1.0,
        cutoff: 3800.0,
        cutoff_sweep: 0.76,
        drift_rate: 0.0,
        drift_depth: 0.0,
        level: 0.04,
    }
}

pub fn ambient() -> VoicePreset {
    VoicePreset {
        partials: [
            p(1.0, 0.33, 1.0, -12.0),
            p(1.0, 0.34, 1.0, 0.0),
            p(1.0, 0.33, 1.0, 12.0),
            SILENT,
            SILENT,
            SILENT,
        ],
        count: 3,
        attack: 0.6,
        release: 3.0,
        cutoff: 800.0,
        cutoff_sweep: 0.0,
        drift_rate: 0.11,
        drift_depth: 6.0,
        level: 0.022,
    }
}

pub fn bass() -> VoicePreset {
    VoicePreset {
        partials: [
            p(1.0, 1.00, 1.0, 0.0),
            p(2.0, 0.28, 0.55, 0.0),
            p(3.0, 0.08, 0.35, 0.0),
            SILENT,
            SILENT,
            SILENT,
        ],
        count: 3,
        attack: 0.008,
        release: 0.20,
        cutoff: 900.0,
        cutoff_sweep: 0.55,
        drift_rate: 0.0,
        drift_depth: 0.0,
        level: 0.085,
    }
}

pub fn drums() -> VoicePreset {
    VoicePreset::silent(0.5)
}

pub fn click(accent: bool) -> VoicePreset {
    let mut preset = VoicePreset::silent(if accent { 0.16 } else { 0.10 });
    preset.partials[0] = p(1.0, 1.0, 0.2, 0.0);
    preset.count = 1;
    preset.attack = 0.001;
    preset.release = 0.045;
    preset.cutoff = 12000.0;
    preset
}

pub fn builtin_for(id: sonora_core::VoiceId) -> VoicePreset {
    use sonora_core::voice::builtin as b;
    match id {
        b::PIANO => piano(),
        b::STRINGS => strings(),
        b::HARP => harp(),
        b::AMBIENT => ambient(),
        b::BASS => bass(),
        b::DRUMS => drums(),
        _ => piano(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::voice::builtin;
    #[test]
    fn builtin_presets_have_active_partials() {
        for id in builtin::ALL {
            if id == builtin::DRUMS {
                continue;
            }
            let preset = builtin_for(id);
            assert!(preset.count > 0, "音色 {id} 应有分音");
            assert!(preset.count <= MAX_PARTIALS);
        }
    }
    #[test]
    fn clamp_keeps_params_in_range() {
        let mut preset = VoicePreset::silent(99.0);
        preset.count = 99;
        preset.attack = -1.0;
        preset.cutoff = 1e9;
        preset.partials[0].ratio = 100.0;
        preset.partials[0].gain = 5.0;
        preset.clamp_params();
        assert_eq!(preset.count, MAX_PARTIALS);
        assert!(preset.attack >= 0.0005);
        assert!(preset.cutoff <= 16000.0);
        assert!(preset.partials[0].ratio <= 16.0);
        assert!(preset.partials[0].gain <= 1.0);
        assert!(preset.level <= 0.5);
    }
    #[test]
    fn slow_attack_detected() {
        assert!(strings().is_slow_attack());
        assert!(!piano().is_slow_attack());
    }
    #[test]
    fn active_partials_respect_count() {
        let preset = piano();
        assert_eq!(preset.active_partials().len(), preset.count);
    }
    #[test]
    fn save_and_load_roundtrip() {
        let preset = strings();
        let json = serde_json::to_string(&preset).unwrap();
        let back: VoicePreset = serde_json::from_str(&json).unwrap();
        assert_eq!(back.count, preset.count);
        assert!((back.attack - preset.attack).abs() < 1e-6);
        assert_eq!(
            back.partials[0].detune_cents,
            preset.partials[0].detune_cents
        );
    }
}
