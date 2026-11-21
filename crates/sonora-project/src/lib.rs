mod error;
mod project;

pub use error::ProjectError;
pub use project::{with_extension, Project, EXTENSION, FORMAT, VERSION};

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::scale::Mode;
    use sonora_core::song::{Note, Track};
    use sonora_core::voice::builtin;
    use sonora_engine::VoiceLibrary;

    fn sample() -> Project {
        let mut song = sonora_core::Song::empty();
        song.bpm = 96.0;
        song.bars = 8;
        song.tracks[0].notes.push(Note {
            bar: 1,
            beat: 1.0,
            midi: 60,
            len: 2.0,
            vel: 0.75,
        });
        song.tracks.push(Track::new(builtin::BASS, false, 0.6));

        let mut project = Project::with_song(song, sonora_core::Scale::new(2, Mode::Dorian));
        project.title = "测试工程".into();
        project
    }

    #[test]
    fn roundtrip_preserves_song() {
        let project = sample();
        let bytes = project.to_bytes().unwrap();
        let back = Project::from_bytes(&bytes).unwrap();

        assert_eq!(back.title, "测试工程");
        assert_eq!(back.song.bpm, 96.0);
        assert_eq!(back.song.bars, 8);
        assert_eq!(back.song.tracks.len(), 2);
        assert_eq!(back.song.total_notes(), 1);
        assert_eq!(back.song.tracks[0].notes[0].midi, 60);
        assert_eq!(back.scale.root, 2);
        assert_eq!(back.scale.mode, Mode::Dorian);
    }

    #[test]
    fn empty_project_is_valid() {
        let p = Project::empty();
        let back = Project::from_bytes(&p.to_bytes().unwrap()).unwrap();
        assert_eq!(back.song.total_notes(), 0);
        assert_eq!(back.format, FORMAT);
        assert_eq!(back.version, VERSION);
    }

    #[test]
    fn rejects_foreign_json() {
        let err = Project::from_bytes(br#"{"hello":"world"}"#).unwrap_err();
        assert!(matches!(err, ProjectError::NotProject));
    }

    #[test]
    fn rejects_other_format_name() {
        let err = Project::from_bytes(br#"{"format":"other-daw","version":1}"#).unwrap_err();
        assert!(matches!(err, ProjectError::WrongFormat { .. }));
    }

    #[test]
    fn rejects_newer_version() {
        let json = format!(r#"{{"format":"{FORMAT}","version":99}}"#);
        let err = Project::from_bytes(json.as_bytes()).unwrap_err();
        match err {
            ProjectError::NewerVersion { supported, found } => {
                assert_eq!(supported, VERSION);
                assert_eq!(found, 99);
            }
            other => panic!("期望 NewerVersion，实际 {other:?}"),
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(Project::from_bytes(b"not json at all").is_err());
        assert!(Project::from_bytes(b"").is_err());
    }

    #[test]
    fn sanitize_clamps_corrupt_values() {
        let json = format!(
            r#"{{"format":"{FORMAT}","version":1,"title":"x","scale":{{"root":0,"mode":"Major"}},
            "song":{{"bpm":-500.0,"bars":0,"tracks":[{{"voice":0,"level":99.0,
            "notes":[{{"bar":0,"beat":99.0,"midi":200,"len":-3.0,"vel":5.0}}]}}],
            "master":7.0,"reverb_size":-2.0,"reverb_damp":2.0,"reverb_wet":9.0}}}}"#
        );
        let p = Project::from_bytes(json.as_bytes()).unwrap();

        assert_eq!(p.song.bpm, 20.0);
        assert_eq!(p.song.bars, 1);
        assert_eq!(p.song.master, 1.0);
        assert!(p.song.reverb_wet <= 1.0);

        let note = &p.song.tracks[0].notes[0];
        assert_eq!(note.bar, 1);
        assert_eq!(note.beat, 4.0);
        assert_eq!(note.midi, 127);
        assert!(note.vel <= 1.0);
    }

    #[test]
    fn endian_safe_on_disk() {
        let dir = std::env::temp_dir().join("sonora-project-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("demo.sonora");

        let project = sample();
        project.save(&path).unwrap();
        assert!(path.exists(), "工程文件应已写入");

        let back = Project::load(&path).unwrap();
        assert_eq!(back.song.total_notes(), 1);
        assert_eq!(back.title, "测试工程");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_reports_path() {
        let path = std::env::temp_dir().join("sonora-does-not-exist-xyz.sonora");
        let err = Project::load(&path).unwrap_err();
        match err {
            ProjectError::Read { path: p, .. } => assert_eq!(p, path),
            other => panic!("期望 Read，实际 {other:?}"),
        }
    }

    #[test]
    fn user_voices_are_embedded() {
        let mut lib = VoiceLibrary::builtins();
        let id = lib.create_from(builtin::PIANO, "我的音色".into());

        let mut project = sample();
        project.voices = lib.export_user();

        let back = Project::from_bytes(&project.to_bytes().unwrap()).unwrap();
        assert_eq!(back.voices.len(), 1);
        assert_eq!(back.voices[0].name, "我的音色");

        let restored = VoiceLibrary::with_user_voices(&back.voices);
        assert_eq!(restored.name(id), "我的音色");
    }

    #[test]
    fn extension_is_added_only_when_missing() {
        let bare = std::path::Path::new("song");
        assert_eq!(with_extension(bare), std::path::PathBuf::from("song.sonora"));

        let named = std::path::Path::new("song.sonora");
        assert_eq!(with_extension(named), std::path::PathBuf::from("song.sonora"));
    }

    #[test]
    fn display_name_falls_back() {
        let mut p = Project::empty();
        p.title = "   ".into();
        assert_eq!(p.display_name(), "未命名工程");
        p.title = "真正名字".into();
        assert_eq!(p.display_name(), "真正名字");
    }
}
