#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_small_signals() {
        for x in [-0.1f32, -0.01, 0.0, 0.01, 0.1] {
            let y = soft_clip(x);
            assert!((y - x).abs() < 1e-3, "{x} → {y} 应近似直通");
        }
    }

    #[test]
    fn compresses_large_signals() {
        assert!(soft_clip(2.0) < 2.0);
        assert!(soft_clip(3.0) < 3.0);
        assert!(soft_clip(-3.0) > -3.0);
    }

    #[test]
    fn stays_bounded_for_extreme_input() {
        for x in [10.0f32, -10.0, 1000.0, -1000.0] {
            let y = soft_clip(x);
            assert!(y.abs() <= 1.0, "soft_clip({x}) = {y} 应不超过 1");
        }
    }

    #[test]
    fn is_odd_function() {
        for x in [0.5f32, 1.0, 2.5] {
            let a = soft_clip(x);
            let b = soft_clip(-x);
            assert!((a + b).abs() < 1e-6, "应为奇函数：{a} vs {b}");
        }
    }

    #[test]
    fn is_monotonic() {
        let mut prev = soft_clip(-3.0);
        let mut x = -3.0f32;
        while x <= 3.0 {
            let y = soft_clip(x);
            assert!(y >= prev - 1e-6, "应单调递增：{x}");
            prev = y;
            x += 0.01;
        }
    }
}
