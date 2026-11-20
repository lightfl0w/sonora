pub mod drum;
pub mod pool;
pub mod preset;
pub mod voice;

pub use drum::{DrumKind, DrumVoice};
pub use pool::VoicePool;
pub use preset::{builtin_for, Partial, VoicePreset, MAX_PARTIALS};
pub use voice::{cents_to_ratio, midi_to_freq, SineVoice};
