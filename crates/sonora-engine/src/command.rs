use sonora_core::{Song, VoiceId};
use serde::{Deserialize, Serialize};

use crate::library::VoiceLibrary;
use crate::preset::VoicePreset;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Param {
    Master(f32),
    ReverbSize(f32),
    ReverbDamp(f32),
    ReverbWet(f32),
    Bpm(f32),
    TrackLevel(usize, f32),
}

#[derive(Clone)]
pub enum Command {
    Play,
    Stop,
    Seek(f32),
    Set(Param),
    Reload(Song),
    UpdateVoices(VoiceLibrary),
    PreviewPreset(Box<VoicePreset>, u8),
    PreviewVoice(VoiceId, u8),
    AllOff,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Play => write!(f, "Play"),
            Command::Stop => write!(f, "Stop"),
            Command::Seek(bar) => write!(f, "Seek({bar})"),
            Command::Set(p) => write!(f, "Set({p:?})"),
            Command::Reload(_) => write!(f, "Reload"),
            Command::UpdateVoices(_) => write!(f, "UpdateVoices"),
            Command::PreviewPreset(_, midi) => write!(f, "PreviewPreset({midi})"),
            Command::PreviewVoice(id, midi) => write!(f, "PreviewVoice({id}, {midi})"),
            Command::AllOff => write!(f, "AllOff"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn param_carries_values() {
        assert_eq!(Param::Master(0.5), Param::Master(0.5));
        assert_ne!(Param::Master(0.5), Param::Master(0.6));
        assert_eq!(Param::TrackLevel(2, 0.7), Param::TrackLevel(2, 0.7));
    }

    #[test]
    fn debug_is_readable() {
        assert_eq!(format!("{:?}", Command::Seek(4.5)), "Seek(4.5)");
        assert_eq!(format!("{:?}", Command::Play), "Play");
    }

    #[test]
    fn param_serializes_for_ui_bridge() {
        let json = serde_json::to_string(&Param::Bpm(120.0)).unwrap();
        let back: Param = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Param::Bpm(120.0));
    }
}
