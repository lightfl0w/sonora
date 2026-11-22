pub mod cli;
pub mod wav;

use std::path::Path;
use std::sync::Arc;

use sonora_core::pitch_name;
use sonora_engine::VoiceLibrary;
use sonora_engine::render_offline;
use sonora_project::{Project, ProjectError};

#[derive(Debug)]
pub enum AppError {
    Project(ProjectError),
    Wav(std::io::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Project(e) => write!(f, "{e}"),
            AppError::Wav(e) => write!(f, "写出 WAV 失败：{e}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<ProjectError> for AppError {
    fn from(e: ProjectError) -> Self {
        AppError::Project(e)
    }
}

pub fn load_project(path: &Path) -> Result<Project, AppError> {
    Ok(Project::load(path)?)
}

pub fn library_for(project: &Project) -> VoiceLibrary {
    VoiceLibrary::with_user_voices(&project.voices)
}

pub fn render_project(project: &Project, sample_rate: u32) -> (Vec<f32>, Vec<f32>) {
    let lib = Arc::new(library_for(project));
    render_offline(&project.song, sample_rate, lib)
}

pub fn render_to_wav(project: &Project, out: &Path, sample_rate: u32) -> Result<usize, AppError> {
    let (left, right) = render_project(project, sample_rate);
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(AppError::Wav)?;
        }
    }
    wav::write_wav(out, &left, &right, sample_rate).map_err(AppError::Wav)?;
    Ok(left.len())
}

pub fn summary(project: &Project) -> String {
    use std::fmt::Write;

    let song = &project.song;
    let mut out = String::new();
    let _ = writeln!(out, "工程：{}", project.display_name());
    let _ = writeln!(out, "音阶：{}", project.scale.name());
    let _ = writeln!(
        out,
        "时长：{:.1} 秒（{} 小节，{} BPM）",
        song.duration_secs(),
        song.bars,
        song.bpm
    );
    let _ = writeln!(
        out,
        "轨道：{}　音符：{}",
        song.tracks.len(),
        song.total_notes()
    );
    let _ = writeln!(out, "用户音色：{}", project.voices.len());
    let _ = writeln!(out, "主音量：{:.0}%", song.master * 100.0);
    let _ = writeln!(
        out,
        "混响：尺寸 {:.0}%　阻尼 {:.0}%　湿度 {:.0}%",
        song.reverb_size * 100.0,
        song.reverb_damp * 100.0,
        song.reverb_wet * 100.0
    );
    let _ = writeln!(out);

    for (i, track) in song.tracks.iter().enumerate() {
        let mut flags = Vec::new();
        if track.muted {
            flags.push("静音");
        }
        if track.solo {
            flags.push("独奏");
        }
        if track.drums {
            flags.push("鼓");
        }
        let flags = if flags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", flags.join("/"))
        };

        let _ = writeln!(
            out,
            "  {}. {}{}　{:.0}%　{} 个音符",
            i + 1,
            track_label(track),
            flags,
            track.level * 100.0,
            track.notes.len()
        );
    }

    if let Some((lo, hi)) = pitch_range(project) {
        let _ = writeln!(out, "音域：{} – {}", pitch_name(lo), pitch_name(hi));
    }

    out
}

fn track_label(track: &sonora_core::Track) -> String {
    if track.name.trim().is_empty() {
        if track.drums {
            "鼓组".to_string()
        } else {
            "未命名轨道".to_string()
        }
    } else {
        track.name.clone()
    }
}

fn pitch_range(project: &Project) -> Option<(u8, u8)> {
    let mut lo: Option<u8> = None;
    let mut hi: Option<u8> = None;

    for note in project.song.tracks.iter().flat_map(|t| t.notes.iter()) {
        lo = Some(lo.map_or(note.midi, |v: u8| v.min(note.midi)));
        hi = Some(hi.map_or(note.midi, |v: u8| v.max(note.midi)));
    }

    match (lo, hi) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_core::Scale;
    use sonora_core::scale::Mode;
    use sonora_core::song::{Note, Track};
    use sonora_core::voice::builtin;

    fn demo() -> Project {
        let mut song = sonora_core::Song::empty();
        song.bpm = 100.0;
        song.bars = 4;
        song.tracks[0].name = "主旋律".into();
        for bar in 1..=4 {
            song.tracks[0].notes.push(Note {
                bar,
                beat: 1.0,
                midi: 72,
                len: 1.0,
                vel: 0.8,
            });
        }
        song.tracks.push(Track::new(builtin::BASS, false, 0.7));
        for bar in 1..=4 {
            song.tracks[1].notes.push(Note {
                bar,
                beat: 1.0,
                midi: 40,
                len: 2.0,
                vel: 0.9,
            });
        }
        Project::with_song(song, Scale::new(0, Mode::Major))
    }

    #[test]
    fn summary_lists_every_track() {
        let text = summary(&demo());
        assert!(text.contains("主旋律"));
        assert!(text.contains("100 BPM"));
        assert!(text.contains("2 轨道") || text.contains("轨道：2"));
        assert!(text.contains("C5") && text.contains("E2"));
    }

    #[test]
    fn summary_marks_mute_and_solo() {
        let mut project = demo();
        project.song.tracks[0].muted = true;
        project.song.tracks[1].solo = true;

        let text = summary(&project);
        assert!(text.contains("静音"));
        assert!(text.contains("独奏"));
    }

    #[test]
    fn summary_handles_empty_project() {
        let text = summary(&Project::empty());
        assert!(text.contains("0"));
        assert!(!text.contains("音域"));
    }

    #[test]
    fn render_produces_audio_with_signal() {
        let project = demo();
        let (left, right) = render_project(&project, 44100);

        assert!(!left.is_empty());
        assert_eq!(left.len(), right.len());

        let peak = left
            .iter()
            .chain(right.iter())
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "渲染结果应有实际声音，实际峰值 {peak}");
    }

    #[test]
    fn render_to_wav_writes_file() {
        let dir = std::env::temp_dir().join("sonora-app-render");
        let _ = std::fs::remove_dir_all(&dir);
        let out = dir.join("demo.wav");

        let frames = render_to_wav(&demo(), &out, 22050).unwrap();
        assert!(frames > 0);

        let data = std::fs::read(&out).unwrap();
        assert_eq!(&data[0..4], b"RIFF");
        assert!(data.len() > 44);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn library_matches_project_voices() {
        let mut project = demo();
        let mut lib = VoiceLibrary::builtins();
        let id = lib.create_from(builtin::HARP, "自建".into());
        project.voices = lib.export_user();

        let restored = library_for(&project);
        assert_eq!(restored.name(id), "自建");
    }

    #[test]
    fn load_project_reports_errors() {
        let missing = Path::new("/nonexistent/nope.sonora");
        assert!(load_project(missing).is_err());
    }

    #[test]
    fn silent_project_still_renders_silence() {
        let project = Project::empty();
        let (left, _) = render_project(&project, 8000);
        let peak = left.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak < 0.01, "空工程应接近静音，实际 {peak}");
    }
}
