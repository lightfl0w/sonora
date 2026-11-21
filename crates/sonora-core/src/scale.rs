use serde::{Deserialize, Serialize};

pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Mode {
    Major,
    NaturalMinor,
    Dorian,
    Mixolydian,
    PentatonicMajor,
    PentatonicMinor,
    Blues,
    Chromatic,
}

impl Mode {
    pub const ALL: [Mode; 8] = [
        Mode::Major,
        Mode::NaturalMinor,
        Mode::Dorian,
        Mode::Mixolydian,
        Mode::PentatonicMajor,
        Mode::PentatonicMinor,
        Mode::Blues,
        Mode::Chromatic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Major => "大调",
            Mode::NaturalMinor => "自然小调",
            Mode::Dorian => "多利亚",
            Mode::Mixolydian => "混合利底亚",
            Mode::PentatonicMajor => "大调五声",
            Mode::PentatonicMinor => "小调五声",
            Mode::Blues => "布鲁斯",
            Mode::Chromatic => "半音阶",
        }
    }

    pub fn offsets(self) -> &'static [i32] {
        match self {
            Mode::Major => &[0, 2, 4, 5, 7, 9, 11],
            Mode::NaturalMinor => &[0, 2, 3, 5, 7, 8, 10],
            Mode::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Mode::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Mode::PentatonicMajor => &[0, 2, 4, 7, 9],
            Mode::PentatonicMinor => &[0, 3, 5, 7, 10],
            Mode::Blues => &[0, 3, 5, 6, 7, 10],
            Mode::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Scale {
    pub root: u8,
    pub mode: Mode,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            root: 0,
            mode: Mode::Major,
        }
    }
}

impl Scale {
    pub fn new(root: u8, mode: Mode) -> Self {
        Self {
            root: root % 12,
            mode,
        }
    }

    pub fn name(&self) -> String {
        format!("{} {}", NOTE_NAMES[self.root as usize], self.mode.label())
    }

    pub fn degree_of(&self, pitch: u8) -> Option<usize> {
        let pc = ((pitch as i32 - self.root as i32).rem_euclid(12)) as i32;
        self.mode.offsets().iter().position(|&o| o == pc)
    }

    pub fn contains(&self, pitch: u8) -> bool {
        if self.mode == Mode::Chromatic {
            return true;
        }
        self.degree_of(pitch).is_some()
    }

    pub fn snap(&self, pitch: u8, floor: u8, ceil: u8) -> u8 {
        if self.contains(pitch) {
            return pitch;
        }
        for d in 1..=6u8 {
            let up = pitch.saturating_add(d);
            if up <= ceil && self.contains(up) {
                return up;
            }
            let down = pitch.saturating_sub(d);
            if down >= floor && self.contains(down) {
                return down;
            }
        }
        pitch
    }
}

pub fn pitch_name(pitch: u8) -> String {
    let pc = (pitch % 12) as usize;
    let octave = pitch as i32 / 12 - 1;
    format!("{}{}", NOTE_NAMES[pc], octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_contains_white_keys_only() {
        let s = Scale::new(0, Mode::Major);
        for p in [60, 62, 64, 65, 67, 69, 71] {
            assert!(s.contains(p));
        }
        for p in [61, 63, 66, 68, 70] {
            assert!(!s.contains(p));
        }
    }

    #[test]
    fn snap_prefers_nearest_then_up() {
        let s = Scale::new(0, Mode::Major);
        assert_eq!(s.snap(61, 0, 127), 62);
        assert_eq!(s.snap(63, 0, 127), 64);
        assert_eq!(s.snap(60, 0, 127), 60);
    }

    #[test]
    fn chromatic_contains_all() {
        let s = Scale::new(0, Mode::Chromatic);
        assert!((0..=127u8).all(|p| s.contains(p)));
    }

    #[test]
    fn transposition_shifts_membership() {
        let s = Scale::new(2, Mode::Major);
        assert!(s.contains(62));
        assert!(!s.contains(60));
        assert_eq!(s.name(), "D 大调");
    }

    #[test]
    fn pitch_names_follow_scientific_notation() {
        assert_eq!(pitch_name(60), "C4");
        assert_eq!(pitch_name(61), "C#4");
        assert_eq!(pitch_name(69), "A4");
        assert_eq!(pitch_name(127), "G9");
        assert_eq!(pitch_name(24), "C1");
    }

    #[test]
    fn pentatonic_has_five_degrees() {
        let s = Scale::new(0, Mode::PentatonicMajor);
        assert_eq!(s.mode.offsets().len(), 5);
        assert!(s.contains(60));
        assert!(!s.contains(65));
    }
}
