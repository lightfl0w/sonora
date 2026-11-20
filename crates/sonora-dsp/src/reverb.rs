const NUM_COMBS: usize = 8;
const NUM_ALLPASS: usize = 4;

const COMB_TUNING: [usize; NUM_COMBS] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; NUM_ALLPASS] = [556, 441, 341, 225];

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    store: f32,
    feedback: f32,
    damp: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
            store: 0.0,
            feedback: 0.84,
            damp: 0.55,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buf[self.idx];
        self.store = out * (1.0 - self.damp) + self.store * self.damp;
        self.buf[self.idx] = input + self.store * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
        self.store = 0.0;
    }
}

struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(1)],
            idx: 0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32, feedback: f32) -> f32 {
        let bufout = self.buf[self.idx];
        let out = -input + bufout;
        self.buf[self.idx] = input + bufout * feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        out
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}

pub struct Reverb {
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    aps_l: Vec<Allpass>,
    aps_r: Vec<Allpass>,
    sr: f32,
    size: f32,
    damp: f32,
    wet: f32,
    silence_frames: u32,
}

impl Reverb {
    pub fn new(sr: f32) -> Self {
        let mut r = Self {
            combs_l: Vec::with_capacity(NUM_COMBS),
            combs_r: Vec::with_capacity(NUM_COMBS),
            aps_l: Vec::with_capacity(NUM_ALLPASS),
            aps_r: Vec::with_capacity(NUM_ALLPASS),
            sr,
            size: 0.82,
            damp: 0.55,
            wet: 0.55,
            silence_frames: 0,
        };
        r.rebuild(sr);
        r
    }

    fn rebuild(&mut self, sr: f32) {
        let scale = sr / 44100.0;
        let spread = (23.0 * scale) as usize;

        self.combs_l.clear();
        self.combs_r.clear();
        for &t in COMB_TUNING.iter() {
            self.combs_l.push(Comb::new((t as f32 * scale) as usize));
            self.combs_r
                .push(Comb::new((t as f32 * scale) as usize + spread));
        }

        self.aps_l.clear();
        self.aps_r.clear();
        for &t in ALLPASS_TUNING.iter() {
            self.aps_l.push(Allpass::new((t as f32 * scale) as usize));
            self.aps_r
                .push(Allpass::new((t as f32 * scale) as usize + spread));
        }

        self.sr = sr;
        self.apply();
    }

    fn apply(&mut self) {
        let fb = 0.70 + self.size.clamp(0.0, 1.0) * 0.28;
        let damp = self.damp.clamp(0.0, 1.0) * 0.6;
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            c.feedback = fb;
            c.damp = damp;
        }
    }

    pub fn set_size(&mut self, size: f32) {
        self.size = size;
        self.apply();
    }

    pub fn set_damp(&mut self, damp: f32) {
        self.damp = damp;
        self.apply();
    }

    pub fn set_wet(&mut self, wet: f32) {
        self.wet = wet.clamp(0.0, 1.0);
    }

    pub fn wet(&self) -> f32 {
        self.wet
    }

    pub fn clear(&mut self) {
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            c.clear();
        }
        for a in self.aps_l.iter_mut().chain(self.aps_r.iter_mut()) {
            a.clear();
        }
        self.silence_frames = 0;
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        if frames == 0 {
            return;
        }

        let silent = left[..frames]
            .iter()
            .chain(right[..frames].iter())
            .all(|s| s.abs() < 1e-6);

        if silent {
            self.silence_frames = self.silence_frames.saturating_add(frames as u32);
            if self.silence_frames > (self.sr * 1.5) as u32 {
                self.clear();
                return;
            }
        } else {
            self.silence_frames = 0;
        }

        let wet = self.wet;
        for n in 0..frames {
            let input = (left[n] + right[n]) * 0.5 * 0.015;

            let mut out_l = 0.0;
            let mut out_r = 0.0;
            for c in self.combs_l.iter_mut() {
                out_l += c.process(input);
            }
            for c in self.combs_r.iter_mut() {
                out_r += c.process(input);
            }
            for ap in self.aps_l.iter_mut() {
                out_l = ap.process(out_l, 0.5);
            }
            for ap in self.aps_r.iter_mut() {
                out_r = ap.process(out_r, 0.5);
            }

            left[n] += out_l * wet;
            right[n] += out_r * wet;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44100.0;

    #[test]
    fn impulse_produces_tail() {
        let mut r = Reverb::new(SR);
        let n = 44100;
        let mut l = vec![0.0f32; n];
        let mut rr = vec![0.0f32; n];
        l[0] = 1.0;
        rr[0] = 1.0;
        r.process(&mut l, &mut rr);

        let tail_energy: f32 = l[n / 4..n / 2].iter().map(|s| s * s).sum();
        assert!(tail_energy > 1e-8, "冲激后应有尾巴，实际 {tail_energy}");
    }

    #[test]
    fn silence_clears_eventually() {
        let mut r = Reverb::new(SR);
        let n = 8192;
        let mut l = vec![0.0f32; n];
        let mut rr = vec![0.0f32; n];
        l[0] = 1.0;
        rr[0] = 1.0;
        r.process(&mut l, &mut rr);

        for _ in 0..12 {
            let mut l = vec![0.0f32; n];
            let mut rr = vec![0.0f32; n];
            r.process(&mut l, &mut rr);
        }

        let mut l = vec![0.0f32; n];
        let mut rr = vec![0.0f32; n];
        r.process(&mut l, &mut rr);
        let energy: f32 = l.iter().map(|s| s * s).sum();
        assert!(energy < 1e-10, "静音 1.5s 后应完全清空，实际 {energy}");
    }

    #[test]
    fn larger_size_gives_longer_tail() {
        let energy_at = |size: f32| {
            let mut r = Reverb::new(SR);
            r.set_size(size);
            let n = 44100;
            let mut l = vec![0.0f32; n];
            let mut rr = vec![0.0f32; n];
            l[0] = 1.0;
            rr[0] = 1.0;
            r.process(&mut l, &mut rr);
            l[n / 2..].iter().map(|s| s * s).sum::<f32>()
        };
        assert!(energy_at(0.95) > energy_at(0.3), "大房间尾巴应更长");
    }

    #[test]
    fn wet_zero_passes_dry_through() {
        let mut r = Reverb::new(SR);
        r.set_wet(0.0);
        let n = 1024;
        let input: Vec<f32> = (0..n).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut l = input.clone();
        let mut rr = input.clone();
        r.process(&mut l, &mut rr);
        assert_eq!(l, input);
    }

    #[test]
    fn stereo_output_is_decorrelated() {
        let mut r = Reverb::new(SR);
        let n = 44100;
        let mut l = vec![0.0f32; n];
        let mut rr = vec![0.0f32; n];
        l[0] = 1.0;
        rr[0] = 1.0;
        r.process(&mut l, &mut rr);

        let diff: f32 = l.iter().zip(rr.iter()).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 0.01, "左右声道应有差异，实际 {diff}");
    }
}
