mod engine;
mod error;

pub use engine::AudioEngine;
pub use error::AudioError;

#[cfg(test)]
mod integration {
    use super::*;
    use sonora_core::Song;
    use sonora_engine::command::Command;
    use sonora_engine::library::VoiceLibrary;
    #[test]
    fn error_display_is_readable() {
        assert_eq!(AudioError::NoDevice.to_string(), "找不到音频输出设备");
        assert!(
            AudioError::UnsupportedFormat("x".into())
                .to_string()
                .contains('x')
        );
    }
    #[test]
    fn engine_starts_or_reports_missing_device() {
        match AudioEngine::start(Song::empty(), VoiceLibrary::builtins()) {
            Ok(engine) => {
                assert!(engine.sample_rate > 0.0);
                assert!(engine.channels >= 1);
                assert!(!engine.device_name.is_empty());
                engine.send(Command::Play);
                std::thread::sleep(std::time::Duration::from_millis(120));
                let mut scope = Vec::new();
                engine.scope_snapshot(&mut scope);
                assert_eq!(scope.len(), sonora_engine::SCOPE_LEN);
                engine.send(Command::Stop);
            }
            Err(e) => {
                eprintln!("跳过：本机音频设备不可用（{e}）");
            }
        }
    }
}
