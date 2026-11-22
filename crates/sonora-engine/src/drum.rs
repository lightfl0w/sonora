use sonora_dsp::FastSin;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrumKind {
    Kick,
    Snare,
    HiHatClosed,
    HiHatOpen,
    Clap,
    Tom,
}

impl DrumKind {
    pub fn from_midi(note: u8) -> Self {
        match note {
            35 | 36 => DrumKind::Kick,
            38 | 40 => DrumKind::Snare,
            39 => DrumKind::Clap,
            42 | 44 => DrumKind::HiHatClosed,
            46 => DrumKind::HiHatOpen,
            41 | 43 | 45 | 47 | 48 | 50 => DrumKind::Tom,
            n if n < 37 => DrumKind::Kick,
            n if n < 40 => DrumKind::Snare,
            n if n < 45 => DrumKind::HiHatClosed,
            n if n < 47 => DrumKind::HiHatOpen,
            _ => DrumKind::Tom,
        }
    }
    fn params(self) -> (f32, f32, f32, f32) {
        match self {
            DrumKind::Kick => (0.95, 0.30, 150.0, 45.0),
            DrumKind::Snare => (0.55, 0.18, 190.0, 160.0),
            DrumKind::HiHatClosed => (0.22, 0.045, 0.0, 0.0),
            DrumKind::HiHatOpen => (0.20, 0.32, 0.0, 0.0),
            DrumKind::Clap => (0.35, 0.16, 0.0, 0.0),
            DrumKind::Tom => (0.60, 0.35, 180.0, 90.0),
        }
    }
}

pub struct DrumVoice {
    kind: DrumKind,
    amp: f32,
    decay: f32,
    phase: f32,
    freq: f32,
    freq_end: f32,
    noise_state: u32,
    hp_z: f32,
    hp_prev: f32,
    burst: u32,
    burst_phase: f32,
    pan_l: f32,
    pan_r: f32,
    level: f32,
    sr: f32,
    pub note_id: u64,
    pub active: bool,
}

impl DrumVoice {
    pub fn new() -> Self {
        Self {
            kind: DrumKind::Kick,
            amp: 0.0,
            decay: 0.0,
            phase: 0.0,
            freq: 60.0,
            freq_end: 40.0,
            noise_state: 0x1234_5678,
            hp_z: 0.0,
            hp_prev: 0.0,
            burst: 0,
            burst_phase: 0.0,
            pan_l: std::f32::consts::FRAC_1_SQRT_2,
            pan_r: std::f32::consts::FRAC_1_SQRT_2,
            level: 0.6,
            sr: 44100.0,
            note_id: 0,
            active: false,
        }
    }
    pub fn trigger(&mut self, midi: u8, dur: f32, level: f32, pan: f32, sr: f32, note_id: u64) {
        let kind = DrumKind::from_midi(midi);
        self.kind = kind;
        self.note_id = note_id;
        self.active = true;
        self.sr = sr;
        self.noise_state = (note_id as u32).wrapping_mul(2_654_435_761) | 1;
        let pan = pan.clamp(-1.0, 1.0);
        let a = (pan + 1.0) * 0.25 * std::f32::consts::PI;
        self.pan_l = a.cos();
        self.pan_r = a.sin();
        let (amp, decay_sec, f0, f1) = kind.params();
        self.level = level * amp;
        self.amp = self.level;
        let dur_scale = (1.0 + (dur - 0.25).clamp(-0.2, 1.5) * 0.15).clamp(0.6, 1.6);
        self.decay = 1.0 / (decay_sec * dur_scale * sr);
        self.freq = f0;
        self.freq_end = f1;
        self.phase = 0.0;
        self.hp_z = 0.0;
        self.hp_prev = 0.0;
        self.burst = if kind == DrumKind::Clap { 3 } else { 0 };
        self.burst_phase = 0.0;
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    #[inline]
    fn noise(&mut self) -> f32 {
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    pub fn render(
        &mut self,
        out_l: &mut [f32],
        out_r: &mut [f32],
        _sr: f32,
        table: &FastSin,
    ) -> bool {
        if !self.active {
            return false;
        }
        let frames = out_l.len().min(out_r.len());
        for n in 0..frames {
            if self.amp <= 0.0 {
                self.active = false;
                return false;
            }
            let s = match self.kind {
                DrumKind::Kick | DrumKind::Tom => {
                    self.phase += self.freq / self.sr;
                    if self.phase >= 1.0 {
                        self.phase -= 1.0;
                    }
                    self.freq += (self.freq_end - self.freq) * 0.0025;
                    let body = table.eval(self.phase);
                    let click = if self.amp > self.level * 0.7 {
                        self.noise() * 0.15
                    } else {
                        0.0
                    };
                    body + click
                }
                DrumKind::Snare => {
                    let nz = self.noise();
                    self.phase += 180.0 / self.sr;
                    if self.phase >= 1.0 {
                        self.phase -= 1.0;
                    }
                    let body = table.eval(self.phase) * 0.25;
                    let hp = nz - self.hp_prev + 0.92 * self.hp_z;
                    self.hp_prev = nz;
                    self.hp_z = hp;
                    hp * 0.8 + body
                }
                DrumKind::HiHatClosed | DrumKind::HiHatOpen => {
                    let nz = self.noise();
                    let hp = nz - self.hp_prev + 0.75 * self.hp_z;
                    self.hp_prev = nz;
                    self.hp_z = hp;
                    hp * 0.7
                }
                DrumKind::Clap => {
                    let env = if self.burst > 0 {
                        self.burst_phase += 1.0 / (self.sr * 0.012);
                        if self.burst_phase >= 1.0 {
                            self.burst_phase = 0.0;
                            self.burst -= 1;
                        }
                        1.0 - self.burst_phase * 0.5
                    } else {
                        0.35
                    };
                    let nz = self.noise();
                    let hp = nz - self.hp_prev + 0.8 * self.hp_z;
                    self.hp_prev = nz;
                    self.hp_z = hp;
                    hp * env
                }
            };
            let sample = s * self.amp;
            out_l[n] += sample * self.pan_l;
            out_r[n] += sample * self.pan_r;
            self.amp -= self.decay * self.level;
        }
        self.active
    }
}

impl Default for DrumVoice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 44100.0;
    fn table() -> FastSin {
        FastSin::new()
    }
    #[test]
    fn gm_midi_maps_to_kinds() {
        assert_eq!(DrumKind::from_midi(36), DrumKind::Kick);
        assert_eq!(DrumKind::from_midi(38), DrumKind::Snare);
        assert_eq!(DrumKind::from_midi(42), DrumKind::HiHatClosed);
        assert_eq!(DrumKind::from_midi(46), DrumKind::HiHatOpen);
        assert_eq!(DrumKind::from_midi(39), DrumKind::Clap);
        assert_eq!(DrumKind::from_midi(45), DrumKind::Tom);
    }
    #[test]
    fn kick_produces_low_frequency_energy() {
        let t = table();
        let mut v = DrumVoice::new();
        v.trigger(36, 0.25, 1.0, 0.0, SR, 1);
        let n = 8192;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        v.render(&mut l, &mut r, SR, &t);
        let mut lp = 0.0f32;
        let mut low = 0.0f64;
        let mut all = 0.0f64;
        for s in &l {
            lp += 0.02 * (s - lp);
            low += (lp as f64) * (lp as f64);
            all += (*s as f64) * (*s as f64);
        }
        let ratio = low / all.max(1e-12);
        assert!(ratio > 0.3, "底鼓低频占比应显著，实际 {ratio:.3}");
    }
    #[test]
    fn hihat_is_short() {
        let t = table();
        let mut v = DrumVoice::new();
        v.trigger(42, 0.25, 1.0, 0.0, SR, 1);
        let n = 1024;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let mut total = 0;
        while v.is_active() && total < SR as usize {
            v.render(&mut l, &mut r, SR, &t);
            total += n;
        }
        assert!(
            total < (SR * 0.3) as usize,
            "闭镲应在 0.3s 内结束，实际 {total} 帧"
        );
    }
    #[test]
    fn drums_are_panned() {
        let t = table();
        let mut v = DrumVoice::new();
        v.trigger(36, 0.25, 1.0, -1.0, SR, 1);
        let n = 2048;
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        v.render(&mut l, &mut r, SR, &t);
        let el: f32 = l.iter().map(|s| s * s).sum();
        let er: f32 = r.iter().map(|s| s * s).sum();
        assert!(el > er * 10.0);
    }
}
