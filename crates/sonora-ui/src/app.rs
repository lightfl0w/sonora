use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::history::{DEFAULT_LIMIT, History};
use crate::layout::{HIGH_PITCH, LOW_PITCH};
use crate::piano_roll::{RollAction, RollState};
use crate::theme::{self, ThemeMode};
use crate::titlebar::{Menu, title_bar};
use crate::track_list::{TrackAction, TrackView, track_list};
use crate::transport::TransportState;
use crate::voice_panel::{PartialField, VoiceAction, VoicePanelState, VoiceView, voice_panel};
use crate::waveform::{WaveformState, waveform};
use gpui::{
    App, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div, px,
    rgb,
};
use sonora_audio::AudioEngine;
use sonora_core::{Note, Song, Track, VoiceId, pitch_name};
use sonora_engine::{Command, MAX_PARTIALS, Param, Partial, VoiceLibrary, VoicePreset};
use sonora_project::Project;

pub struct SonoraApp {
    project: Project,
    library: VoiceLibrary,
    engine: Option<AudioEngine>,
    path: Option<PathBuf>,
    selected_track: usize,
    zoom: f32,
    scroll_bars: f32,
    pitch_scroll: usize,
    snap_grid: f32,
    theme_mode: ThemeMode,
    open_menu: Option<Menu>,
    status: String,
    dirty: bool,
    last_bar: u32,
    last_playing: bool,
    last_voices: u32,
    playhead_bars: f32,
    ticker_started: bool,
    want_playing: bool,
    row_h: f32,
    track_row_h: f32,
    drag_mode: Option<crate::piano_roll::DragMode>,
    focus: Option<gpui::FocusHandle>,
    focus_requested: bool,
    track_renaming: Option<usize>,
    rename_buffer: String,
    add_menu_open: bool,
    track_voice_pick: Option<usize>,
    snap_to_scale: bool,
    grid_beats: f32,
    roll_view_h: Rc<Cell<f32>>,
    roll_canvas_rect: Rc<Cell<(f32, f32, f32)>>,
    pitch_center_pending: bool,
    open_dropdown: Option<crate::toolbar::Dropdown>,
    voice_panel_open: bool,
    voice_edit: Option<VoiceId>,
    voice_draft: Option<VoicePreset>,
    history: History,
    wave_sidebar_open: bool,
    scope_buf: Vec<f32>,
    voices_path: Option<PathBuf>,
    play_requested_at: Option<std::time::Instant>,
    follow_playhead: bool,
    bpm_editing: bool,
    bpm_buffer: String,
    prompt: Option<crate::file_dialog::PathPromptState>,
    exit_prompt: bool,
    close_hook_registered: bool,
    last_autosave: std::time::Instant,
    projects_dir: Option<PathBuf>,
    selected_notes: Vec<usize>,
    note_clipboard: Vec<Note>,
    metronome_on: bool,
    count_in_on: bool,
    marquee_anchor: Option<(f32, f32)>,
    marquee_base: Vec<usize>,
    marquee_rect: Option<(f32, f32, f32, f32)>,
}

impl SonoraApp {
    pub fn new(project: Project, cx: &mut Context<Self>) -> Self {
        let mut app = Self::bare(project);
        app.focus = Some(cx.focus_handle());
        app
    }
    pub fn bare(project: Project) -> Self {
        let library = VoiceLibrary::with_user_voices(&project.voices);
        Self {
            project,
            library,
            engine: None,
            path: None,
            selected_track: 0,
            zoom: 2.5,
            scroll_bars: 0.0,
            pitch_scroll: crate::piano_roll::default_pitch_scroll(false, 600.0, 26.0),
            snap_grid: 0.25,
            theme_mode: ThemeMode::Light,
            open_menu: None,
            status: String::from("就绪"),
            dirty: false,
            last_bar: 1,
            last_playing: false,
            last_voices: 0,
            playhead_bars: 0.0,
            ticker_started: false,
            want_playing: false,
            row_h: 26.0,
            track_row_h: 58.0,
            drag_mode: None,
            focus: None,
            focus_requested: false,
            track_renaming: None,
            rename_buffer: String::new(),
            add_menu_open: false,
            track_voice_pick: None,
            snap_to_scale: false,
            grid_beats: 0.25,
            roll_view_h: Rc::new(Cell::new(0.0)),
            roll_canvas_rect: Rc::new(Cell::new((0.0, 0.0, 0.0))),
            pitch_center_pending: true,
            open_dropdown: None,
            voice_panel_open: false,
            voice_edit: None,
            voice_draft: None,
            history: History::new(DEFAULT_LIMIT),
            wave_sidebar_open: true,
            scope_buf: Vec::new(),
            voices_path: None,
            play_requested_at: None,
            follow_playhead: true,
            bpm_editing: false,
            bpm_buffer: String::new(),
            prompt: None,
            exit_prompt: false,
            close_hook_registered: false,
            last_autosave: std::time::Instant::now(),
            projects_dir: None,
            selected_notes: Vec::new(),
            note_clipboard: Vec::new(),
            metronome_on: false,
            count_in_on: false,
            marquee_anchor: None,
            marquee_base: Vec::new(),
            marquee_rect: None,
        }
    }
    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker_started {
            return;
        }
        self.ticker_started = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let alive = this.update(cx, |app: &mut SonoraApp, cx| {
                    app.sync_transport();
                    app.maybe_autosave();
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }
    pub fn enable_voice_persistence(&mut self, path: PathBuf) {
        let stored = VoiceLibrary::load(&path).export_user();
        self.library.merge_user_voices(&stored);
        self.voices_path = Some(path);
        self.status = format!("音色库已就绪：{} 个用户音色", self.user_voice_count());
        self.publish_voices();
    }
    pub fn enable_default_voice_persistence(&mut self) {
        self.enable_voice_persistence(VoiceLibrary::default_path());
    }
    pub fn with_audio(mut self, engine: AudioEngine) -> Self {
        self.engine = Some(engine);
        self
    }
    pub fn start_audio(&mut self) {
        match AudioEngine::start(self.project.song.clone(), self.library.clone()) {
            Ok(engine) => {
                engine.send(Command::Set(Param::Bpm(self.project.song.bpm)));
                self.status = format!("音频已就绪：{}", engine.device_name);
                self.engine = Some(engine);
            }
            Err(e) => {
                self.status = format!("音频不可用：{e}");
                self.engine = None;
            }
        }
        self.sync_loop();
        self.sync_metronome();
    }
    fn song(&self) -> &Song {
        &self.project.song
    }
    fn song_mut(&mut self) -> &mut Song {
        &mut self.project.song
    }
    fn send(&self, cmd: Command) {
        if let Some(engine) = &self.engine {
            engine.send(cmd);
        }
    }
    fn reload(&mut self) {
        let song = self.project.song.clone();
        self.send(Command::Reload(song));
    }
    fn checkpoint(&mut self, key: Option<&str>) {
        if self.history.in_transaction() && self.drag_mode.is_none() {
            self.history.end();
        }
        let before = self.project.clone();
        self.history.record(before, key);
    }
    fn begin_history_txn(&mut self) {
        if self.history.in_transaction() {
            return;
        }
        let before = self.project.clone();
        self.history.begin(before);
    }
    fn end_history_txn(&mut self) {
        self.history.end();
    }
    fn undo(&mut self) {
        self.selected_notes.clear();
        let current = self.project.clone();
        match self.history.undo(current) {
            Some(prev) => {
                self.restore_project(prev);
                self.status = "已撤销".into();
            }
            None => self.status = "没有可撤销的操作".into(),
        }
    }
    fn save_project(&mut self) {
        self.project.voices = self.library.export_user();
        self.persist_voice_file();
        let path = match self.path.clone() {
            Some(path) => path,
            None => {
                let dir = self
                    .projects_dir
                    .clone()
                    .unwrap_or_else(Project::default_dir);
                let path = dir.join(Project::file_name_for(&self.project.title));
                self.path = Some(path.clone());
                path
            }
        };
        match self.project.save(&path) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("已保存到 {}", path.display());
            }
            Err(e) => self.status = format!("保存失败：{e}"),
        }
    }
    fn save_project_to(&mut self, path: std::path::PathBuf, rename: bool) {
        self.project.voices = self.library.export_user();
        self.persist_voice_file();
        match self.project.save(&path) {
            Ok(()) => {
                if rename {
                    if let Some(stem) = path.file_stem() {
                        let stem = stem.to_string_lossy().to_string();
                        if !stem.trim().is_empty() {
                            self.project.title = stem;
                        }
                    }
                }
                self.path = Some(path.clone());
                self.dirty = false;
                self.status = format!("已保存到 {}", path.display());
            }
            Err(e) => self.status = format!("保存失败：{e}"),
        }
    }
    fn default_save_path(&self) -> std::path::PathBuf {
        self.path.clone().unwrap_or_else(|| {
            let dir = self
                .projects_dir
                .clone()
                .unwrap_or_else(Project::default_dir);
            dir.join(Project::file_name_for(&self.project.title))
        })
    }
    fn project_candidates(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(dir) = &self.projects_dir {
            dirs.push(dir.clone());
        }
        dirs.push(Project::default_dir());
        if let Some(current) = &self.path {
            if let Some(parent) = current.parent() {
                dirs.push(parent.to_path_buf());
            }
        }
        let mut out: Vec<PathBuf> = Vec::new();
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let is_project = path
                    .extension()
                    .map(|ext| ext == sonora_project::EXTENSION)
                    .unwrap_or(false);
                if is_project && !out.contains(&path) {
                    out.push(path);
                }
            }
        }
        out.sort();
        out.truncate(12);
        out
    }
    fn open_path_prompt(&mut self, kind: crate::file_dialog::PathPromptKind) {
        use crate::file_dialog::PathPromptKind;
        let dir = self
            .projects_dir
            .clone()
            .unwrap_or_else(Project::default_dir);
        let stem = self.project.display_name();
        let buffer = match kind {
            PathPromptKind::SaveAs => self.default_save_path().display().to_string(),
            PathPromptKind::Open | PathPromptKind::ImportMidi => format!("{}/", dir.display()),
            PathPromptKind::ExportMidi => dir.join(format!("{stem}.mid")).display().to_string(),
            PathPromptKind::ExportWav => dir.join(format!("{stem}.wav")).display().to_string(),
        };
        self.prompt = Some(crate::file_dialog::PathPromptState {
            kind,
            buffer,
            candidates: self.project_candidates(),
            warning: match (kind, self.dirty) {
                (PathPromptKind::Open | PathPromptKind::ImportMidi, true) => {
                    Some("当前工程有未保存的改动，打开其它工程会丢弃这些改动".to_string())
                }
                _ => None,
            },
        });
        self.status = match kind {
            PathPromptKind::SaveAs => "选择保存位置，回车确认，Esc 取消".into(),
            PathPromptKind::Open => "输入或选择一个工程文件，回车打开，Esc 取消".into(),
            PathPromptKind::ImportMidi => "输入 .mid/.midi 路径，回车导入（会替换当前工程）".into(),
            PathPromptKind::ExportMidi => "输入导出的 .mid 路径，回车导出".into(),
            PathPromptKind::ExportWav => "输入导出的 .wav 路径，回车导出".into(),
        };
    }
    fn cancel_path_prompt(&mut self) {
        self.prompt = None;
        self.status = "已取消".into();
    }
    fn pick_project_candidate(&mut self, index: usize) {
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        if let Some(path) = prompt.candidates.get(index) {
            prompt.buffer = path.display().to_string();
        }
    }
    fn commit_path_prompt(&mut self) {
        use crate::file_dialog::PathPromptKind;
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        let text = prompt.buffer.trim().to_string();
        if text.is_empty() {
            self.status = "路径为空，已取消".into();
            return;
        }
        let raw = std::path::Path::new(&text);
        let path = match prompt.kind {
            PathPromptKind::SaveAs | PathPromptKind::Open => sonora_project::with_extension(raw),
            PathPromptKind::ImportMidi => raw.to_path_buf(),
            PathPromptKind::ExportMidi => with_extension_unless(raw, "mid"),
            PathPromptKind::ExportWav => with_extension_unless(raw, "wav"),
        };
        match prompt.kind {
            PathPromptKind::SaveAs => self.save_project_to(path, true),
            PathPromptKind::Open => self.open_project(path),
            PathPromptKind::ImportMidi => self.import_midi(path),
            PathPromptKind::ExportMidi => self.export_midi(path),
            PathPromptKind::ExportWav => self.export_wav(path),
        }
    }
    fn handle_path_prompt_key(&mut self, keystroke: &gpui::Keystroke) {
        match keystroke.key.as_str() {
            "enter" => {
                self.commit_path_prompt();
                return;
            }
            "escape" => {
                self.cancel_path_prompt();
                return;
            }
            "backspace" => {
                if let Some(prompt) = self.prompt.as_mut() {
                    prompt.buffer.pop();
                }
                return;
            }
            _ => {}
        }
        if keystroke.modifiers.control
            || keystroke.modifiers.alt
            || keystroke.modifiers.platform
            || keystroke.modifiers.function
        {
            return;
        }
        let Some(typed) = keystroke.key_char.as_deref() else {
            return;
        };
        if typed.chars().count() != 1 {
            return;
        }
        if let Some(prompt) = self.prompt.as_mut() {
            for ch in typed.chars() {
                if !ch.is_control() && prompt.buffer.chars().count() < 240 {
                    prompt.buffer.push(ch);
                }
            }
        }
    }
    fn import_midi(&mut self, path: PathBuf) {
        match sonora_midi::read_file(&path) {
            Ok(file) => {
                let notes = file.notes.len();
                let song = sonora_midi::to_song(&file);
                self.checkpoint(None);
                self.set_song(song);
                self.dirty = true;
                self.status = format!(
                    "已导入 {}（{} 个音符，{} 小节）",
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    notes,
                    self.song().bars
                );
            }
            Err(e) => self.status = format!("导入 MIDI 失败：{e}"),
        }
    }
    fn export_midi(&mut self, path: PathBuf) {
        match sonora_midi::write_file(&path, &self.project.song) {
            Ok(()) => {
                self.status = format!(
                    "已导出 MIDI 到 {}（{} 个音符）",
                    path.display(),
                    self.song().total_notes()
                )
            }
            Err(e) => self.status = format!("导出 MIDI 失败：{e}"),
        }
    }
    fn export_wav(&mut self, path: PathBuf) {
        let song = self.project.song.clone();
        let lib = std::sync::Arc::new(self.library.clone());
        self.status = "正在离线渲染…".into();
        match sonora_engine::render_offline(&song, 44100, lib) {
            (l, r) => match write_wav_simple(&path, &l, &r, 44100) {
                Ok(()) => {
                    self.status = format!(
                        "已导出 {} 秒到 {}",
                        l.len() as f32 / 44100.0,
                        path.display()
                    )
                }
                Err(e) => self.status = format!("写入失败：{e}"),
            },
        }
    }
    fn open_project(&mut self, path: PathBuf) {
        match Project::load(&path) {
            Ok(project) => {
                self.project = project;
                self.library = VoiceLibrary::with_user_voices(&self.project.voices);
                if let Some(voices_path) = self.voices_path.clone() {
                    let stored = VoiceLibrary::load(&voices_path).export_user();
                    self.library.merge_user_voices(&stored);
                }
                self.path = Some(path.clone());
                self.selected_track = 0;
                self.scroll_bars = 0.0;
                self.playhead_bars = 0.0;
                self.pitch_center_pending = true;
                self.want_playing = false;
                self.play_requested_at = None;
                self.follow_playhead = true;
                self.drag_mode = None;
                self.history.clear();
                self.track_renaming = None;
                self.rename_buffer.clear();
                self.track_voice_pick = None;
                self.add_menu_open = false;
                self.voice_panel_open = false;
                self.voice_edit = None;
                self.voice_draft = None;
                self.bpm_editing = false;
                self.bpm_buffer.clear();
                self.selected_notes.clear();
                self.dirty = false;
                self.send(Command::Stop);
                let lib = self.library.clone();
                self.send(Command::UpdateVoices(lib));
                let song = self.project.song.clone();
                self.send(Command::Reload(song));
                self.sync_loop();
                self.status = format!("已打开 {}", path.display());
            }
            Err(e) => self.status = format!("打开失败：{e}"),
        }
    }
    fn on_close_requested(&mut self) -> bool {
        if !self.dirty {
            self.exit_prompt = false;
            return true;
        }
        self.exit_prompt = true;
        false
    }
    fn apply_exit_action(&mut self, action: crate::file_dialog::ExitAction) -> bool {
        use crate::file_dialog::ExitAction;
        match action {
            ExitAction::Cancel => {
                self.exit_prompt = false;
                self.status = "已取消退出".into();
                false
            }
            ExitAction::DiscardAndExit => {
                self.exit_prompt = false;
                true
            }
            ExitAction::SaveAndExit => {
                self.save_project();
                if self.dirty {
                    self.status = format!("{}（未退出）", self.status);
                    false
                } else {
                    self.exit_prompt = false;
                    true
                }
            }
        }
    }
    fn autosave_path(&self) -> std::path::PathBuf {
        let base = self.default_save_path();
        let stem = base
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名工程".into());
        base.with_file_name(format!("{stem}.autosave.{}", sonora_project::EXTENSION))
    }
    fn maybe_autosave(&mut self) {
        if !self.dirty {
            return;
        }
        if self.last_autosave.elapsed() < AUTOSAVE_INTERVAL {
            return;
        }
        self.last_autosave = std::time::Instant::now();
        let path = self.autosave_path();
        self.project.voices = self.library.export_user();
        match self.project.save(&path) {
            Ok(()) => self.status = format!("已自动备份到 {}", path.display()),
            Err(e) => self.status = format!("自动备份失败：{e}"),
        }
    }
    fn redo(&mut self) {
        self.selected_notes.clear();
        let current = self.project.clone();
        match self.history.redo(current) {
            Some(next) => {
                self.restore_project(next);
                self.status = "已重做".into();
            }
            None => self.status = "没有可重做的操作".into(),
        }
    }
    fn restore_project(&mut self, project: Project) {
        self.drag_mode = None;
        self.history.end();
        self.project = project;
        self.library = VoiceLibrary::with_user_voices(&self.project.voices);
        self.selected_track = self
            .selected_track
            .min(self.song().tracks.len().saturating_sub(1));
        self.track_renaming = None;
        self.rename_buffer.clear();
        self.track_voice_pick = None;
        self.voice_edit = self.voice_edit.filter(|id| self.library.get(*id).is_some());
        self.voice_draft = self.voice_edit.map(|id| self.library.preset(id));
        self.dirty = true;
        let lib = self.library.clone();
        self.send(Command::UpdateVoices(lib));
        self.persist_voice_file();
        self.sync_loop();
        self.reload();
    }
    fn sync_loop(&self) {
        let range = if self.project.loop_enabled {
            self.project.loop_bars
        } else {
            None
        };
        self.send(Command::SetLoop(range));
    }
    fn sync_metronome(&self) {
        self.send(Command::SetMetronome(self.metronome_on));
        self.send(Command::SetCountIn(if self.count_in_on { 4 } else { 0 }));
    }
    fn toggle_loop(&mut self) {
        if self.project.loop_bars.is_none() {
            let bars = self.song().bars.max(1) as f32;
            self.project.loop_bars = Some((0.0, bars.min(4.0)));
        }
        self.project.loop_enabled = !self.project.loop_enabled;
        self.sync_loop();
        self.status = match (self.project.loop_enabled, self.project.loop_bars) {
            (true, Some((a, b))) => format!("循环已开启：{} → {} 小节", a + 1.0, b + 1.0),
            (false, _) => "循环已关闭".into(),
            _ => "循环区间为空".into(),
        };
    }
    fn toggle_play(&mut self) {
        if self.engine.is_none() {
            self.start_audio();
        }
        if self.engine.is_none() {
            return;
        }
        self.want_playing = !self.want_playing;
        if self.want_playing {
            self.play_requested_at = Some(std::time::Instant::now());
            self.follow_playhead = true;
            self.send(Command::Play);
            self.status = "播放中".into();
        } else {
            self.play_requested_at = None;
            self.send(Command::Stop);
            self.status = "已暂停".into();
        }
        self.last_playing = self.want_playing;
    }
    fn stop(&mut self) {
        self.send(Command::Stop);
        self.send(Command::Seek(0.0));
        self.want_playing = false;
        self.play_requested_at = None;
        self.playhead_bars = 0.0;
        self.scroll_bars = 0.0;
        self.follow_playhead = true;
        self.status = "已停止".into();
    }
    fn rewind(&mut self) {
        self.seek_bar(1);
        self.scroll_bars = 0.0;
        self.playhead_bars = 0.0;
    }
    fn run_menu_action(&mut self, action: &str, cx: &mut Context<Self>) {
        match action {
            "new-empty" => {
                self.checkpoint(None);
                self.set_song(Song::empty());
                self.status = "已新建空白工程".into();
            }
            "load-demo" => {
                self.checkpoint(None);
                self.set_song(Song::example());
                self.status = "已载入示例工程".into();
            }
            "save" => {
                self.save_project();
            }
            "save-as" => self.open_path_prompt(crate::file_dialog::PathPromptKind::SaveAs),
            "open" => self.open_path_prompt(crate::file_dialog::PathPromptKind::Open),
            "export-wav" => self.open_path_prompt(crate::file_dialog::PathPromptKind::ExportWav),
            "export-midi" => self.open_path_prompt(crate::file_dialog::PathPromptKind::ExportMidi),
            "import-midi" => self.open_path_prompt(crate::file_dialog::PathPromptKind::ImportMidi),
            "toggle-theme" => {
                self.theme_mode = self.theme_mode.toggled();
                theme::set_mode(self.theme_mode);
            }
            "play-pause" => self.toggle_play(),
            "stop" => {
                self.stop();
            }
            "rewind" => self.rewind(),
            "menu-loop" => self.toggle_loop(),
            "menu-metronome" => {
                self.metronome_on = !self.metronome_on;
                self.sync_metronome();
                self.status = if self.metronome_on {
                    "节拍器已开启".into()
                } else {
                    "节拍器已关闭".into()
                };
            }
            "menu-countin" => {
                self.count_in_on = !self.count_in_on;
                self.sync_metronome();
                self.status = if self.count_in_on {
                    "预卷已开启：播放前先数一小节".into()
                } else {
                    "预卷已关闭".into()
                };
            }
            "toggle-wave" => {
                self.wave_sidebar_open = !self.wave_sidebar_open;
                self.status = if self.wave_sidebar_open {
                    "波形侧栏已展开".into()
                } else {
                    "波形侧栏已收起".into()
                };
            }
            "toggle-voices" => {
                if self.voice_panel_open {
                    self.close_voice_panel();
                    self.status = "音色管理已收起".into();
                } else {
                    self.open_voice_panel();
                    self.status = "音色管理已展开".into();
                }
            }
            "undo" => self.undo(),
            "redo" => self.redo(),
            "about" => {
                self.status = format!("Sonora {}", env!("CARGO_PKG_VERSION"));
            }
            other => {
                self.status = format!("未实现：{other}");
            }
        }
        cx.notify();
    }
    fn set_song(&mut self, song: Song) {
        self.project.song = song;
        self.selected_track = 0;
        self.selected_notes.clear();
        self.scroll_bars = 0.0;
        self.playhead_bars = 0.0;
        self.pitch_center_pending = true;
        self.want_playing = false;
        self.dirty = false;
        self.send(Command::Stop);
        let bars = self.song().bars.max(1) as f32;
        if let Some((a, b)) = self.project.loop_bars {
            self.project.loop_bars = if b > bars || a >= bars {
                None
            } else {
                Some((a, b))
            };
        }
        self.sync_loop();
        let song = self.project.song.clone();
        self.send(Command::Reload(song));
    }
    fn seek_bar(&mut self, bar: u32) {
        let target = bar.saturating_sub(1) as f32;
        self.send(Command::Seek(target));
        self.last_bar = bar;
    }
    fn selected_track_index(&self) -> usize {
        self.selected_track
            .min(self.song().tracks.len().saturating_sub(1))
    }
    fn track_views(&self) -> Vec<TrackView> {
        self.song()
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| TrackView {
                name: if t.name.trim().is_empty() {
                    default_track_name(i, t)
                } else {
                    t.name.clone()
                },
                color: theme::palette().track_color(i),
                level: t.level,
                muted: t.muted,
                solo: t.solo,
                notes: t.notes.len(),
            })
            .collect()
    }
    fn roll_state(&self) -> RollState {
        let idx = self.selected_track_index();
        let track = self.song().tracks.get(idx);
        let notes = track
            .map(|t| {
                t.notes
                    .iter()
                    .map(|n| crate::piano_roll::NoteView {
                        bar: n.bar,
                        beat: n.beat,
                        midi: n.midi,
                        len: n.len,
                        vel: n.vel,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let drums = track.map(|t| t.drums).unwrap_or(false);
        let in_scale = if drums {
            Vec::new()
        } else {
            (LOW_PITCH..=HIGH_PITCH)
                .filter(|p| self.project.scale.contains(*p))
                .collect()
        };
        RollState {
            notes,
            selected: self.selected_notes.clone(),
            bars: self.song().bars,
            zoom: self.zoom,
            scroll: self.scroll_bars,
            pitch_scroll: self.pitch_scroll,
            row_h: self.row_h,
            grid_beats: self.snap_grid,
            playhead: self.playhead_bars,
            color: theme::palette().track_color(idx),
            drums,
            scale_root: self.project.scale.root,
            in_scale,
            snap_to_scale: self.snap_to_scale,
            drag: self.drag_mode,
            loop_bars: self.project.loop_bars,
            loop_enabled: self.project.loop_enabled,
            marquee: self.marquee_rect,
        }
    }
    #[cfg(test)]
    fn bar_width(&self) -> f32 {
        crate::layout::DEFAULT_BAR_W * self.zoom
    }
    fn max_scroll_bars(&self) -> f32 {
        let bars = self.song().bars.max(1) as f32;
        bars.max(0.0)
    }
    fn grow_to_fit(&mut self, track_idx: usize) -> bool {
        let needed = self
            .song()
            .tracks
            .get(track_idx)
            .map(|t| {
                t.notes
                    .iter()
                    .map(|n| n.bar.saturating_sub(1) + (n.len / 4.0).ceil() as u32)
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        if needed > self.song().bars {
            self.song_mut().bars = needed;
            true
        } else {
            false
        }
    }
    fn set_scroll_bars(&mut self, target: f32) {
        self.scroll_bars = if target.is_finite() {
            target.clamp(0.0, self.max_scroll_bars())
        } else {
            self.scroll_bars
        };
    }
    fn handle_roll(&mut self, action: RollAction, cx: &mut Context<Self>) {
        let idx = self.selected_track_index();
        let bars = self.song().bars;
        match action {
            RollAction::Seek(pos) => {
                let bar = pos.floor().max(0.0) as u32 + 1;
                self.send(Command::Seek(pos.max(0.0)));
                self.playhead_bars = pos.max(0.0);
                self.last_bar = bar.min(bars);
                self.follow_playhead = true;
            }
            RollAction::ScrollBars(target) => {
                self.follow_playhead = false;
                self.set_scroll_bars(target);
            }
            RollAction::ScrollPitch(step) => {
                let drums = self
                    .song()
                    .tracks
                    .get(idx)
                    .map(|t| t.drums)
                    .unwrap_or(false);
                let rows = crate::piano_roll::visible_rows(drums).len();
                let vh = self.roll_view_h.get();
                let vh = if vh > 1.0 { vh } else { 600.0 };
                let vis = crate::piano_roll::visible_count(vh, self.row_h);
                let max = rows.saturating_sub(vis);
                let next = self.pitch_scroll as isize + step;
                self.pitch_scroll = next.clamp(0, max as isize) as usize;
            }
            RollAction::Zoom { zoom, scroll } => {
                self.zoom = crate::piano_roll::clamp_zoom(zoom);
                self.set_scroll_bars(scroll);
            }
            RollAction::Create {
                bar,
                beat,
                midi,
                len,
            } => {
                let new_start = (bar.saturating_sub(1)) as f32 * 4.0 + (beat - 1.0);
                let new_end = new_start + len;
                let overlap = self.song().tracks[idx].notes.iter().any(|n| {
                    if n.midi != midi {
                        return false;
                    }
                    let s = (n.bar.saturating_sub(1)) as f32 * 4.0 + (n.beat - 1.0);
                    let e = s + n.len;
                    new_start < e - 1e-4 && s < new_end - 1e-4
                });
                if overlap {
                    self.status =
                        format!("{} 在此区间已有音符，换个位置或关闭吸附", pitch_name(midi));
                } else {
                    self.checkpoint(None);
                    let track = &mut self.song_mut().tracks[idx];
                    track.notes.push(Note {
                        bar,
                        beat,
                        midi,
                        len,
                        vel: 0.8,
                    });
                    self.dirty = true;
                    let grew = self.grow_to_fit(idx);
                    self.selected_notes = vec![self.song().tracks[idx].notes.len() - 1];
                    self.reload();
                    self.status = if grew {
                        format!(
                            "添加 {}，工程延长到 {} 小节",
                            pitch_name(midi),
                            self.song().bars
                        )
                    } else {
                        format!("添加 {}", pitch_name(midi))
                    };
                }
            }
            RollAction::Delete { index } => {
                if index < self.song().tracks[idx].notes.len() {
                    self.checkpoint(None);
                    self.song_mut().tracks[idx].notes.remove(index);
                    self.selected_notes.retain(|i| *i != index);
                    for sel in self.selected_notes.iter_mut() {
                        if *sel > index {
                            *sel -= 1;
                        }
                    }
                    self.dirty = true;
                    self.reload();
                    self.status = "删除音符".into();
                }
            }
            RollAction::Select { index, additive } => {
                if additive {
                    if let Some(pos) = self.selected_notes.iter().position(|i| *i == index) {
                        self.selected_notes.remove(pos);
                    } else {
                        self.selected_notes.push(index);
                    }
                } else if self.selected_notes.len() > 1 && self.selected_notes.contains(&index) {
                } else if self.selected_notes != vec![index] {
                    self.selected_notes = vec![index];
                }
                let count = self.selected_notes.len();
                self.status = if count > 1 {
                    format!("选中 {count} 个音符")
                } else {
                    "选中 1 个音符（Shift 多选，Delete 删除，方向键微调）".into()
                };
            }
            RollAction::ClearSelection => {
                self.selected_notes.clear();
            }
            RollAction::MarqueeStart { bar, row, additive } => {
                self.marquee_anchor = Some((bar, row));
                self.marquee_base = if additive {
                    self.selected_indices()
                } else {
                    Vec::new()
                };
                self.marquee_rect = Some((bar, row, bar, row));
                self.status = "拖出一个框来选中音符".into();
            }
            RollAction::MarqueeTo { bar, row } => {
                let Some((ab, ar)) = self.marquee_anchor else {
                    cx.notify();
                    return;
                };
                self.marquee_rect = Some((ab, ar, bar, row));
                self.selected_notes = self
                    .notes_in_rect(ab, ar, bar, row)
                    .into_iter()
                    .chain(self.marquee_base.iter().copied())
                    .collect();
                self.selected_notes.sort_unstable();
                self.selected_notes.dedup();
                let count = self.selected_notes.len();
                self.status = format!("框选：{count} 个音符");
            }
            RollAction::SetLoop { a, b } => {
                let grid = self.snap_grid.max(0.03125);
                let snap = |v: f32| crate::piano_roll::snap_beats(v * 4.0, grid) / 4.0;
                let a = snap(a.max(0.0));
                let b = snap(b.max(0.0));
                let (start, end) = if b > a { (a, b) } else { (b, a) };
                self.project.loop_bars = if (end - start) < 1e-3 {
                    None
                } else {
                    Some((start, end))
                };
                self.sync_loop();
                self.status = match self.project.loop_bars {
                    Some((s, e)) => format!(
                        "循环 {} → {} 小节（{}）",
                        (s * 4.0).round() / 4.0 + 1.0,
                        (e * 4.0).round() / 4.0 + 1.0,
                        if self.project.loop_enabled {
                            "已启用"
                        } else {
                            "未启用，按 L 打开"
                        }
                    ),
                    None => "循环区间已清除".into(),
                };
            }
            RollAction::Move {
                index,
                bar,
                beat,
                midi,
            } => {
                if self.selected_notes.len() > 1 && self.selected_notes.contains(&index) {
                    let note = self.song().tracks[idx].notes.get(index).copied();
                    if let Some(note) = note {
                        let target_start = (bar.saturating_sub(1)) as f32 * 4.0 + (beat - 1.0);
                        let d_beats = target_start - note.start_beats();
                        let d_pitch = midi as i32 - note.midi as i32;
                        if d_beats.abs() > 1e-6 || d_pitch != 0 {
                            self.transform_selection_inner(d_beats, d_pitch, Some(index), true);
                        }
                    }
                    cx.notify();
                    return;
                }
                let track_notes = self.song().tracks[idx].notes.clone();
                let Some(note) = track_notes.get(index) else {
                    cx.notify();
                    return;
                };
                let new_start = (bar.saturating_sub(1)) as f32 * 4.0 + (beat - 1.0);
                let new_end = new_start + note.len;
                let blocked = track_notes.iter().enumerate().any(|(i, n)| {
                    i != index && n.midi == midi && {
                        let s = (n.bar.saturating_sub(1)) as f32 * 4.0 + (n.beat - 1.0);
                        let e = s + n.len;
                        new_start < e - 1e-4 && s < new_end - 1e-4
                    }
                });
                if blocked {
                    cx.notify();
                    return;
                }
                self.checkpoint(None);
                if let Some(n) = self.song_mut().tracks[idx].notes.get_mut(index) {
                    n.bar = bar;
                    n.beat = beat;
                    n.midi = midi;
                    self.dirty = true;
                }
                self.grow_to_fit(idx);
            }
            RollAction::Resize { index, len } => {
                let grid = self.snap_grid;
                let track_notes = self.song().tracks[idx].notes.clone();
                let Some(note) = track_notes.get(index) else {
                    cx.notify();
                    return;
                };
                let start = note.start_beats();
                let mut limit = f32::INFINITY;
                for (i, n) in track_notes.iter().enumerate() {
                    if i == index || n.midi != note.midi {
                        continue;
                    }
                    let s = n.start_beats();
                    if s > start + 1e-4 {
                        limit = limit.min(s - start);
                    }
                }
                let len = len.max(grid).min(limit.max(grid));
                self.checkpoint(None);
                if let Some(n) = self.song_mut().tracks[idx].notes.get_mut(index) {
                    n.len = len;
                    self.dirty = true;
                }
                self.grow_to_fit(idx);
            }
            RollAction::DragStart(mode) => {
                match mode {
                    crate::piano_roll::DragMode::MoveNote { .. }
                    | crate::piano_roll::DragMode::ResizeNote { .. } => self.begin_history_txn(),
                    crate::piano_roll::DragMode::DrawNote { .. } => {
                        self.history.begin_after_change()
                    }
                    _ => {}
                }
                self.drag_mode = Some(mode);
            }
            RollAction::DragEnd => {
                if matches!(
                    self.drag_mode,
                    Some(crate::piano_roll::DragMode::MoveNote { .. })
                        | Some(crate::piano_roll::DragMode::ResizeNote { .. })
                        | Some(crate::piano_roll::DragMode::DrawNote { .. })
                ) {
                    self.reload();
                }
                if self.history.in_transaction() {
                    self.end_history_txn();
                }
                self.marquee_anchor = None;
                self.marquee_rect = None;
                self.drag_mode = None;
            }
        }
        cx.notify();
    }
    fn selected_indices(&self) -> Vec<usize> {
        let idx = self.selected_track_index();
        let len = self
            .song()
            .tracks
            .get(idx)
            .map(|t| t.notes.len())
            .unwrap_or(0);
        let mut sel: Vec<usize> = self
            .selected_notes
            .iter()
            .copied()
            .filter(|i| *i < len)
            .collect();
        sel.sort_unstable();
        sel.dedup();
        sel
    }
    fn select_all_notes(&mut self) {
        let idx = self.selected_track_index();
        let len = self.song().tracks[idx].notes.len();
        self.selected_notes = (0..len).collect();
        self.status = if len == 0 {
            "这条轨道还没有音符".into()
        } else {
            format!("选中 {len} 个音符")
        };
    }
    fn delete_selection(&mut self) {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        self.checkpoint(None);
        let idx = self.selected_track_index();
        let notes = &mut self.song_mut().tracks[idx].notes;
        for i in sel.iter().rev() {
            notes.remove(*i);
        }
        self.selected_notes.clear();
        self.dirty = true;
        self.reload();
        self.status = format!("删除 {} 个音符", sel.len());
    }
    fn transform_selection(&mut self, delta_beats: f32, delta_pitch: i32) {
        self.transform_selection_inner(delta_beats, delta_pitch, None, false);
    }
    fn transform_selection_inner(
        &mut self,
        delta_beats: f32,
        delta_pitch: i32,
        priority: Option<usize>,
        live: bool,
    ) {
        let mut sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        if let Some(p) = priority {
            sel.retain(|i| *i != p);
            sel.insert(0, p);
        }
        let idx = self.selected_track_index();
        let originals: Vec<Note> = self.song().tracks[idx].notes.clone();
        let moving: Vec<bool> = (0..originals.len()).map(|i| sel.contains(&i)).collect();
        let mut accepted: Vec<(usize, Note)> = Vec::new();
        let mut skipped = 0;
        for i in sel.iter().copied() {
            let note = originals[i];
            let start = self.snap_position(note.start_beats() + delta_beats);
            if start < -1e-4 {
                skipped += 1;
                continue;
            }
            let midi = note.midi as i32 + delta_pitch;
            if !(LOW_PITCH as i32..=HIGH_PITCH as i32).contains(&midi) {
                skipped += 1;
                continue;
            }
            let candidate = note_at_beats(start, midi as u8, note.len, note.vel);
            let blocked = originals.iter().enumerate().any(|(j, other)| {
                j != i
                    && !moving[j]
                    && other.midi == candidate.midi
                    && intervals_overlap(other, &candidate)
            }) || accepted.iter().any(|(_, placed)| {
                placed.midi == candidate.midi && intervals_overlap(placed, &candidate)
            });
            if blocked {
                skipped += 1;
                continue;
            }
            accepted.push((i, candidate));
        }
        if let Some(p) = priority {
            if !accepted.iter().any(|(i, _)| *i == p) {
                self.status = "目标位置被占用".into();
                return;
            }
        }
        if accepted.is_empty() {
            self.status = format!("{skipped} 个音符位置被占用，没有移动");
            return;
        }
        if live {
            if !self.history.in_transaction() {
                self.checkpoint(Some("note-transform"));
            }
        } else {
            self.checkpoint(Some("note-transform"));
        }
        let notes = &mut self.song_mut().tracks[idx].notes;
        for (i, placed) in accepted.iter() {
            notes[*i] = *placed;
        }
        self.dirty = true;
        self.grow_to_fit(idx);
        if !live {
            self.reload();
        }
        self.status = if skipped > 0 {
            format!(
                "移动 {} 个音符，跳过 {skipped} 个（位置被占用）",
                accepted.len()
            )
        } else {
            format!("移动 {} 个音符", accepted.len())
        };
    }
    fn duplicate_selection(&mut self) {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        let idx = self.selected_track_index();
        let originals: Vec<Note> = self.song().tracks[idx].notes.clone();
        let first = sel
            .iter()
            .map(|i| originals[*i].start_beats())
            .fold(f32::INFINITY, f32::min);
        let last = sel
            .iter()
            .map(|i| originals[*i].start_beats() + originals[*i].len)
            .fold(0.0f32, f32::max);
        let span = self.snap_position((last - first).max(self.snap_grid));
        self.checkpoint(None);
        let base = originals.len();
        let mut copies = Vec::with_capacity(sel.len());
        for i in sel.iter().copied() {
            let n = originals[i];
            let start = self.snap_position(n.start_beats() + span);
            copies.push(note_at_beats(start, n.midi, n.len, n.vel));
        }
        let count = copies.len();
        let notes = &mut self.song_mut().tracks[idx].notes;
        for copy in copies {
            notes.push(copy);
        }
        self.selected_notes = (base..base + count).collect();
        self.dirty = true;
        self.grow_to_fit(idx);
        self.reload();
        self.status = format!("复制了 {count} 个音符");
    }
    fn set_selection_velocity(&mut self, value: f32) {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        self.checkpoint(Some("note-velocity"));
        let idx = self.selected_track_index();
        let value = value.clamp(0.05, 1.0);
        let notes = &mut self.song_mut().tracks[idx].notes;
        for i in sel {
            if let Some(n) = notes.get_mut(i) {
                n.vel = value;
            }
        }
        self.dirty = true;
        self.reload();
    }
    fn notes_in_rect(&self, bar0: f32, row0: f32, bar1: f32, row1: f32) -> Vec<usize> {
        let idx = self.selected_track_index();
        let track = &self.song().tracks[idx];
        let rows = crate::piano_roll::visible_rows(track.drums);
        let (x0, x1) = (bar0.min(bar1), bar0.max(bar1));
        let (y0, y1) = (row0.min(row1), row0.max(row1));
        track
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                let start = n.start_beats() / 4.0;
                let end = start + n.len / 4.0;
                let row = rows.iter().position(|p| *p == n.midi).map(|r| r as f32);
                let Some(row) = row else {
                    return false;
                };
                end >= x0 - 1e-4 && start <= x1 + 1e-4 && row >= y0 - 0.5 && row <= y1 + 0.5
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn snap_position(&self, beats: f32) -> f32 {
        if beats <= 0.0 {
            0.0
        } else {
            crate::piano_roll::snap_beats(beats, self.snap_grid)
        }
    }
    fn copy_selection(&mut self) {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        let idx = self.selected_track_index();
        let notes = &self.song().tracks[idx].notes;
        self.note_clipboard = sel.iter().map(|i| notes[*i]).collect();
        self.status = format!("已复制 {} 个音符", self.note_clipboard.len());
    }
    fn paste_clipboard(&mut self) {
        if self.note_clipboard.is_empty() {
            return;
        }
        let idx = self.selected_track_index();
        let first = self
            .note_clipboard
            .iter()
            .map(|n| n.start_beats())
            .fold(f32::INFINITY, f32::min);
        let anchor = self.snap_position(self.playhead_bars * 4.0);
        let mut pasted = Vec::with_capacity(self.note_clipboard.len());
        for n in self.note_clipboard.clone() {
            let start = self.snap_position(anchor + (n.start_beats() - first));
            pasted.push(note_at_beats(start, n.midi, n.len, n.vel));
        }
        self.checkpoint(None);
        let base = self.song().tracks[idx].notes.len();
        let count = pasted.len();
        self.song_mut().tracks[idx].notes.extend(pasted);
        self.selected_notes = (base..base + count).collect();
        self.dirty = true;
        self.grow_to_fit(idx);
        self.reload();
        self.status = format!("粘贴了 {count} 个音符");
    }
    fn quantize_selection(&mut self) {
        let sel = self.selected_indices();
        if sel.is_empty() {
            return;
        }
        let idx = self.selected_track_index();
        let originals: Vec<Note> = self.song().tracks[idx].notes.clone();
        let moving: Vec<bool> = (0..originals.len()).map(|i| sel.contains(&i)).collect();
        let mut accepted: Vec<(usize, Note)> = Vec::new();
        let mut skipped = 0;
        for i in sel.iter().copied() {
            let n = originals[i];
            let start = self.snap_position(n.start_beats());
            if (start - n.start_beats()).abs() < 1e-4 {
                continue;
            }
            let candidate = note_at_beats(start, n.midi, n.len, n.vel);
            let blocked = originals.iter().enumerate().any(|(j, other)| {
                j != i
                    && !moving[j]
                    && other.midi == candidate.midi
                    && intervals_overlap(other, &candidate)
            }) || accepted.iter().any(|(_, placed)| {
                placed.midi == candidate.midi && intervals_overlap(placed, &candidate)
            });
            if blocked {
                skipped += 1;
                continue;
            }
            accepted.push((i, candidate));
        }
        if accepted.is_empty() {
            self.status = if skipped > 0 {
                format!("{skipped} 个音符无法量化（位置被占用）")
            } else {
                "选中的音符已经在栅格上".into()
            };
            return;
        }
        self.checkpoint(None);
        let notes = &mut self.song_mut().tracks[idx].notes;
        for (i, placed) in accepted.iter() {
            notes[*i] = *placed;
        }
        self.dirty = true;
        self.reload();
        self.status = format!("量化了 {} 个音符", accepted.len());
    }
    fn handle_edit_key(&mut self, key: &str, secondary: bool, _shift: bool) -> bool {
        let has_selection = !self.selected_indices().is_empty();
        match key {
            "delete" | "backspace" => {
                if !has_selection {
                    return false;
                }
                self.delete_selection();
                true
            }
            "left" | "right" | "up" | "down" => {
                if !has_selection {
                    return false;
                }
                let grid = self.snap_grid.max(0.03125);
                match key {
                    "left" => self.transform_selection(-grid, 0),
                    "right" => self.transform_selection(grid, 0),
                    "up" => self.transform_selection(0.0, 1),
                    _ => self.transform_selection(0.0, -1),
                }
                true
            }
            "a" if secondary => {
                self.select_all_notes();
                true
            }
            "d" if secondary => {
                if !has_selection {
                    return false;
                }
                self.duplicate_selection();
                true
            }
            "c" if secondary => {
                if !has_selection {
                    return false;
                }
                self.copy_selection();
                true
            }
            "v" if secondary => {
                if self.note_clipboard.is_empty() {
                    return false;
                }
                self.paste_clipboard();
                true
            }
            "q" if secondary => {
                if !has_selection {
                    return false;
                }
                self.quantize_selection();
                true
            }
            "escape" => {
                self.selected_notes.clear();
                false
            }
            _ => false,
        }
    }
    fn commit_rename(&mut self) {
        let Some(i) = self.track_renaming else {
            return;
        };
        let name = self.rename_buffer.trim().to_string();
        if name.is_empty() {
            self.track_renaming = None;
            self.status = "名字为空，已取消".into();
            return;
        }
        if self.song().tracks.get(i).map(|t| t.name.clone()) != Some(name.clone()) {
            self.checkpoint(None);
            if let Some(track) = self.song_mut().tracks.get_mut(i) {
                track.name = name;
            }
            self.dirty = true;
            self.reload();
        }
        self.track_renaming = None;
        self.rename_buffer.clear();
        self.status = "已重命名轨道".into();
    }
    fn handle_rename_key(&mut self, keystroke: &gpui::Keystroke) {
        match keystroke.key.as_str() {
            "enter" => {
                self.commit_rename();
                return;
            }
            "escape" => {
                self.track_renaming = None;
                self.rename_buffer.clear();
                self.status = "已取消改名".into();
                return;
            }
            "backspace" => {
                self.rename_buffer.pop();
                return;
            }
            _ => {}
        }
        if keystroke.modifiers.control
            || keystroke.modifiers.alt
            || keystroke.modifiers.platform
            || keystroke.modifiers.function
        {
            return;
        }
        let Some(typed) = keystroke.key_char.as_deref() else {
            return;
        };
        if typed.chars().count() != 1 {
            return;
        }
        for ch in typed.chars() {
            if !ch.is_control() && self.rename_buffer.chars().count() < 32 {
                self.rename_buffer.push(ch);
            }
        }
    }
    fn handle_track(&mut self, action: TrackAction, cx: &mut Context<Self>) {
        match action {
            TrackAction::Select(i) => {
                self.selected_track = i.min(self.song().tracks.len().saturating_sub(1));
                self.track_voice_pick = None;
                self.selected_notes.clear();
            }
            TrackAction::ToggleMute(i) => {
                self.checkpoint(None);
                if let Some(t) = self.song_mut().tracks.get_mut(i) {
                    t.muted = !t.muted;
                }
                self.dirty = true;
                self.reload();
            }
            TrackAction::ToggleSolo(i) => {
                self.checkpoint(None);
                if let Some(t) = self.song_mut().tracks.get_mut(i) {
                    t.solo = !t.solo;
                }
                self.dirty = true;
                self.reload();
            }
            TrackAction::Delete(i) => {
                if self.song().tracks.len() > 1 {
                    self.checkpoint(None);
                    self.song_mut().tracks.remove(i);
                    self.selected_track = self.selected_track.min(self.song().tracks.len() - 1);
                    self.dirty = true;
                    self.reload();
                    self.status = "已删除音轨".into();
                } else {
                    self.status = "至少要保留一条音轨".into();
                }
            }
            TrackAction::Duplicate(i) => {
                if let Some(source) = self.song().tracks.get(i).cloned() {
                    self.checkpoint(None);
                    let mut copy = source;
                    copy.name = format!("{} 副本", copy.name);
                    let notes = copy.notes.len();
                    let voice = self.library.name(copy.voice);
                    self.song_mut().tracks.insert(i + 1, copy);
                    self.selected_track = i + 1;
                    self.selected_notes.clear();
                    self.dirty = true;
                    self.reload();
                    self.status = format!("已复制轨道（{voice}，{notes} 个音符）");
                }
            }
            TrackAction::Level(i, level) => {
                self.checkpoint(Some(&format!("track-level-{i}")));
                if let Some(t) = self.song_mut().tracks.get_mut(i) {
                    t.level = level.clamp(0.0, 1.0);
                }
                self.send(Command::Set(Param::TrackLevel(i, level)));
            }
            TrackAction::Rename(i) => {
                self.track_renaming = Some(i);
                self.rename_buffer = self.song().tracks[i].name.clone();
            }
            TrackAction::CommitRename => {
                if let Some(i) = self.track_renaming {
                    let name = self.rename_buffer.clone();
                    if self.song().tracks[i].name != name {
                        self.checkpoint(None);
                    }
                    if let Some(t) = self.song_mut().tracks.get_mut(i) {
                        t.name = name;
                        self.dirty = true;
                        self.reload();
                    }
                }
                self.track_renaming = None;
            }
            TrackAction::PickVoice(i) => {
                let was_open = self.track_voice_pick == Some(i);
                self.close_popups();
                if !was_open {
                    self.track_voice_pick = Some(i);
                }
            }
            TrackAction::SetVoice(i, id) => {
                let drums = id == sonora_core::voice::builtin::DRUMS;
                self.checkpoint(None);
                if let Some(t) = self.song_mut().tracks.get_mut(i) {
                    t.voice = id;
                    t.drums = drums;
                    self.dirty = true;
                    self.reload();
                    self.status = format!("音色已切换为 {}", self.library.name(id));
                }
                self.track_voice_pick = None;
            }
            TrackAction::OpenAddMenu => {
                let was_open = self.add_menu_open;
                self.close_popups();
                self.add_menu_open = !was_open;
            }
            TrackAction::AddVoice(id) => {
                let level = 0.8;
                let drums = id == sonora_core::voice::builtin::DRUMS;
                self.checkpoint(None);
                self.song_mut().tracks.push(Track::new(id, drums, level));
                self.selected_track = self.song().tracks.len() - 1;
                self.add_menu_open = false;
                self.dirty = true;
                self.reload();
                self.status = format!("已添加 {}", self.library.name(id));
            }
        }
        cx.notify();
    }
    fn close_popups(&mut self) {
        self.open_menu = None;
        self.open_dropdown = None;
        self.add_menu_open = false;
        self.track_voice_pick = None;
    }
    #[allow(dead_code)]
    fn add_track(&mut self, cx: &mut Context<Self>) {
        let voice = sonora_core::voice::builtin::PIANO;
        self.song_mut().tracks.push(Track::new(voice, false, 0.8));
        self.selected_track = self.song().tracks.len() - 1;
        self.dirty = true;
        self.reload();
        self.status = "新建轨道".into();
        cx.notify();
    }
    fn handle_toolbar(&mut self, action: crate::toolbar::ToolbarAction, cx: &mut Context<Self>) {
        self.apply_toolbar(action);
        cx.notify();
    }
    fn apply_toolbar(&mut self, action: crate::toolbar::ToolbarAction) {
        use crate::toolbar::ToolbarAction as A;
        match action {
            A::TogglePlay => self.toggle_play(),
            A::Rewind => self.rewind(),
            A::EditBpm => {
                self.bpm_editing = true;
                self.bpm_buffer = format!("{:.0}", self.song().bpm);
                self.status = "输入速度，回车确认，Esc 取消".into();
            }
            A::RowHeightSmaller => {
                self.row_h = (self.row_h - 3.0).clamp(10.0, 60.0);
                self.track_row_h = (self.track_row_h - 6.0).clamp(40.0, 140.0);
            }
            A::RowHeightBigger => {
                self.row_h = (self.row_h + 3.0).clamp(10.0, 60.0);
                self.track_row_h = (self.track_row_h + 6.0).clamp(40.0, 140.0);
            }
            A::ToggleSnap => {
                self.snap_to_scale = !self.snap_to_scale;
            }
            A::ToggleLoop => {
                self.toggle_loop();
            }
            A::ToggleDropdown(which) => {
                let was_open = self.open_dropdown == Some(which);
                self.close_popups();
                if !was_open {
                    self.open_dropdown = Some(which);
                }
            }
            A::PickGrid(index) => {
                if let Some(g) = GRID_OPTIONS.get(index) {
                    self.grid_beats = *g;
                    self.status = format!("网格 {}", grid_label(self.grid_beats));
                }
                self.open_dropdown = None;
            }
            A::PickRoot(index) => {
                self.project.scale.root = (index % 12) as u8;
                self.status = self.project.scale.name();
                self.open_dropdown = None;
            }
            A::PickMode(index) => {
                let modes = sonora_core::Mode::ALL;
                if let Some(m) = modes.get(index) {
                    self.project.scale.mode = *m;
                    self.status = self.project.scale.name();
                }
                self.open_dropdown = None;
            }
        }
    }
    fn apply_side(&mut self, kind: SideParam, v: f32) {
        self.checkpoint(Some(match kind {
            SideParam::Master => "side-master",
            SideParam::Bpm => "side-bpm",
            SideParam::ReverbSize => "side-reverb-size",
            SideParam::ReverbDamp => "side-reverb-damp",
            SideParam::ReverbWet => "side-reverb-wet",
        }));
        match kind {
            SideParam::Master => {
                self.song_mut().master = v;
                self.send(Command::Set(Param::Master(v)));
            }
            SideParam::Bpm => {
                self.song_mut().bpm = v.clamp(BPM_MIN, BPM_MAX);
                self.send(Command::Set(Param::Bpm(v)));
            }
            SideParam::ReverbSize => {
                self.song_mut().reverb_size = v;
                self.send(Command::Set(Param::ReverbSize(v)));
            }
            SideParam::ReverbDamp => {
                self.song_mut().reverb_damp = v;
                self.send(Command::Set(Param::ReverbDamp(v)));
            }
            SideParam::ReverbWet => {
                self.song_mut().reverb_wet = v;
                self.send(Command::Set(Param::ReverbWet(v)));
            }
        }
    }
    fn handle_bpm_key(&mut self, keystroke: &gpui::Keystroke) {
        match keystroke.key.as_str() {
            "enter" => {
                self.bpm_commit();
                return;
            }
            "escape" => {
                self.bpm_editing = false;
                self.bpm_buffer.clear();
                self.status = "已取消速度输入".into();
                return;
            }
            "backspace" => {
                self.bpm_buffer.pop();
                return;
            }
            "up" => {
                self.bpm_nudge(1.0);
                return;
            }
            "down" => {
                self.bpm_nudge(-1.0);
                return;
            }
            _ => {}
        }
        if keystroke.modifiers.control
            || keystroke.modifiers.alt
            || keystroke.modifiers.platform
            || keystroke.modifiers.function
        {
            return;
        }
        let Some(typed) = keystroke.key_char.as_deref() else {
            return;
        };
        if typed.chars().count() != 1 {
            return;
        }
        for ch in typed.chars() {
            if ch.is_ascii_digit() || (ch == '.' && !self.bpm_buffer.contains('.')) {
                if self.bpm_buffer.chars().count() < 5 {
                    self.bpm_buffer.push(ch);
                }
            }
        }
    }
    fn bpm_nudge(&mut self, delta: f32) {
        let base =
            crate::number_input::parse_number(&self.bpm_buffer).unwrap_or_else(|| self.song().bpm);
        let next = crate::number_input::clamp_value(base + delta, BPM_MIN, BPM_MAX);
        self.bpm_buffer = format!("{next:.0}");
        self.apply_bpm(next);
    }
    fn bpm_commit(&mut self) {
        self.bpm_editing = false;
        let Some(value) = crate::number_input::parse_number(&self.bpm_buffer) else {
            self.bpm_buffer.clear();
            self.status = "速度输入无效，保持原值".into();
            return;
        };
        let value = crate::number_input::clamp_value(value, BPM_MIN, BPM_MAX);
        self.bpm_buffer.clear();
        self.apply_bpm(value);
        self.status = format!("速度 {value:.0} BPM");
    }
    fn apply_bpm(&mut self, value: f32) {
        let value = value.clamp(BPM_MIN, BPM_MAX);
        if (self.song().bpm - value).abs() < 1e-4 {
            return;
        }
        self.checkpoint(Some("bpm-input"));
        self.song_mut().bpm = value;
        self.dirty = true;
        self.send(Command::Set(Param::Bpm(value)));
    }
    fn open_voice_panel(&mut self) {
        let id = self
            .song()
            .tracks
            .get(self.selected_track_index())
            .map(|t| t.voice)
            .unwrap_or(sonora_core::voice::builtin::PIANO);
        self.voice_edit = Some(id);
        self.voice_draft = Some(self.library.preset(id));
        self.voice_panel_open = true;
    }
    fn close_voice_panel(&mut self) {
        self.voice_panel_open = false;
        self.voice_edit = None;
        self.voice_draft = None;
    }
    fn publish_voices(&mut self) {
        self.project.voices = self.library.export_user();
        self.dirty = true;
        let lib = self.library.clone();
        self.send(Command::UpdateVoices(lib));
        self.persist_voice_file();
    }
    fn persist_voice_file(&mut self) {
        let Some(path) = self.voices_path.clone() else {
            return;
        };
        if let Err(e) = self.library.save(&path) {
            self.status = format!("音色库写入失败：{e}");
        }
    }
    fn apply_voice(&mut self, action: VoiceAction, cx: &mut Context<Self>) {
        match action {
            VoiceAction::Close => {
                self.close_voice_panel();
                self.status = "音色管理已收起".into();
            }
            VoiceAction::Select(id) => {
                self.voice_edit = Some(id);
                self.voice_draft = Some(self.library.preset(id));
            }
            VoiceAction::New => {
                self.checkpoint(None);
                let tpl = self
                    .voice_edit
                    .unwrap_or(sonora_core::voice::builtin::PIANO);
                let n = self.user_voice_count() + 1;
                let id = self.library.create_from(tpl, format!("我的音色 {n}"));
                if let Some(draft) = self.voice_draft {
                    self.library.update_preset(id, draft);
                }
                self.publish_voices();
                self.voice_edit = Some(id);
                self.voice_draft = Some(self.library.preset(id));
                self.status = "已新建音色".into();
            }
            VoiceAction::Save => {
                if let (Some(id), Some(draft)) = (self.voice_edit, self.voice_draft) {
                    if !self.library.get(id).is_some_and(|v| !v.builtin) {
                        self.status = "内置音色不可修改，请先「另存为」".into();
                        cx.notify();
                        return;
                    }
                    self.checkpoint(None);
                    if self.library.update_preset(id, draft) {
                        self.publish_voices();
                        self.status = "已保存音色".into();
                    }
                }
            }
            VoiceAction::SaveAs => {
                self.checkpoint(None);
                let tpl = self
                    .voice_edit
                    .unwrap_or(sonora_core::voice::builtin::PIANO);
                let n = self.user_voice_count() + 1;
                let id = self.library.create_from(tpl, format!("我的音色 {n}"));
                if let Some(draft) = self.voice_draft {
                    self.library.update_preset(id, draft);
                }
                self.publish_voices();
                self.voice_edit = Some(id);
                self.voice_draft = Some(self.library.preset(id));
                self.status = "已另存为新音色".into();
            }
            VoiceAction::Delete(id) => {
                if !self.library.get(id).is_some_and(|v| !v.builtin) {
                    cx.notify();
                    return;
                }
                self.checkpoint(None);
                if self.library.delete(id) {
                    let mut remapped = false;
                    for track in self.song_mut().tracks.iter_mut() {
                        if track.voice == id {
                            track.voice = sonora_core::voice::builtin::PIANO;
                            track.drums = false;
                            remapped = true;
                        }
                    }
                    if remapped {
                        self.reload();
                    }
                    self.publish_voices();
                    if self.voice_edit == Some(id) {
                        let first = self.library.all().first().map(|v| v.id);
                        self.voice_edit = first;
                        self.voice_draft = first.map(|v| self.library.preset(v));
                    }
                    self.status = "已删除音色".into();
                }
            }
            VoiceAction::Audition => {
                if let Some(draft) = self.voice_draft {
                    if self.engine.is_none() {
                        self.start_audio();
                    }
                    if self.engine.is_some() {
                        for midi in [60u8, 72] {
                            self.send(Command::PreviewPreset(Box::new(draft), midi));
                        }
                        self.status = "试听中".into();
                    } else {
                        self.status = self.status.clone();
                    }
                }
            }
            VoiceAction::AddPartial => {
                if let Some(mut d) = self.voice_draft {
                    if d.count < MAX_PARTIALS {
                        self.checkpoint(None);
                        let n = d.count;
                        d.partials[n] = Partial {
                            ratio: (n + 1) as f32,
                            gain: 0.35,
                            decay_scale: 0.7,
                            detune_cents: 0.0,
                        };
                        d.count += 1;
                        self.voice_draft = Some(d);
                    }
                }
            }
            VoiceAction::RemovePartial(i) => {
                if let Some(mut d) = self.voice_draft {
                    if i < d.count {
                        self.checkpoint(None);
                        for k in i..MAX_PARTIALS - 1 {
                            d.partials[k] = d.partials[k + 1];
                        }
                        d.partials[MAX_PARTIALS - 1] = Partial::default();
                        d.count -= 1;
                        self.voice_draft = Some(d);
                    }
                }
            }
            VoiceAction::BumpPartial(i, field, delta) => {
                if let Some(mut d) = self.voice_draft {
                    if i < MAX_PARTIALS {
                        self.checkpoint(Some(&format!("voice-partial-{i}-{field:?}")));
                        let partial = &mut d.partials[i];
                        match field {
                            PartialField::Ratio => {
                                partial.ratio = (partial.ratio + delta).clamp(0.25, 16.0)
                            }
                            PartialField::Decay => {
                                partial.decay_scale = (partial.decay_scale + delta).clamp(0.05, 2.0)
                            }
                        }
                        self.voice_draft = Some(d);
                    }
                }
            }
            VoiceAction::SetPartialGain(i, gain) => {
                if let Some(mut d) = self.voice_draft {
                    if i < MAX_PARTIALS {
                        self.checkpoint(Some(&format!("voice-gain-{i}")));
                        d.partials[i].gain = gain.clamp(0.0, 1.0);
                        self.voice_draft = Some(d);
                    }
                }
            }
            VoiceAction::SetParam(param, v) => {
                if let Some(mut d) = self.voice_draft {
                    self.checkpoint(Some(&format!("voice-param-{param:?}")));
                    param.set(&mut d, v);
                    d.clamp_params();
                    self.voice_draft = Some(d);
                }
            }
        }
        cx.notify();
    }
    fn user_voice_count(&self) -> usize {
        self.library.all().iter().filter(|v| !v.builtin).count()
    }
    fn waveform_state(&mut self) -> Option<WaveformState> {
        if !self.wave_sidebar_open {
            return None;
        }
        match &self.engine {
            Some(engine) => engine.scope_snapshot(&mut self.scope_buf),
            None => self.scope_buf.clear(),
        }
        Some(WaveformState {
            samples: self.scope_buf.clone(),
            playing: self.last_playing,
            bar: self.last_bar,
            total_bars: self.song().bars,
            position_bars: self.playhead_bars,
            audio_ready: self.engine.is_some(),
        })
    }
    fn voice_panel_state(&self) -> VoicePanelState {
        VoicePanelState {
            voices: self
                .library
                .all()
                .iter()
                .map(|v| VoiceView {
                    id: v.id,
                    name: v.name.clone(),
                    builtin: v.builtin,
                })
                .collect(),
            editing: self.voice_edit,
            draft: self.voice_draft,
            track_name: self
                .song()
                .tracks
                .get(self.selected_track_index())
                .map(|t| t.name.clone())
                .unwrap_or_default(),
            audio_ready: self.engine.is_some(),
        }
    }
    fn side_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette();
        let s = self.song().clone();
        let weak = cx.entity().downgrade();
        let handler = std::rc::Rc::new(
            move |kind: SideParam, v: f32, _window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    this.apply_side(kind, v);
                    cx.notify();
                });
            },
        );
        div()
            .flex()
            .flex_col()
            .w(px(180.0))
            .flex_shrink_0()
            .gap(px(10.0))
            .p(px(10.0))
            .border_l_1()
            .border_color(theme::border())
            .bg(rgb(p.panel))
            .child(
                div()
                    .text_size(px(9.5))
                    .text_color(rgb(p.text_faint))
                    .child("混响"),
            )
            .child(side_bar(
                "大小",
                s.reverb_size,
                SideParam::ReverbSize,
                Some(handler.clone()),
            ))
            .child(side_bar(
                "阻尼",
                s.reverb_damp,
                SideParam::ReverbDamp,
                Some(handler.clone()),
            ))
            .child(side_bar(
                "湿度",
                s.reverb_wet,
                SideParam::ReverbWet,
                Some(handler.clone()),
            ))
            .child(div().h(px(1.0)).bg(theme::border()))
            .child(
                div()
                    .text_size(px(9.5))
                    .text_color(rgb(p.text_faint))
                    .child("总线"),
            )
            .child(side_bar(
                "音量",
                s.master,
                SideParam::Master,
                Some(handler.clone()),
            ))
            .child(side_bar("速度", s.bpm, SideParam::Bpm, Some(handler)))
    }
    fn bottombar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let p = theme::palette();
        let (pl, pr) = self
            .engine
            .as_ref()
            .map(|e| e.transport.take_peaks())
            .unwrap_or((0.0, 0.0));
        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .h(px(26.0))
            .flex_shrink_0()
            .border_t_1()
            .border_color(theme::border())
            .bg(rgb(p.panel))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .w(px(70.0))
                    .flex_shrink_0()
                    .child(meter("L", pl))
                    .child(meter("R", pr)),
            )
            .child(div().w(px(1.0)).h(px(14.0)).bg(theme::border()))
            .child(
                div()
                    .text_size(px(9.5))
                    .text_color(rgb(p.text_muted))
                    .child(format!("第 {} 小节", self.last_bar)),
            );
        let selected = self.selected_indices();
        if !selected.is_empty() {
            let track = self.song().tracks.get(self.selected_track_index());
            let velocity = track
                .and_then(|t| t.notes.get(selected[0]))
                .map(|n| n.vel)
                .unwrap_or(0.8);
            let weak = cx.entity().downgrade();
            let slider =
                crate::slider::mini_slider(gpui::ElementId::Name("sel-velocity".into()), velocity)
                    .range(0.05, 1.0)
                    .selector("sel-velocity")
                    .on_change(move |v, _window, cx| {
                        let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                            this.set_selection_velocity(v);
                            cx.notify();
                        });
                    });
            bar = bar
                .child(div().w(px(1.0)).h(px(14.0)).bg(theme::border()))
                .child(
                    div()
                        .id("selection-info")
                        .debug_selector(|| "selection-info".into())
                        .flex_shrink_0()
                        .text_size(px(9.5))
                        .text_color(rgb(p.text_muted))
                        .child(format!("选中 {} 个", selected.len())),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(9.5))
                        .text_color(rgb(p.text_faint))
                        .child("力度"),
                )
                .child(div().w(px(96.0)).flex_shrink_0().child(slider))
                .child(
                    div()
                        .id("selection-velocity")
                        .debug_selector(|| "selection-velocity".into())
                        .w(px(34.0))
                        .flex_shrink_0()
                        .text_size(px(9.5))
                        .text_color(rgb(p.text_faint))
                        .child(format!("{:.0}%", velocity * 100.0)),
                );
        }
        bar = bar
            .child(div().w(px(1.0)).h(px(14.0)).bg(theme::border()))
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .text_size(px(9.5))
                    .text_color(rgb(p.text_faint))
                    .child(self.status.clone()),
            );
        bar.into_any_element()
    }
    fn sync_transport(&mut self) -> bool {
        let Some(engine) = &self.engine else {
            return false;
        };
        let engine_playing = engine.is_playing();
        let bar = engine.transport.bar().max(1);
        let voices = engine.transport.voices();
        let head = engine.transport.position_bars();
        let since_play = self.play_requested_at.map(|t| t.elapsed());
        if should_clear_want_playing(self.want_playing, engine_playing, voices, since_play) {
            self.want_playing = false;
            self.play_requested_at = None;
        }
        if engine_playing {
            self.play_requested_at = None;
        }
        self.advance_follow(head);
        let changed = (head - self.playhead_bars).abs() > 0.004
            || bar != self.last_bar
            || engine_playing != self.last_playing
            || voices != self.last_voices;
        self.playhead_bars = head;
        self.last_bar = bar;
        self.last_playing = engine_playing;
        self.last_voices = voices;
        changed
    }
    fn advance_follow(&mut self, head: f32) {
        if !self.want_playing || !self.follow_playhead {
            return;
        }
        if let Some(next) = follow_scroll(self.scroll_bars, head, self.song().bars, self.zoom) {
            self.set_scroll_bars(next);
        }
    }
    fn transport_state(&self) -> TransportState {
        TransportState {
            playing: self.want_playing,
            bar: self.last_bar,
            position_bars: self.playhead_bars,
            total_bars: self.song().bars,
            bpm: self.song().bpm,
            voices: self.last_voices,
        }
    }
}

fn default_track_name(i: usize, track: &Track) -> String {
    if track.drums {
        "鼓组".to_string()
    } else {
        format!("轨道 {}", i + 1)
    }
}

impl Render for SonoraApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(f) = &self.focus
            && !f.is_focused(window)
            && !self.focus_requested
        {
            window.focus(f, cx);
            self.focus_requested = true;
        }
        if !self.close_hook_registered {
            self.close_hook_registered = true;
            let weak = cx.weak_entity();
            window.on_window_should_close(cx, move |_window, cx| {
                weak.update(cx, |this: &mut SonoraApp, cx| {
                    let allow = this.on_close_requested();
                    cx.notify();
                    allow
                })
                .unwrap_or(true)
            });
        }
        let p = theme::palette();
        self.ensure_ticker(cx);
        self.sync_transport();
        if self.pitch_center_pending {
            let vh = self.roll_view_h.get();
            if vh > 1.0 {
                let drums = self
                    .song()
                    .tracks
                    .get(self.selected_track_index())
                    .map(|t| t.drums)
                    .unwrap_or(false);
                self.pitch_scroll = crate::piano_roll::default_pitch_scroll(drums, vh, self.row_h);
                self.pitch_center_pending = false;
                cx.notify();
            }
        }
        let title = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名工程".to_string());
        let subtitle = if self.dirty {
            format!("{} 小节  {:.0} BPM  •", self.song().bars, self.song().bpm)
        } else {
            format!("{} 小节  {:.0} BPM", self.song().bars, self.song().bpm)
        };
        let theme_mode = self.theme_mode;
        let open_menu = self.open_menu;
        let selected_track = self.selected_track;
        let track_views = self.track_views();
        let add_menu_open = self.add_menu_open;
        let track_renaming = self.track_renaming;
        let rename_buffer = self.rename_buffer.clone();
        let roll_state = self.roll_state();
        let transport_state = self.transport_state();
        let weak = cx.entity().downgrade();
        let on_theme = {
            let weak = weak.clone();
            move |_window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    this.theme_mode = this.theme_mode.toggled();
                    theme::set_mode(this.theme_mode);
                    cx.notify();
                });
            }
        };
        let on_menu_action = {
            let weak = weak.clone();
            move |action: String, _window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    this.run_menu_action(&action, cx);
                    this.open_menu = None;
                    cx.notify();
                });
            }
        };
        let on_menu = {
            let weak = weak.clone();
            move |menu: Menu, _window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    let was_open = this.open_menu == Some(menu);
                    this.close_popups();
                    if !was_open {
                        this.open_menu = Some(menu);
                    }
                    cx.notify();
                });
            }
        };
        let on_track = {
            let weak = weak.clone();
            move |action: TrackAction, _window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    this.handle_track(action, cx);
                });
            }
        };
        let on_roll = {
            let weak = weak.clone();
            move |action: RollAction, _window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    this.handle_roll(action, cx);
                });
            }
        };
        let bar = title_bar("Sonora")
            .subtitle(format!("{title} {subtitle}"))
            .dark(theme_mode == ThemeMode::Dark)
            .open_menu(open_menu)
            .on_theme(on_theme)
            .on_menu(on_menu)
            .on_menu_action(on_menu_action);
        let toolbar_state = crate::toolbar::ToolbarState {
            playing: transport_state.playing,
            bpm: if self.bpm_editing {
                self.bpm_buffer.clone()
            } else {
                format!("{:.0}", transport_state.bpm)
            },
            bpm_editing: self.bpm_editing,
            grid_label: grid_label(self.grid_beats),
            root_label: sonora_core::NOTE_NAMES[self.project.scale.root as usize].to_string(),
            mode_label: self.project.scale.mode.label().to_string(),
            row_h: self.row_h,
            snap: self.snap_to_scale,
            loop_on: self.project.loop_enabled,
            open_dropdown: self.open_dropdown,
            grid_beats: self.grid_beats,
            root_index: self.project.scale.root as usize,
            mode_index: sonora_core::Mode::ALL
                .iter()
                .position(|m| *m == self.project.scale.mode)
                .unwrap_or(0),
        };
        let on_toolbar = {
            let weak = weak.clone();
            move |action: crate::toolbar::ToolbarAction, _window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                    this.handle_toolbar(action, cx);
                });
            }
        };
        let toolbar = crate::toolbar::toolbar(toolbar_state).on_action(on_toolbar);
        let wave = self
            .waveform_state()
            .map(|s| waveform(s).into_any_element());
        let bottom = self.bottombar(cx);
        let side = self.side_panel(cx);
        let track_state = crate::track_list::TrackListState {
            tracks: track_views,
            selected: selected_track,
            track_row_h: self.track_row_h,
            add_menu_open,
            voices: self
                .library
                .all()
                .iter()
                .map(|v| (v.id, v.name.clone(), v.builtin))
                .collect(),
            renaming: track_renaming,
            rename_buffer,
            voice_pick: self.track_voice_pick,
        };
        let tracks = track_list(track_state).on_action(on_track);
        let roll = crate::piano_roll::piano_roll(roll_state)
            .on_action(on_roll)
            .view_h_cell(self.roll_view_h.clone())
            .canvas_rect_cell(self.roll_canvas_rect.clone());
        let status_text = self.status.clone();
        let note_total = self.song().total_notes();
        let track_total = self.song().tracks.len();
        let scale_name = self.project.scale.name();
        let undo_len = self.history.undo_len();
        let redo_len = self.history.redo_len();
        let weak_keys = weak.clone();
        let focus = self.focus.clone();
        let root = div()
            .flex()
            .flex_col()
            .size_full()
            .relative()
            .bg(rgb(p.bg))
            .text_color(rgb(p.text));
        let root = match &focus {
            Some(f) => root.track_focus(f),
            None => root,
        };
        let mut root = root.on_key_down(
            move |ev: &gpui::KeyDownEvent, _w: &mut Window, cx: &mut App| {
                let closing = weak_keys
                    .update(cx, |this: &mut SonoraApp, _| this.exit_prompt)
                    .unwrap_or(false);
                if closing {
                    let key = ev.keystroke.key.clone();
                    let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                        match key.as_str() {
                            "enter" => {
                                this.apply_exit_action(crate::file_dialog::ExitAction::SaveAndExit);
                            }
                            "escape" => {
                                this.apply_exit_action(crate::file_dialog::ExitAction::Cancel);
                            }
                            _ => {}
                        }
                        cx.notify();
                    });
                    return;
                }
                let prompting = weak_keys
                    .update(cx, |this: &mut SonoraApp, _| this.prompt.is_some())
                    .unwrap_or(false);
                if prompting {
                    let keystroke = ev.keystroke.clone();
                    let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                        this.handle_path_prompt_key(&keystroke);
                        cx.notify();
                    });
                    return;
                }
                let editing = weak_keys
                    .update(cx, |this: &mut SonoraApp, _| this.bpm_editing)
                    .unwrap_or(false);
                if editing {
                    let keystroke = ev.keystroke.clone();
                    let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                        this.handle_bpm_key(&keystroke);
                        cx.notify();
                    });
                    return;
                }
                let renaming = weak_keys
                    .update(cx, |this: &mut SonoraApp, _| this.track_renaming.is_some())
                    .unwrap_or(false);
                if renaming {
                    let keystroke = ev.keystroke.clone();
                    let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                        this.handle_rename_key(&keystroke);
                        cx.notify();
                    });
                    return;
                }
                let secondary = ev.keystroke.modifiers.secondary(cx);
                let key = ev.keystroke.key.clone();
                let shift = ev.keystroke.modifiers.shift;
                let edited = weak_keys
                    .update(cx, |this: &mut SonoraApp, cx| {
                        let handled = this.handle_edit_key(&key, secondary, shift);
                        if handled {
                            cx.notify();
                        }
                        handled
                    })
                    .unwrap_or(false);
                if edited {
                    return;
                }
                if secondary && !ev.keystroke.modifiers.alt {
                    match ev.keystroke.key.as_str() {
                        "z" => {
                            let redo = ev.keystroke.modifiers.shift;
                            let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                                if redo {
                                    this.redo();
                                } else {
                                    this.undo();
                                }
                                cx.notify();
                            });
                            return;
                        }
                        "y" => {
                            let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                                this.redo();
                                cx.notify();
                            });
                            return;
                        }
                        "s" => {
                            let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                                this.save_project();
                                cx.notify();
                            });
                            return;
                        }
                        _ => {}
                    }
                }
                if ev.keystroke.modifiers.modified() {
                    return;
                }
                match ev.keystroke.key.as_str() {
                    "space" => {
                        let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                            this.toggle_play();
                            cx.notify();
                        });
                    }
                    "home" => {
                        let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                            this.rewind();
                            cx.notify();
                        });
                    }
                    "l" => {
                        let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                            this.toggle_loop();
                            cx.notify();
                        });
                    }
                    "escape" => {
                        let _ = weak_keys.update(cx, |this: &mut SonoraApp, cx| {
                            this.close_popups();
                            this.close_voice_panel();
                            cx.notify();
                        });
                    }
                    _ => {}
                }
            },
        );
        let has_popup = self.open_menu.is_some()
            || self.open_dropdown.is_some()
            || self.add_menu_open
            || self.track_voice_pick.is_some();
        if has_popup || self.bpm_editing {
            let weak_popup = weak.clone();
            root = root.on_mouse_down(gpui::MouseButton::Left, move |_ev, _window, cx| {
                let _ = weak_popup.update(cx, |this: &mut SonoraApp, cx| {
                    this.close_popups();
                    if this.bpm_editing {
                        this.bpm_commit();
                    }
                    cx.notify();
                });
            });
        }
        let mut content = div().flex().flex_row().flex_1().min_h_0();
        if let Some(wave) = wave {
            content = content.child(wave);
        }
        content = content
            .child(tracks)
            .child(div().flex().flex_col().flex_1().min_w_0().child(roll))
            .child(side);
        let root = root
            .child(bar)
            .child(toolbar)
            .child(content)
            .child(bottom)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .h(px(crate::layout::STATUSBAR_H))
                    .flex_shrink_0()
                    .px(px(10.0))
                    .gap(px(12.0))
                    .bg(rgb(p.panel))
                    .border_t_1()
                    .border_color(theme::border())
                    .text_size(px(10.0))
                    .text_color(rgb(p.text_faint))
                    .child(div().truncate().child(status_text))
                    .child({
                        let mut info = div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.0))
                            .child(format!("{track_total} 轨道"))
                            .child(format!("{note_total} 音符"))
                            .child(scale_name);
                        if undo_len > 0 || redo_len > 0 {
                            info = info.child(
                                div()
                                    .id("history-indicator")
                                    .debug_selector(|| "history-indicator".into())
                                    .text_color(rgb(p.text_faint))
                                    .child(format!("↶{undo_len} ↷{redo_len}")),
                            );
                        }
                        info
                    }),
            );
        if self.voice_panel_open {
            let on_voice = {
                let weak = weak.clone();
                move |action: VoiceAction, _window: &mut Window, cx: &mut App| {
                    let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                        this.apply_voice(action, cx);
                    });
                }
            };
            return root
                .child(voice_panel(self.voice_panel_state()).on_action(on_voice))
                .into_any_element();
        }
        let root = if let Some(state) = self.prompt.clone() {
            let on_prompt = {
                let weak = weak.clone();
                move |action: crate::file_dialog::PathPromptAction,
                      _window: &mut Window,
                      cx: &mut App| {
                    let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                        match action {
                            crate::file_dialog::PathPromptAction::Confirm => {
                                this.commit_path_prompt()
                            }
                            crate::file_dialog::PathPromptAction::Cancel => {
                                this.cancel_path_prompt()
                            }
                            crate::file_dialog::PathPromptAction::Pick(i) => {
                                this.pick_project_candidate(i)
                            }
                        }
                        cx.notify();
                    });
                }
            };
            root.child(crate::file_dialog::path_prompt(state).on_action(on_prompt))
        } else {
            root
        };
        let root = if self.exit_prompt {
            let on_exit = {
                let weak = weak.clone();
                move |action: crate::file_dialog::ExitAction, window: &mut Window, cx: &mut App| {
                    let _ = weak.update(cx, |this: &mut SonoraApp, cx| {
                        if this.apply_exit_action(action) {
                            window.remove_window();
                        }
                        cx.notify();
                    });
                }
            };
            let project = self
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| self.project.display_name());
            root.child(crate::file_dialog::exit_prompt(project).on_action(on_exit))
        } else {
            root
        };
        root.into_any_element()
    }
}

#[doc(hidden)]
impl SonoraApp {
    pub fn has_audio(&self) -> bool {
        self.engine.is_some()
    }
    pub fn debug_toggle_play(&mut self) {
        self.toggle_play();
    }
    pub fn debug_sync(&mut self) -> bool {
        self.sync_transport()
    }
    pub fn debug_is_playing(&self) -> bool {
        self.want_playing
    }
    pub fn debug_note_count(&self) -> usize {
        self.song()
            .tracks
            .get(self.selected_track_index())
            .map(|t| t.notes.len())
            .unwrap_or(0)
    }
    pub fn debug_total_notes(&self) -> usize {
        self.song().total_notes()
    }
    pub fn debug_last_note(&self) -> Option<(u32, f32, u8, f32)> {
        self.song()
            .tracks
            .get(self.selected_track_index())
            .and_then(|t| t.notes.last())
            .map(|n| (n.bar, n.beat, n.midi, n.len))
    }
    pub fn debug_drag_active(&self) -> bool {
        self.drag_mode.is_some()
    }
    pub fn debug_open_menu(&self) -> Option<Menu> {
        self.open_menu
    }
    pub fn debug_open_dropdown(&self) -> Option<crate::toolbar::Dropdown> {
        self.open_dropdown
    }
    pub fn debug_add_menu_open(&self) -> bool {
        self.add_menu_open
    }
    pub fn debug_voice_pick(&self) -> Option<usize> {
        self.track_voice_pick
    }
    pub fn debug_grid_beats(&self) -> f32 {
        self.grid_beats
    }
    pub fn debug_track_count(&self) -> usize {
        self.song().tracks.len()
    }
    pub fn debug_track_level(&self, i: usize) -> f32 {
        self.song().tracks.get(i).map(|t| t.level).unwrap_or(0.0)
    }
    pub fn debug_master(&self) -> f32 {
        self.song().master
    }
    pub fn debug_voice_panel_open(&self) -> bool {
        self.voice_panel_open
    }
    pub fn debug_voice_edit(&self) -> Option<VoiceId> {
        self.voice_edit
    }
    pub fn debug_voice_draft(&self) -> Option<VoicePreset> {
        self.voice_draft
    }
    pub fn debug_voice_count(&self) -> usize {
        self.library.all().len()
    }
    pub fn debug_user_voice_count(&self) -> usize {
        self.user_voice_count()
    }
    pub fn debug_library_preset(&self, id: VoiceId) -> VoicePreset {
        self.library.preset(id)
    }
    pub fn debug_track_voice(&self, i: usize) -> Option<VoiceId> {
        self.song().tracks.get(i).map(|t| t.voice)
    }
    pub fn debug_persisted_voice_count(&self) -> usize {
        self.project.voices.len()
    }
    pub fn debug_status(&self) -> &str {
        &self.status
    }
    pub fn debug_undo_len(&self) -> usize {
        self.history.undo_len()
    }
    pub fn debug_redo_len(&self) -> usize {
        self.history.redo_len()
    }
    pub fn debug_can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn debug_can_redo(&self) -> bool {
        self.history.can_redo()
    }
    pub fn debug_apply_undo(&mut self) {
        self.undo();
    }
    pub fn debug_apply_redo(&mut self) {
        self.redo();
    }
    pub fn debug_set_path(&mut self, path: std::path::PathBuf) {
        self.path = Some(path);
    }
    pub fn debug_is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn debug_bpm_editing(&self) -> bool {
        self.bpm_editing
    }
    pub fn debug_bpm_buffer(&self) -> &str {
        &self.bpm_buffer
    }
    pub fn debug_song_bpm(&self) -> f32 {
        self.song().bpm
    }
    pub fn debug_save_path(&self) -> Option<std::path::PathBuf> {
        self.path.clone()
    }
    pub fn debug_save_as_open(&self) -> bool {
        self.prompt
            .as_ref()
            .is_some_and(|p| p.kind == crate::file_dialog::PathPromptKind::SaveAs)
    }
    pub fn debug_save_as_buffer(&self) -> Option<String> {
        self.prompt.as_ref().map(|p| p.buffer.clone())
    }
    pub fn debug_prompt_kind(&self) -> Option<crate::file_dialog::PathPromptKind> {
        self.prompt.as_ref().map(|p| p.kind)
    }
    pub fn debug_prompt_candidates(&self) -> Vec<std::path::PathBuf> {
        self.prompt
            .as_ref()
            .map(|p| p.candidates.clone())
            .unwrap_or_default()
    }
    pub fn debug_set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
    pub fn debug_on_close_requested(&mut self) -> bool {
        self.on_close_requested()
    }
    pub fn debug_exit_prompt_open(&self) -> bool {
        self.exit_prompt
    }
    pub fn debug_apply_exit(&mut self, action: crate::file_dialog::ExitAction) -> bool {
        self.apply_exit_action(action)
    }
    pub fn debug_run_autosave(&mut self) {
        self.last_autosave = std::time::Instant::now() - AUTOSAVE_INTERVAL;
        self.maybe_autosave();
    }
    pub fn debug_set_projects_dir(&mut self, dir: std::path::PathBuf) {
        self.projects_dir = Some(dir);
    }
    pub fn debug_wave_sidebar_open(&self) -> bool {
        self.wave_sidebar_open
    }
    pub fn debug_set_voices_path(&mut self, path: std::path::PathBuf) {
        self.voices_path = Some(path);
    }
    pub fn debug_set_popups(
        &mut self,
        menu: Option<Menu>,
        add_menu: bool,
        voice_pick: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.open_menu = menu;
        self.add_menu_open = add_menu;
        self.track_voice_pick = voice_pick;
        cx.notify();
    }
    pub fn debug_song_bars(&self) -> u32 {
        self.song().bars
    }
    pub fn debug_zoom(&self) -> f32 {
        self.zoom
    }
    pub fn debug_scroll_bars(&self) -> f32 {
        self.scroll_bars
    }
    pub fn debug_set_scroll_bars(&mut self, target: f32) {
        self.set_scroll_bars(target);
    }
    pub fn debug_set_scroll_bars_notify(&mut self, target: f32, cx: &mut Context<Self>) {
        self.set_scroll_bars(target);
        cx.notify();
    }
    pub fn debug_canvas_rect(&self) -> (f32, f32, f32) {
        self.roll_canvas_rect.get()
    }
    pub fn debug_row_h(&self) -> f32 {
        self.row_h
    }
    pub fn debug_selected(&self) -> Vec<usize> {
        self.selected_indices()
    }
    pub fn debug_note(&self, i: usize) -> Option<(u32, f32, u8, f32, f32)> {
        let track = self.song().tracks.get(self.selected_track_index())?;
        track
            .notes
            .get(i)
            .map(|n| (n.bar, n.beat, n.midi, n.len, n.vel))
    }
    pub fn debug_track_name(&self, i: usize) -> Option<String> {
        self.song().tracks.get(i).map(|t| t.name.clone())
    }
    pub fn debug_loop(&self) -> Option<(f32, f32)> {
        self.project.loop_bars
    }
    pub fn debug_loop_enabled(&self) -> bool {
        self.project.loop_enabled
    }
    pub fn debug_set_loop(&mut self, region: Option<(f32, f32)>, enabled: bool) {
        self.project.loop_bars = region;
        self.project.loop_enabled = enabled;
        self.sync_loop();
    }
    pub fn debug_metronome_on(&self) -> bool {
        self.metronome_on
    }
    pub fn debug_count_in_on(&self) -> bool {
        self.count_in_on
    }
    pub fn debug_clipboard_len(&self) -> usize {
        self.note_clipboard.len()
    }
    pub fn debug_add_note(&mut self, bar: u32, beat: f32, midi: u8, len: f32, vel: f32) {
        let idx = self.selected_track_index();
        self.song_mut().tracks[idx].notes.push(Note {
            bar,
            beat,
            midi,
            len,
            vel,
        });
        self.selected_notes = vec![self.song().tracks[idx].notes.len() - 1];
    }
    pub fn debug_select(&mut self, indices: Vec<usize>) {
        self.selected_notes = indices;
    }
    pub fn debug_renaming(&self) -> Option<usize> {
        self.track_renaming
    }
    pub fn debug_rename_buffer(&self) -> String {
        self.rename_buffer.clone()
    }
    pub fn debug_pitch_scroll(&self) -> usize {
        self.pitch_scroll
    }
    pub fn debug_x_for_beats(&self, beats: f32) -> f32 {
        let bars = self.song().bars.max(1);
        let (_, _, w) = self.roll_canvas_rect.get();
        use crate::piano_roll::{bar_width, clamp_zoom};
        let bar_w = bar_width(w, bars, clamp_zoom(self.zoom));
        (beats / 4.0 - self.scroll_bars) * bar_w
    }
    pub fn debug_note_center(&self, index: usize) -> Option<(f32, f32)> {
        let track = self.song().tracks.get(self.selected_track_index())?;
        let note = track.notes.get(index)?;
        let (cx0, cy0, w) = self.roll_canvas_rect.get();
        if w <= 0.0 {
            return None;
        }
        let bars = self.song().bars.max(1);
        let bar_w = crate::piano_roll::bar_width(w, bars, crate::piano_roll::clamp_zoom(self.zoom));
        let row_h = crate::piano_roll::clamp_row_h(self.row_h);
        let rows = crate::piano_roll::visible_rows(track.drums);
        let abs_row = rows.iter().position(|p| *p == note.midi)?;
        let nx = ((note.bar.saturating_sub(1)) as f32 + (note.beat - 1.0) / 4.0 - self.scroll_bars)
            * bar_w;
        let ny = (abs_row as f32 - self.pitch_scroll as f32) * row_h;
        Some((cx0 + nx + 8.0, cy0 + ny + row_h / 2.0))
    }
    pub fn debug_same_pitch_overlaps(&self) -> usize {
        let Some(track) = self.song().tracks.get(self.selected_track_index()) else {
            return 0;
        };
        let notes = &track.notes;
        let mut count = 0;
        for i in 0..notes.len() {
            for j in (i + 1)..notes.len() {
                let (a, b) = (&notes[i], &notes[j]);
                if a.midi != b.midi {
                    continue;
                }
                let sa = a.start_beats();
                let sb = b.start_beats();
                if sa < sb + b.len - 1e-4 && sb < sa + a.len - 1e-4 {
                    count += 1;
                }
            }
        }
        count
    }
    pub fn debug_transport(&self) -> (u32, bool, u32) {
        (self.last_bar, self.last_playing, self.last_voices)
    }
    pub fn debug_heard_signal(&mut self) -> bool {
        let Some(engine) = &self.engine else {
            return false;
        };
        let mut scope = Vec::new();
        engine.scope_snapshot(&mut scope);
        scope.iter().fold(0.0f32, |m, s| m.max(s.abs())) > 0.001
    }
}

pub fn keyboard_shortcuts_change(app: &mut SonoraApp, key: &str) -> bool {
    match key {
        "space" => {
            app.toggle_play();
            true
        }
        "home" => {
            app.rewind();
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> SonoraApp {
        SonoraApp::bare(Project::empty())
    }
    #[test]
    fn note_beats_roundtrip_and_overlap() {
        let n = note_at_beats(5.0, 60, 1.0, 0.8);
        assert_eq!((n.bar, n.beat), (2, 2.0));
        assert!((n.start_beats() - 5.0).abs() < 1e-5);
        assert_eq!(note_at_beats(0.0, 60, 1.0, 0.8).bar, 1);
        let a = Note {
            bar: 1,
            beat: 1.0,
            midi: 60,
            len: 1.0,
            vel: 0.8,
        };
        let b = Note {
            bar: 1,
            beat: 1.5,
            midi: 60,
            len: 1.0,
            vel: 0.8,
        };
        let c = Note {
            bar: 1,
            beat: 3.0,
            midi: 60,
            len: 1.0,
            vel: 0.8,
        };
        assert!(intervals_overlap(&a, &b), "相邻重叠应被识别");
        assert!(!intervals_overlap(&a, &c), "错开的音符不算重叠");
    }
    #[test]
    fn new_app_starts_clean() {
        let a = app();
        assert_eq!(a.selected_track, 0);
        assert!(a.engine.is_none());
        assert!(!a.dirty);
        assert_eq!(a.song().total_notes(), 0);
    }
    #[test]
    fn default_track_name_for_drums() {
        let drums = Track::new(sonora_core::voice::builtin::DRUMS, true, 0.8);
        assert_eq!(default_track_name(0, &drums), "鼓组");
    }
    #[test]
    fn default_track_name_numbers_tracks() {
        let t = Track::new(0, false, 0.8);
        assert_eq!(default_track_name(0, &t), "轨道 1");
        assert_eq!(default_track_name(4, &t), "轨道 5");
    }
    #[test]
    fn track_views_match_song() {
        let mut a = app();
        a.song_mut().tracks.push(Track::new(0, false, 0.5));
        let views = a.track_views();
        assert_eq!(views.len(), 2);
        assert!((views[0].level - 0.9).abs() < 0.2);
    }
    #[test]
    fn roll_state_reflects_selected_track() {
        let mut a = app();
        a.song_mut().tracks[0].notes.push(Note {
            bar: 1,
            beat: 1.0,
            midi: 60,
            len: 1.0,
            vel: 0.8,
        });
        let state = a.roll_state();
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.notes[0].midi, 60);
        assert_eq!(state.bars, a.song().bars);
    }
    #[test]
    fn selected_track_is_clamped() {
        let mut a = app();
        a.selected_track = 99;
        assert_eq!(a.selected_track_index(), 0);
    }
    #[test]
    fn zoom_affects_bar_width() {
        let mut a = app();
        a.zoom = 1.0;
        let a1 = a.bar_width();
        a.zoom = 2.0;
        let a2 = a.bar_width();
        assert!((a2 / a1 - 2.0).abs() < 1e-5);
    }
    #[test]
    fn transport_state_reads_song() {
        let a = app();
        let s = a.transport_state();
        assert_eq!(s.total_bars, a.song().bars);
        assert_eq!(s.bpm, a.song().bpm);
        assert_eq!(s.voices, 0);
        assert!(!s.playing);
    }
    #[test]
    fn play_toggle_flips_intent_without_audio_device() {
        let mut a = app();
        a.start_audio();
        if a.engine.is_none() {
            eprintln!("跳过：本机没有音频输出设备");
            return;
        }
        assert!(!a.want_playing);
        a.toggle_play();
        assert!(a.want_playing, "首次点击应进入播放意图");
        assert!(a.transport_state().playing, "UI 状态应立即反映播放");
        a.toggle_play();
        assert!(!a.want_playing, "再次点击应停止");
        assert!(!a.transport_state().playing);
    }
    #[test]
    fn transport_playing_follows_intent_not_engine() {
        let mut a = app();
        a.want_playing = true;
        assert!(
            a.transport_state().playing,
            "显示状态必须来自 UI 意图，不能等引擎异步回报"
        );
    }
    #[test]
    fn playback_state_waits_for_the_engine_before_giving_up() {
        use std::time::Duration;
        assert!(!should_clear_want_playing(
            true,
            false,
            0,
            Some(Duration::from_millis(30))
        ));
        assert!(should_clear_want_playing(
            true,
            false,
            0,
            Some(PLAY_GRACE + Duration::from_millis(1))
        ));
        assert!(!should_clear_want_playing(true, true, 0, None));
        assert!(!should_clear_want_playing(false, false, 0, None));
        assert!(!should_clear_want_playing(
            true,
            false,
            3,
            Some(PLAY_GRACE * 2)
        ));
    }
    #[test]
    fn follow_scroll_keeps_playhead_visible() {
        assert_eq!(follow_scroll(0.0, 3.0, 16, 2.5), None);
        let next = follow_scroll(0.0, 8.0, 16, 2.5).expect("播放头超出视野应滚动");
        assert!(next > 0.0 && next < 8.0, "应平滑追上去，实际 {next}");
        let mut scroll = 0.0;
        for _ in 0..600 {
            match follow_scroll(scroll, 8.0, 16, 2.5) {
                Some(n) => scroll = n,
                None => break,
            }
        }
        let visible = 16.0 / 2.5;
        assert!(
            8.0 - scroll <= visible * FOLLOW_TRIGGER + 0.01,
            "播放头应收进视图：scroll={scroll}"
        );
        assert!(scroll >= 0.0);
        assert_eq!(follow_scroll(0.0, 3.0, 8, 1.0), None);
        let back = follow_scroll(6.0, 1.0, 16, 2.5).expect("应往回滚");
        assert!(back < 6.0 && back >= 0.0);
    }
    #[test]
    fn playing_scrolls_the_roll_but_manual_scroll_wins() {
        let mut a = app();
        a.want_playing = true;
        a.follow_playhead = true;
        a.scroll_bars = 0.0;
        a.advance_follow(8.0);
        assert!(a.scroll_bars > 0.0, "播放时应自动滚动");
        let kept = a.scroll_bars;
        a.follow_playhead = false;
        a.advance_follow(12.0);
        assert_eq!(a.scroll_bars, kept, "手动滚动后不应被自动滚动抢走");
        a.follow_playhead = true;
        a.want_playing = false;
        let kept = a.scroll_bars;
        a.advance_follow(15.0);
        assert_eq!(a.scroll_bars, kept);
    }
    #[test]
    fn delete_last_track_is_rejected() {
        let mut a = app();
        let mut guard = 0;
        while a.song().tracks.len() > 1 && guard < 10 {
            a.song_mut().tracks.pop();
            guard += 1;
        }
        assert_eq!(a.song().tracks.len(), 1);
    }
    #[test]
    fn pitch_scroll_centers_middle_c() {
        let a = app();
        let rows = crate::piano_roll::visible_rows(false);
        let c4 = rows.iter().position(|&p| p == 60).unwrap();
        let vis = crate::piano_roll::visible_count(600.0, a.row_h);
        let mid = a.pitch_scroll + vis / 2;
        assert!(
            (c4 as i32 - mid as i32).abs() <= 1,
            "C4 应居中：scroll={} vis={vis} c4={c4}",
            a.pitch_scroll
        );
        assert!(
            c4 > a.pitch_scroll,
            "C4 不能落在视口顶行（顶行会让可视区间看起来掉到 C3）"
        );
    }
    #[test]
    fn clamp_helpers_keep_values_sane() {
        use crate::layout::{MIN_PITCH_SCROLL, clamp_scroll, clamp_zoom};
        assert_eq!(clamp_zoom(-1.0), crate::layout::MIN_ZOOM);
        assert_eq!(clamp_zoom(100.0), crate::layout::MAX_ZOOM);
        assert_eq!(clamp_scroll(-5.0, 100.0), 0.0);
        assert_eq!(MIN_PITCH_SCROLL, 0);
    }
    #[test]
    fn note_addition_detects_duplicates() {
        let mut a = app();
        a.song_mut().tracks[0].notes.push(Note {
            bar: 1,
            beat: 2.0,
            midi: 64,
            len: 1.0,
            vel: 0.8,
        });
        let dup = a.song().tracks[0]
            .notes
            .iter()
            .any(|n| n.bar == 1 && (n.beat - 2.0).abs() < 1e-3 && n.midi == 64);
        assert!(dup, "重复检测应命中");
    }
    fn app_with_notes(notes: &[(u32, f32, u8, f32)]) -> SonoraApp {
        let mut a = app();
        for &(bar, beat, midi, len) in notes {
            a.song_mut().tracks[0].notes.push(Note {
                bar,
                beat,
                midi,
                len,
                vel: 0.8,
            });
        }
        a
    }
    fn move_blocked(notes: &[Note], index: usize, bar: u32, beat: f32, midi: u8) -> bool {
        let Some(note) = notes.get(index) else {
            return true;
        };
        let new_start = (bar.saturating_sub(1)) as f32 * 4.0 + (beat - 1.0);
        let new_end = new_start + note.len;
        notes.iter().enumerate().any(|(i, n)| {
            i != index && n.midi == midi && {
                let s = n.start_beats();
                let e = s + n.len;
                new_start < e - 1e-4 && s < new_end - 1e-4
            }
        })
    }
    #[test]
    fn move_onto_same_pitch_is_blocked() {
        let a = app_with_notes(&[(1, 1.0, 60, 1.0), (2, 1.0, 60, 1.0)]);
        let notes = &a.song().tracks[0].notes;
        assert!(
            move_blocked(notes, 1, 1, 1.0, 60),
            "拖到同音高已有音符上必须被拒绝"
        );
        assert!(!move_blocked(notes, 1, 3, 1.0, 60), "拖到空白处应允许");
        assert!(!move_blocked(notes, 1, 1, 1.0, 62), "换成不同音高不算重叠");
    }
    #[test]
    fn scale_snap_is_off_by_default_so_black_keys_work() {
        let a = app();
        assert!(
            !a.snap_to_scale,
            "调内吸附默认关闭，否则钢琴窗里点黑键会被吸到白键上"
        );
    }
    #[test]
    fn black_key_pitch_survives_when_snap_disabled() {
        let in_scale: Vec<u8> = (24..=127).filter(|p| (p % 12) <= 11).collect();
        let pitch = crate::piano_roll::snap_pitch_to_scale(61, false, false, &in_scale);
        assert_eq!(pitch, 61, "关闭吸附后 C#4 不应被改成别的音");
    }
    #[test]
    fn dropdown_click_opens_without_changing_value() {
        use crate::toolbar::{Dropdown, ToolbarAction as A};
        let mut a = app();
        let before = a.grid_beats;
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Grid));
        assert_eq!(a.open_dropdown, Some(Dropdown::Grid), "应展开网格列表");
        assert_eq!(
            a.grid_beats, before,
            "仅仅展开列表不能改动当前值（旧实现是点一下就切下一个）"
        );
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Grid));
        assert_eq!(a.open_dropdown, None, "再次点击应收起");
        assert_eq!(a.grid_beats, before);
    }
    #[test]
    fn close_popups_clears_every_overlay() {
        let mut a = app();
        a.open_menu = Some(crate::titlebar::Menu::Help);
        a.open_dropdown = Some(crate::toolbar::Dropdown::Mode);
        a.add_menu_open = true;
        a.track_voice_pick = Some(1);
        a.close_popups();
        assert!(a.open_menu.is_none(), "标题栏菜单应收起");
        assert!(a.open_dropdown.is_none(), "下拉框应收起");
        assert!(!a.add_menu_open, "添加乐器浮层应收起");
        assert!(a.track_voice_pick.is_none(), "音色挑选浮层应收起");
    }
    #[test]
    fn opening_one_popup_closes_the_others() {
        use crate::toolbar::{Dropdown, ToolbarAction as A};
        let mut a = app();
        a.open_menu = Some(crate::titlebar::Menu::File);
        a.add_menu_open = true;
        a.track_voice_pick = Some(0);
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Grid));
        assert_eq!(a.open_dropdown, Some(Dropdown::Grid), "新浮层应打开");
        assert!(a.open_menu.is_none(), "打开下拉框应顺手收起标题栏菜单");
        assert!(!a.add_menu_open, "打开下拉框应顺手收起添加乐器");
        assert!(a.track_voice_pick.is_none(), "打开下拉框应顺手收起音色挑选");
    }
    #[test]
    fn picking_dropdown_option_applies_and_closes() {
        use crate::toolbar::{Dropdown, ToolbarAction as A};
        let mut a = app();
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Mode));
        a.apply_toolbar(A::PickMode(1));
        assert_eq!(a.project.scale.mode, sonora_core::Mode::ALL[1]);
        assert_eq!(a.open_dropdown, None, "选中后应自动收起");
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Root));
        a.apply_toolbar(A::PickRoot(7));
        assert_eq!(a.project.scale.root, 7);
        assert_eq!(a.open_dropdown, None);
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Grid));
        a.apply_toolbar(A::PickGrid(1));
        assert!((a.grid_beats - GRID_OPTIONS[1]).abs() < 1e-6);
        assert_eq!(a.open_dropdown, None);
    }
    #[test]
    fn opening_one_dropdown_closes_the_other() {
        use crate::toolbar::{Dropdown, ToolbarAction as A};
        let mut a = app();
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Grid));
        a.apply_toolbar(A::ToggleDropdown(Dropdown::Root));
        assert_eq!(
            a.open_dropdown,
            Some(Dropdown::Root),
            "同一时刻只能展开一个下拉框"
        );
    }
    #[test]
    fn scroll_can_pass_the_end_of_the_song() {
        let mut a = app();
        a.song_mut().bars = 32;
        a.zoom = 2.5;
        let max = a.max_scroll_bars();
        assert!(
            max > 32.0 - 12.8,
            "滚动上限应超过'末小节贴右边'的位置，实际 {max}"
        );
        assert_eq!(max, 32.0, "上限应能滚到 32 小节处（末小节位于视口左侧）");
    }
    #[test]
    fn scroll_target_is_clamped_to_new_limit() {
        let mut a = app();
        a.song_mut().bars = 32;
        a.zoom = 2.5;
        a.set_scroll_bars(100.0);
        assert!((a.scroll_bars - 32.0).abs() < 1e-4, "超出上限应被钳制");
        a.set_scroll_bars(-10.0);
        assert_eq!(a.scroll_bars, 0.0, "不能滚到负数");
        a.set_scroll_bars(5.0);
        a.set_scroll_bars(f32::NAN);
        assert!((a.scroll_bars - 5.0).abs() < 1e-4);
    }
    #[test]
    fn grow_to_fit_extends_song_for_note_past_end() {
        let mut a = app();
        a.song_mut().bars = 32;
        a.song_mut().tracks[0].notes.push(Note {
            bar: 40,
            beat: 1.0,
            midi: 60,
            len: 4.0,
            vel: 0.8,
        });
        let grew = a.grow_to_fit(0);
        assert!(grew, "音符落在 40 小节应触发延长");
        assert_eq!(a.song().bars, 40, "工程应延长到能容纳该音符");
    }
    #[test]
    fn grow_to_fit_accounts_for_note_length() {
        let mut a = app();
        a.song_mut().bars = 8;
        a.song_mut().tracks[0].notes.push(Note {
            bar: 8,
            beat: 1.0,
            midi: 60,
            len: 12.0,
            vel: 0.8,
        });
        a.grow_to_fit(0);
        assert!(
            a.song().bars >= 10,
            "按音符尾部计算，工程应延长到能覆盖其结尾，实际 {}",
            a.song().bars
        );
    }
    #[test]
    fn grow_to_fit_does_not_shrink_song() {
        let mut a = app();
        a.song_mut().bars = 32;
        let grew = a.grow_to_fit(0);
        assert!(!grew, "没有越界音符时不应改动工程长度");
        assert_eq!(a.song().bars, 32);
    }
}

pub const GRID_OPTIONS: [f32; 6] = [1.0, 0.5, 0.25, 0.125, 0.0625, 0.03125];

pub const BPM_MIN: f32 = 40.0;
pub const BPM_MAX: f32 = 160.0;

pub const AUTOSAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(120);

pub fn grid_label(grid: f32) -> String {
    if grid >= 1.0 {
        "1/4".to_string()
    } else if (grid - 0.5).abs() < 1e-6 {
        "1/8".to_string()
    } else if (grid - 0.25).abs() < 1e-6 {
        "1/16".to_string()
    } else if (grid - 0.125).abs() < 1e-6 {
        "1/32".to_string()
    } else if (grid - 0.0625).abs() < 1e-6 {
        "1/64".to_string()
    } else {
        "1/128".to_string()
    }
}

pub fn next_grid(grid: f32) -> f32 {
    let idx = GRID_OPTIONS
        .iter()
        .position(|g| (g - grid).abs() < 1e-6)
        .unwrap_or(2);
    GRID_OPTIONS[(idx + 1) % GRID_OPTIONS.len()]
}

pub const PLAY_GRACE: std::time::Duration = std::time::Duration::from_millis(600);

pub fn should_clear_want_playing(
    want_playing: bool,
    engine_playing: bool,
    voices: u32,
    since_play: Option<std::time::Duration>,
) -> bool {
    if !want_playing || engine_playing {
        return false;
    }
    if since_play.is_some_and(|d| d < PLAY_GRACE) {
        return false;
    }
    voices == 0
}

pub const FOLLOW_ANCHOR: f32 = 0.25;

pub const FOLLOW_TRIGGER: f32 = 0.8;

pub const FOLLOW_SMOOTHING: f32 = 0.3;

pub fn follow_scroll(scroll: f32, head: f32, bars: u32, zoom: f32) -> Option<f32> {
    let bars = bars.max(1) as f32;
    let visible = bars / crate::piano_roll::clamp_zoom(zoom);
    if visible >= bars - 1e-3 {
        return None;
    }
    let left = scroll + visible * FOLLOW_ANCHOR;
    let right = scroll + visible * FOLLOW_TRIGGER;
    if head >= left && head <= right {
        return None;
    }
    let target = (head - visible * FOLLOW_ANCHOR).max(0.0);
    let next = (scroll + (target - scroll) * FOLLOW_SMOOTHING).max(0.0);
    if (next - scroll).abs() < 1e-4 {
        None
    } else {
        Some(next)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SideParam {
    Master,
    Bpm,
    ReverbSize,
    ReverbDamp,
    ReverbWet,
}

impl SideParam {
    pub fn range(self) -> (f32, f32) {
        match self {
            SideParam::Bpm => (BPM_MIN, BPM_MAX),
            _ => (0.0, 1.0),
        }
    }
    pub fn display(self, value: f32) -> String {
        match self {
            SideParam::Bpm => format!("{:.0}", value.round()),
            _ => format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0),
        }
    }
}

fn side_bar(
    label: &'static str,
    value: f32,
    kind: SideParam,
    handler: Option<std::rc::Rc<dyn Fn(SideParam, f32, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let p = theme::palette();
    let (mn, mx) = kind.range();
    let mut bar =
        crate::slider::mini_slider(gpui::ElementId::Name(format!("sb-{label}").into()), value)
            .range(mn, mx)
            .selector(format!("sb-{label}"));
    if let Some(h) = handler {
        bar = bar.on_change(move |v, window, cx| h(kind, v, window, cx));
    }
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .text_size(px(9.5))
                .text_color(rgb(p.text_muted))
                .child(label)
                .child(
                    div()
                        .text_color(rgb(p.text_faint))
                        .child(kind.display(value)),
                ),
        )
        .child(bar)
}

fn with_extension_unless(path: &std::path::Path, ext: &str) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension(ext)
    }
}

fn note_at_beats(start_beats: f32, midi: u8, len: f32, vel: f32) -> Note {
    let bar = (start_beats / 4.0).floor().max(0.0) as u32 + 1;
    let beat = start_beats - (bar.saturating_sub(1)) as f32 * 4.0 + 1.0;
    Note {
        bar,
        beat: beat.clamp(1.0, 4.0),
        midi,
        len,
        vel,
    }
}

fn intervals_overlap(a: &Note, b: &Note) -> bool {
    let a_start = a.start_beats();
    let b_start = b.start_beats();
    a_start < b_start + b.len - 1e-4 && b_start < a_start + a.len - 1e-4
}

fn meter(label: &'static str, value: f32) -> impl IntoElement {
    let p = theme::palette();
    let v = value.clamp(0.0, 1.0);
    let col = if v > 0.92 {
        rgb(p.danger)
    } else {
        rgb(p.accent)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .w(px(12.0))
                .flex_shrink_0()
                .text_size(px(11.0))
                .text_color(rgb(p.text_faint))
                .child(label),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .h(px(4.0))
                .rounded(px(2.0))
                .bg(theme::alpha(p.border_base, p.border_a * 1.8))
                .child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px(0.0))
                        .h_full()
                        .rounded(px(2.0))
                        .w(gpui::relative(v))
                        .bg(col),
                ),
        )
}

fn write_wav_simple(
    path: &std::path::Path,
    left: &[f32],
    right: &[f32],
    sr: u32,
) -> std::io::Result<()> {
    use std::io::Write;
    let frames = left.len().min(right.len());
    let data_bytes = (frames * 4) as u32;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&sr.to_le_bytes());
    header.extend_from_slice(&(sr * 4).to_le_bytes());
    header.extend_from_slice(&4u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());
    let mut f = std::fs::File::create(path)?;
    f.write_all(&header)?;
    for i in 0..frames {
        let l = (left[i].clamp(-1.0, 1.0) * 32767.0) as i16;
        let r = (right[i].clamp(-1.0, 1.0) * 32767.0) as i16;
        f.write_all(&l.to_le_bytes())?;
        f.write_all(&r.to_le_bytes())?;
    }
    Ok(())
}
