use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Default)]
pub struct Transport {
    playing: AtomicBool,
    position: AtomicU32,
    bar: AtomicU32,
    voices: AtomicU32,
    peak_l: AtomicU32,
    peak_r: AtomicU32,
}

impl Transport {
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub fn position_bars(&self) -> f32 {
        f32::from_bits(self.position.load(Ordering::Relaxed))
    }

    pub fn bar(&self) -> u32 {
        self.bar.load(Ordering::Relaxed)
    }

    pub fn voices(&self) -> u32 {
        self.voices.load(Ordering::Relaxed)
    }

    pub fn take_peaks(&self) -> (f32, f32) {
        let l = f32::from_bits(self.peak_l.swap(0, Ordering::Relaxed));
        let r = f32::from_bits(self.peak_r.swap(0, Ordering::Relaxed));
        (l, r)
    }

    pub fn set_playing(&self, v: bool) {
        self.playing.store(v, Ordering::Relaxed);
    }

    pub fn set_position(&self, bars: f32) {
        self.position.store(bars.to_bits(), Ordering::Relaxed);
    }

    pub fn set_bar(&self, bar: u32) {
        self.bar.store(bar, Ordering::Relaxed);
    }

    pub fn set_voices(&self, n: u32) {
        self.voices.store(n, Ordering::Relaxed);
    }

    pub fn push_peaks(&self, l: f32, r: f32) {
        if l > 0.0 || r > 0.0 {
            self.peak_l.store(l.to_bits(), Ordering::Relaxed);
            self.peak_r.store(r.to_bits(), Ordering::Relaxed);
        } else {
            self.peak_l.store(0, Ordering::Relaxed);
            self.peak_r.store(0, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_roundtrips_through_bits() {
        let t = Transport::default();
        t.set_position(3.75);
        assert!((t.position_bars() - 3.75).abs() < 1e-6);

        t.set_position(0.0001);
        assert!(t.position_bars() > 0.0);
    }

    #[test]
    fn peaks_are_consumed_once() {
        let t = Transport::default();
        t.push_peaks(0.8, 0.6);

        let (l, r) = t.take_peaks();
        assert!((l - 0.8).abs() < 1e-6);
        assert!((r - 0.6).abs() < 1e-6);

        let (l, r) = t.take_peaks();
        assert_eq!((l, r), (0.0, 0.0));
    }

    #[test]
    fn silent_block_does_not_keep_old_peak() {
        let t = Transport::default();
        t.push_peaks(0.8, 0.6);
        t.push_peaks(0.0, 0.0);
        assert_eq!(t.take_peaks(), (0.0, 0.0));
    }

    #[test]
    fn flags_update() {
        let t = Transport::default();
        assert!(!t.is_playing());
        t.set_playing(true);
        assert!(t.is_playing());

        t.set_bar(7);
        assert_eq!(t.bar(), 7);

        t.set_voices(5);
        assert_eq!(t.voices(), 5);
    }
}
