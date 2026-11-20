pub struct Compressor {
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    env: f32,
}

impl Compressor {
    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32, sr: f32) -> Self {
        Self {
            threshold: 10f32.powf(threshold_db / 20.0),
            ratio: ratio.max(1.0),
            attack: (-1.0 / (attack_ms * 0.001 * sr)).exp(),
            release: (-1.0 / (release_ms * 0.001 * sr)).exp(),
            env: 0.0,
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let level = x.abs();
        let coeff = if level > self.env {
            self.attack
        } else {
            self.release
        };
        self.env = level + coeff * (self.env - level);

        if self.env <= self.threshold {
            return x;
        }
        let over = self.env / self.threshold;
        let target = self.threshold * over.powf(1.0 / self.ratio);
        x * (target / self.env)
    }

    pub fn gain_reduction(&self) -> f32 {
        if self.env <= self.threshold {
            1.0
        } else {
            let over = self.env / self.threshold;
            (self.threshold * over.powf(1.0 / self.ratio)) / self.env
        }
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn quiet_signal_passes_unchanged() {
        let mut c = Compressor::new(-20.0, 4.0, 5.0, 100.0, SR);
        for _ in 0..1000 {
            let x = 0.01f32;
            let y = c.process(x);
            assert!((y - x).abs() < 1e-4);
        }
    }

    #[test]
    fn loud_signal_is_attenuated() {
        let mut c = Compressor::new(-20.0, 4.0, 1.0, 100.0, SR);
        let mut last = 0.0f32;
        for _ in 0..10000 {
            last = c.process(0.9);
        }
        assert!(last < 0.9, "输出 {last} 应被压缩");
        assert!(c.gain_reduction() < 1.0);
    }

    #[test]
    fn never_boosts_signal() {
        let mut c = Compressor::new(-24.0, 1.5, 20.0, 400.0, SR);
        let mut rng = 12345u32;
        for _ in 0..20000 {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let x = (rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let y = c.process(x * 0.9);
            assert!(y.abs() <= x.abs() * 0.9 + 1e-6, "压缩器不应放大信号");
        }
    }

    #[test]
    fn reset_clears_envelope() {
        let mut c = Compressor::new(-20.0, 4.0, 1.0, 100.0, SR);
        for _ in 0..1000 {
            c.process(0.9);
        }
        c.reset();
        assert_eq!(c.gain_reduction(), 1.0);
    }
}
