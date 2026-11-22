const TABLE_SIZE: usize = 1024;

pub struct FastSin {
    table: [f32; TABLE_SIZE + 1],
}

impl FastSin {
    pub fn new() -> Self {
        let mut table = [0.0f32; TABLE_SIZE + 1];
        for (i, slot) in table.iter_mut().enumerate() {
            let phase = i as f32 / TABLE_SIZE as f32;
            *slot = (phase * core::f32::consts::TAU).sin();
        }
        Self { table }
    }
    #[inline]
    pub fn eval(&self, phase: f32) -> f32 {
        let p = phase - phase.floor();
        let x = p * TABLE_SIZE as f32;
        let i = x as usize;
        let frac = x - i as f32;
        let a = self.table[i.min(TABLE_SIZE)];
        let b = self.table[(i + 1).min(TABLE_SIZE)];
        a + (b - a) * frac
    }
}

impl Default for FastSin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_std_sin_closely() {
        let f = FastSin::new();
        let mut max_err = 0.0f32;
        for i in 0..10000 {
            let phase = i as f32 / 10000.0;
            let expected = (phase * core::f32::consts::TAU).sin();
            let err = (f.eval(phase) - expected).abs();
            max_err = max_err.max(err);
        }
        assert!(max_err < 1e-4, "最大误差 {max_err} 应小于 1e-4");
    }
    #[test]
    fn anchors_are_exact() {
        let f = FastSin::new();
        assert!(f.eval(0.0).abs() < 1e-6);
        assert!((f.eval(0.25) - 1.0).abs() < 1e-6);
        assert!(f.eval(0.5).abs() < 1e-6);
        assert!((f.eval(0.75) + 1.0).abs() < 1e-6);
    }
    #[test]
    fn wraps_out_of_range_phase() {
        let f = FastSin::new();
        assert!((f.eval(1.25) - 1.0).abs() < 1e-5);
        assert!((f.eval(-0.75) - 1.0).abs() < 1e-5);
        assert!((f.eval(2.25) - 1.0).abs() < 1e-5);
    }
    #[test]
    fn output_stays_in_unit_range() {
        let f = FastSin::new();
        for i in 0..50000 {
            let v = f.eval(i as f32 / 50000.0);
            assert!(v.abs() <= 1.0001, "输出 {v} 超出 [-1,1]");
        }
    }
}
