use gpui::{AppContext as _, Entity, Modifiers, VisualTestContext};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup_with_path(
    cx: &mut gpui::TestAppContext,
    path: std::path::PathBuf,
) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| {
        let mut app = SonoraApp::new(project, cx);
        app.debug_set_path(path);
        app
    });
    cx.run_until_parked();
    (app, cx)
}

fn click_element(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

fn open_panel(cx: &mut VisualTestContext) {
    click_element(cx, "menu-视图");
    click_element(cx, "mi-toggle-voices");
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("sonora-voice-persist");
    std::fs::create_dir_all(&dir).expect("创建临时目录");
    dir.join(format!("{tag}-{}.sonora", std::process::id()))
}

fn temp_voices_path(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("sonora-voice-persist");
    std::fs::create_dir_all(&dir).expect("创建临时目录");
    dir.join(format!("{tag}-voices-{}.json", std::process::id()))
}

fn set_voices_path(cx: &mut VisualTestContext, app: &Entity<SonoraApp>, path: std::path::PathBuf) {
    cx.update(|_window, app_cx| {
        app.update(app_cx, |this, _| this.debug_set_voices_path(path));
    });
    cx.run_until_parked();
}

#[gpui::test]
fn created_voice_survives_save_and_reopen(cx: &mut gpui::TestAppContext) {
    let path = temp_path("roundtrip");
    let _ = std::fs::remove_file(&path);
    let (app, cx) = setup_with_path(cx, path.clone());
    open_panel(cx);
    click_element(cx, "pv-new");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_user_voice_count()), 1);
    cx.simulate_keystrokes("ctrl-s");
    cx.run_until_parked();
    let saved = Project::load(&path).expect("工程应能重新载入");
    assert_eq!(
        saved.voices.len(),
        1,
        "保存的工程里应带上用户音色，实际 {}",
        saved.voices.len()
    );
    let (app2, cx2) = cx
        .cx
        .add_window_view(move |_, cx| SonoraApp::new(saved, cx));
    cx2.run_until_parked();
    assert_eq!(
        cx2.read_entity(&app2, |a, _| a.debug_user_voice_count()),
        1,
        "重新打开工程后用户音色应还在"
    );
    let _ = std::fs::remove_file(&path);
}

#[gpui::test]
fn voice_edits_are_written_to_library_file(cx: &mut gpui::TestAppContext) {
    let voices_path = temp_voices_path("write");
    let _ = std::fs::remove_file(&voices_path);
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    set_voices_path(cx, &app, voices_path.clone());
    open_panel(cx);
    click_element(cx, "pv-new");
    assert!(voices_path.exists(), "用户音色应写入音色库文件");
    let lib = sonora_engine::VoiceLibrary::load(&voices_path);
    let custom: Vec<_> = lib.all().iter().filter(|v| !v.builtin).collect();
    assert_eq!(custom.len(), 1, "文件里应有 1 个用户音色");
    assert_eq!(custom[0].name, "我的音色 1");
    assert!(
        custom[0].id >= sonora_core::USER_ID_BASE,
        "用户音色 id 应从 {} 起",
        sonora_core::USER_ID_BASE
    );
    let id = custom[0].id;
    click_element(cx, "vdel-100");
    let lib = sonora_engine::VoiceLibrary::load(&voices_path);
    assert_eq!(
        lib.all().iter().filter(|v| !v.builtin).count(),
        0,
        "删除音色后文件里也应删掉（id={id}）"
    );
    let _ = std::fs::remove_file(&voices_path);
}

#[gpui::test]
fn library_file_is_restored_on_startup(cx: &mut gpui::TestAppContext) {
    let voices_path = temp_voices_path("restore");
    let _ = std::fs::remove_file(&voices_path);
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    set_voices_path(cx, &app, voices_path.clone());
    open_panel(cx);
    click_element(cx, "pv-new");
    let id = cx.read_entity(&app, |a, _| a.debug_voice_edit()).unwrap();
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app2, cx2) = cx
        .cx
        .add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx2.run_until_parked();
    cx2.update(|_window, app_cx| {
        app2.update(app_cx, |this, _| {
            this.enable_voice_persistence(voices_path.clone())
        });
    });
    cx2.run_until_parked();
    assert_eq!(
        cx2.read_entity(&app2, |a, _| a.debug_user_voice_count()),
        1,
        "重新启动后应把用户音色读回来"
    );
    assert!(
        cx2.read_entity(&app2, |a, _| a.debug_library_preset(id).attack) >= 0.0,
        "读回来的音色 id 必须还能用"
    );
    assert_eq!(
        cx2.read_entity(&app2, |a, _| a.debug_persisted_voice_count()),
        1,
        "启动后工程里也应带上用户音色"
    );
    let _ = std::fs::remove_file(&voices_path);
}
