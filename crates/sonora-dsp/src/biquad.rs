#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

enum Kind {
    LowPass,
    HighPass,
    HighShelf,
}

impl Biquad {
    pub fn lowpass(freq: f32, q: f32, sr: f32) -> Self {
        Self::new(Kind::LowPass, freq, q, 0.0, sr)
    }
    pub fn highpass(freq: f32, q: f32, sr: f32) -> Self {
        Self::new(Kind::HighPass, freq, q, 0.0, sr)
    }
    pub fn highshelf(freq: f32, gain_db: f32, sr: f32) -> Self {
        Self::new(Kind::HighShelf, freq, 0.5, gain_db, sr)
    }
    fn new(kind: Kind, freq: f32, q: f32, gain_db: f32, sr: f32) -> Self {
        let mut b = Self::default();
        b.set(kind, freq, q, gain_db, sr);
        b
    }
    pub fn set_lowpass(&mut self, freq: f32, q: f32, sr: f32) {
        self.set(Kind::LowPass, freq, q, 0.0, sr);
    }
    pub fn set_highpass(&mut self, freq: f32, q: f32, sr: f32) {
        self.set(Kind::HighPass, freq, q, 0.0, sr);
    }
    pub fn set_highshelf(&mut self, freq: f32, gain_db: f32, sr: f32) {
        self.set(Kind::HighShelf, freq, 0.5, gain_db, sr);
    }
    fn set(&mut self, kind: Kind, freq: f32, q: f32, gain_db: f32, sr: f32) {
        let w0 = core::f32::consts::TAU * (freq / sr).clamp(0.0001, 0.49);
        let (sn, cs) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match kind {
            Kind::LowPass => {
                let alpha = sn / (2.0 * q.max(0.05));
                let a0 = 1.0 + alpha;
                (
                    (1.0 - cs) * 0.5,
                    1.0 - cs,
                    (1.0 - cs) * 0.5,
                    a0,
                    -2.0 * cs,
                    1.0 - alpha,
                )
            }
            Kind::HighPass => {
                let alpha = sn / (2.0 * q.max(0.05));
                let a0 = 1.0 + alpha;
                (
                    (1.0 + cs) * 0.5,
                    -(1.0 + cs),
                    (1.0 + cs) * 0.5,
                    a0,
                    -2.0 * cs,
                    1.0 - alpha,
                )
            }
            Kind::HighShelf => {
                let a = 10f32.powf(gain_db / 40.0);
                let alpha = sn / 2.0 * core::f32::consts::SQRT_2;
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
                let a0 = (a + 1.0) - (a - 1.0) * cs + two_sqrt_a_alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cs + two_sqrt_a_alpha),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                    a * ((a + 1.0) + (a - 1.0) * cs - two_sqrt_a_alpha),
                    a0,
                    2.0 * ((a - 1.0) - (a + 1.0) * cs),
                    (a + 1.0) - (a - 1.0) * cs - two_sqrt_a_alpha,
                )
            }
        };
        let inv = 1.0 / a0;
        self.b0 = b0 * inv;
        self.b1 = b1 * inv;
        self.b2 = b2 * inv;
        self.a1 = a1 * inv;
        self.a2 = a2 * inv;
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SR: f32 = 44100.0;
    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }
    fn sine(freq: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (core::f32::consts::TAU * freq * i as f32 / SR).sin())
            .collect()
    }
    #[test]
    fn lowpass_passes_low_attenuates_high() {
        let n = 8192;
        let low = sine(200.0, n);
        let high = sine(10000.0, n);
        let mut lp = Biquad::lowpass(1000.0, 0.707, SR);
        let low_out: Vec<f32> = low.iter().map(|s| lp.process(*s)).collect();
        let low_gain = rms(&low_out[2048..]) / rms(&low[2048..]);
        let mut lp = Biquad::lowpass(1000.0, 0.707, SR);
        let high_out: Vec<f32> = high.iter().map(|s| lp.process(*s)).collect();
        let high_gain = rms(&high_out[2048..]) / rms(&high[2048..]);
        assert!(low_gain > 0.9, "低频应基本无损，实际 {low_gain}");
        assert!(high_gain < 0.1, "10kHz 应被大幅衰减，实际 {high_gain}");
    }
    #[test]
    fn highpass_does_the_opposite() {
        let n = 8192;
        let low = sine(100.0, n);
        let high = sine(8000.0, n);
        let mut hp = Biquad::highpass(1000.0, 0.707, SR);
        let low_gain = rms(&low.iter().map(|s| hp.process(*s)).collect::<Vec<_>>()[2048..])
            / rms(&low[2048..]);
        let mut hp = Biquad::highpass(1000.0, 0.707, SR);
        let high_gain = rms(&high.iter().map(|s| hp.process(*s)).collect::<Vec<_>>()[2048..])
            / rms(&high[2048..]);
        assert!(low_gain < 0.1);
        assert!(high_gain > 0.9);
    }
    #[test]
    fn highshelf_reduces_high_band() {
        let n = 8192;
        let high = sine(12000.0, n);
        let mut flat = Biquad::highshelf(6000.0, 0.0, SR);
        let flat_gain = rms(&flat.process_all(&high)[2048..]) / rms(&high[2048..]);
        let mut cut = Biquad::highshelf(6000.0, -6.0, SR);
        let cut_gain = rms(&cut.process_all(&high)[2048..]) / rms(&high[2048..]);
        assert!(
            (flat_gain - 1.0).abs() < 0.05,
            "0dB 应无损，实际 {flat_gain}"
        );
        assert!(cut_gain < 0.75, "-6dB 应衰减，实际 {cut_gain}");
    }
    #[test]
    fn reset_clears_state() {
        let mut f = Biquad::lowpass(1000.0, 0.707, SR);
        for _ in 0..100 {
            f.process(1.0);
        }
        f.reset();
        assert_eq!(f.process(0.0), 0.0);
    }
    impl Biquad {
        fn process_all(&mut self, input: &[f32]) -> Vec<f32> {
            input.iter().map(|s| self.process(*s)).collect()
        }
    }
}
