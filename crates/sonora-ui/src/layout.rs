pub const TITLEBAR_H: f32 = 34.0;
pub const TRANSPORT_H: f32 = 46.0;
pub const STATUSBAR_H: f32 = 24.0;

pub const TRACK_HEADER_W: f32 = 232.0;
pub const TRACK_ROW_H: f32 = 64.0;
pub const TRACK_STRIP_W: f32 = 6.0;

pub const RULER_H: f32 = 26.0;
pub const KEY_COL_W: f32 = 56.0;

pub const PIANO_ROW_H: f32 = 14.0;
pub const PIANO_MIN_PITCH: u8 = 21;
pub const PIANO_MAX_PITCH: u8 = 108;

pub const LOW_PITCH: u8 = 24;
pub const HIGH_PITCH: u8 = 127;

pub const ROW_H_MIN: f32 = 10.0;
pub const ROW_H_MAX: f32 = 60.0;

pub const MIN_BAR_W: f32 = 18.0;
pub const DEFAULT_BAR_W: f32 = 96.0;
pub const MAX_BAR_W: f32 = 480.0;

pub const MIN_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 5.0;
pub const DEFAULT_ZOOM: f32 = 1.0;
pub const MIN_PITCH_SCROLL: usize = 0;

pub fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        DEFAULT_ZOOM
    }
}

pub fn clamp_scroll(scroll: f32, max: f32) -> f32 {
    if !scroll.is_finite() {
        return 0.0;
    }
    scroll.clamp(0.0, max.max(0.0))
}

pub fn row_of_pitch(pitch: u8) -> usize {
    (PIANO_MAX_PITCH.saturating_sub(pitch)) as usize
}

pub fn pitch_of_row(row: usize) -> Option<u8> {
    let span = (PIANO_MAX_PITCH - PIANO_MIN_PITCH + 1) as usize;
    if row >= span {
        return None;
    }
    Some(PIANO_MAX_PITCH - row as u8)
}

pub fn total_rows() -> usize {
    (PIANO_MAX_PITCH - PIANO_MIN_PITCH + 1) as usize
}

pub fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

pub fn max_scroll_bars(bars: u32, viewport_w: f32, bar_w: f32) -> f32 {
    let content = bars as f32 * bar_w;
    (content - viewport_w).max(0.0)
}

pub fn bar_at(x: f32, bar_w: f32, bars: u32) -> u32 {
    if bar_w <= 0.0 {
        return 1;
    }
    let raw = (x / bar_w).floor();
    if raw < 0.0 {
        1
    } else {
        ((raw as u32) + 1).min(bars.max(1))
    }
}

pub fn beat_at(x: f32, bar_w: f32) -> f32 {
    if bar_w <= 0.0 {
        return 1.0;
    }
    let beats = x / bar_w * 4.0;
    let within = beats.rem_euclid(4.0);
    (within + 1.0).clamp(1.0, 4.0)
}

pub fn snap_beat(beat: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        return beat.clamp(1.0, 4.0);
    }
    let zero_based = beat - 1.0;
    let snapped = (zero_based / grid).round() * grid;
    (snapped + 1.0).clamp(1.0, 4.0)
}

pub fn note_x(bar: u32, beat: f32, bar_w: f32) -> f32 {
    let beats = (bar.saturating_sub(1)) as f32 * 4.0 + (beat - 1.0);
    beats / 4.0 * bar_w
}

pub fn note_w(len: f32, bar_w: f32) -> f32 {
    (len / 4.0 * bar_w).max(2.0)
}

pub fn pitch_y(pitch: u8, row_h: f32) -> f32 {
    row_of_pitch(pitch) as f32 * row_h
}

pub fn pitch_at_y(y: f32, row_h: f32) -> Option<u8> {
    if row_h <= 0.0 || y < 0.0 {
        return None;
    }
    pitch_of_row((y / row_h).floor() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pitch_and_row_roundtrip() {
        for pitch in PIANO_MIN_PITCH..=PIANO_MAX_PITCH {
            let row = row_of_pitch(pitch);
            assert_eq!(pitch_of_row(row), Some(pitch));
        }
    }
    #[test]
    fn row_zero_is_highest_pitch() {
        assert_eq!(pitch_of_row(0), Some(PIANO_MAX_PITCH));
    }
    #[test]
    fn out_of_range_rows_are_none() {
        assert_eq!(pitch_of_row(total_rows()), None);
        assert_eq!(pitch_of_row(total_rows() + 10), None);
    }
    #[test]
    fn black_keys_are_correct() {
        assert!(!is_black_key(60));
        assert!(is_black_key(61));
        assert!(!is_black_key(62));
        assert!(is_black_key(63));
        assert!(!is_black_key(64));
        assert!(!is_black_key(65));
        assert!(is_black_key(66));
    }
    #[test]
    fn bar_at_positions() {
        assert_eq!(bar_at(0.0, 100.0, 8), 1);
        assert_eq!(bar_at(99.0, 100.0, 8), 1);
        assert_eq!(bar_at(100.0, 100.0, 8), 2);
        assert_eq!(bar_at(750.0, 100.0, 8), 8);
        assert_eq!(bar_at(9999.0, 100.0, 8), 8);
    }
    #[test]
    fn bar_at_handles_zero_width() {
        assert_eq!(bar_at(50.0, 0.0, 8), 1);
    }
    #[test]
    fn beat_at_maps_within_bar() {
        assert!((beat_at(0.0, 100.0) - 1.0).abs() < 1e-5);
        assert!((beat_at(25.0, 100.0) - 2.0).abs() < 1e-5);
        assert!((beat_at(50.0, 100.0) - 3.0).abs() < 1e-5);
        assert!((beat_at(75.0, 100.0) - 4.0).abs() < 1e-5);
    }
    #[test]
    fn snap_beat_alignment() {
        assert!((snap_beat(1.4, 1.0) - 1.0).abs() < 1e-5);
        assert!((snap_beat(1.6, 1.0) - 2.0).abs() < 1e-5);
        assert!((snap_beat(2.1, 0.5) - 2.0).abs() < 1e-5);
        assert!((snap_beat(2.3, 0.5) - 2.5).abs() < 1e-5);
    }
    #[test]
    fn snap_beat_zero_grid_just_clamps() {
        assert!((snap_beat(2.3, 0.0) - 2.3).abs() < 1e-5);
        assert!((snap_beat(9.0, 0.0) - 4.0).abs() < 1e-5);
    }
    #[test]
    fn note_geometry() {
        assert!((note_x(1, 1.0, 100.0) - 0.0).abs() < 1e-5);
        assert!((note_x(2, 1.0, 100.0) - 100.0).abs() < 1e-5);
        assert!((note_x(1, 2.0, 100.0) - 25.0).abs() < 1e-5);
        assert!((note_w(4.0, 100.0) - 100.0).abs() < 1e-5);
        assert!((note_w(1.0, 100.0) - 25.0).abs() < 1e-5);
    }
    #[test]
    fn note_width_has_minimum() {
        assert!(note_w(0.01, 10.0) >= 2.0);
    }
    #[test]
    fn pitch_y_and_hit_test() {
        let row_h = 10.0;
        let y = pitch_y(60, row_h);
        assert_eq!(pitch_at_y(y + 1.0, row_h), Some(60));
        assert_eq!(pitch_at_y(-1.0, row_h), None);
        assert_eq!(pitch_at_y(0.0, 0.0), None);
    }
    #[test]
    fn scroll_is_zero_when_content_fits() {
        assert_eq!(max_scroll_bars(4, 1000.0, 100.0), 0.0);
        assert!((max_scroll_bars(8, 100.0, 100.0) - 700.0).abs() < 1e-5);
    }
}
