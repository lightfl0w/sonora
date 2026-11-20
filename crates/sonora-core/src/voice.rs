pub type VoiceId = u32;

pub const USER_ID_BASE: VoiceId = 100;

pub mod builtin {
    use super::VoiceId;

    pub const PIANO: VoiceId = 0;
    pub const STRINGS: VoiceId = 1;
    pub const HARP: VoiceId = 2;
    pub const AMBIENT: VoiceId = 3;
    pub const BASS: VoiceId = 4;
    pub const DRUMS: VoiceId = 5;

    pub const COUNT: usize = 6;

    pub const ALL: [VoiceId; COUNT] = [PIANO, STRINGS, HARP, AMBIENT, BASS, DRUMS];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ids_are_contiguous() {
        for (i, id) in builtin::ALL.iter().enumerate() {
            assert_eq!(*id, i as VoiceId);
        }
    }

    #[test]
    fn user_ids_do_not_collide() {
        assert!(USER_ID_BASE > builtin::ALL[builtin::COUNT - 1]);
    }
}
