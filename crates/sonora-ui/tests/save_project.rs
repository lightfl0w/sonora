use gpui::{AppContext as _, Entity, Modifiers, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;
use sonora_ui::file_dialog::{ExitAction, PathPromptKind};

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    (app, cx)
}

fn set_path(cx: &mut VisualTestContext, app: &Entity<SonoraApp>, path: std::path::PathBuf) {
    cx.update(|_window, app_cx| {
        app.update(app_cx, |this, _| this.debug_set_path(path));
    });
    cx.run_until_parked();
}

fn set_projects_dir(cx: &mut VisualTestContext, app: &Entity<SonoraApp>, dir: std::path::PathBuf) {
    cx.update(|_window, app_cx| {
        app.update(app_cx, |this, _| this.debug_set_projects_dir(dir));
    });
    cx.run_until_parked();
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sonora-save-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

fn click_element(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

fn type_text(cx: &mut VisualTestContext, text: &str) {
    let keys: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    cx.simulate_keystrokes(&keys.join(" "));
    cx.run_until_parked();
}

fn call<R>(
    cx: &mut VisualTestContext,
    app: &Entity<SonoraApp>,
    f: impl FnOnce(&mut SonoraApp) -> R,
) -> R {
    let out = cx.update(|_window, app_cx| app.update(app_cx, |this, _| f(this)));
    cx.run_until_parked();
    out
}

#[gpui::test]
fn ctrl_s_writes_project_file(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = std::env::temp_dir().join("sonora-save-test");
    std::fs::create_dir_all(&dir).expect("创建临时目录");
    let path = dir.join(format!("save-{}.sonora", std::process::id()));
    let _ = std::fs::remove_file(&path);
    set_path(cx, &app, path.clone());
    let before = cx.read_entity(&app, |a, _| a.debug_total_notes());
    cx.simulate_click(point(px(500.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.read_entity(&app, |a, _| a.debug_is_dirty()),
        "改过内容后应处于未保存状态"
    );
    cx.simulate_keystrokes("ctrl-s");
    cx.run_until_parked();
    assert!(path.exists(), "Ctrl+S 应把工程写到当前路径");
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_is_dirty()),
        "保存后不应再标记为未保存"
    );
    let saved = Project::load(&path).expect("保存出来的工程应能重新载入");
    assert_eq!(
        saved.song.total_notes(),
        before + 1,
        "保存的文件里应包含新建的音符"
    );
    let _ = std::fs::remove_file(&path);
}

#[gpui::test]
fn ctrl_s_without_path_saves_to_the_default_folder(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("default");
    set_projects_dir(cx, &app, dir.clone());
    cx.simulate_keystrokes("ctrl-s");
    cx.run_until_parked();
    let path = cx
        .read_entity(&app, |a, _| a.debug_save_path())
        .expect("保存后应记住落盘路径");
    assert!(path.starts_with(&dir), "应落在默认目录：{}", path.display());
    assert!(path.exists(), "工程文件应真的写出来：{}", path.display());
    assert!(!cx.read_entity(&app, |a, _| a.debug_is_dirty()));
    let files_before = std::fs::read_dir(&dir).unwrap().count();
    cx.simulate_keystrokes("ctrl-s");
    cx.run_until_parked();
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), files_before);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_save_path()),
        Some(path.clone())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn open_prompt_lists_projects_and_opens_the_picked_one(cx: &mut gpui::TestAppContext) {
    let dir = temp_dir("open");
    let target = dir.join("demo.sonora");
    let mut project = Project::with_song(Song::example(), sonora_core::Scale::default());
    project.title = "demo".into();
    project.song.bars = 24;
    project.save(&target).expect("先造一个工程文件");
    let (app, cx) = setup(cx);
    set_projects_dir(cx, &app, dir.clone());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-open");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_prompt_kind()),
        Some(PathPromptKind::Open),
        "文件 → 打开工程… 应弹出打开对话框"
    );
    let candidates = cx.read_entity(&app, |a, _| a.debug_prompt_candidates());
    let index = candidates
        .iter()
        .position(|p| p == &target)
        .expect("候选列表里应有刚造的工程");
    let selector: &'static str = Box::leak(format!("prompt-file-{index}").into_boxed_str());
    click_element(cx, selector);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_save_as_buffer()),
        Some(target.display().to_string()),
        "点候选行应把路径填进输入框"
    );
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_save_path()),
        Some(target.clone()),
        "打开后当前文件应切换过去"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bars()),
        24,
        "应载入文件里的工程内容"
    );
    assert!(!cx.read_entity(&app, |a, _| a.debug_is_dirty()));
    call(cx, &app, |a| a.debug_set_dirty(true));
    click_element(cx, "menu-文件");
    click_element(cx, "mi-open");
    assert!(
        cx.debug_bounds("prompt-warning").is_some(),
        "有未保存改动时应提示打开会丢弃"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn arrow_keys_do_not_type_into_path_prompt(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("arrow-keys");
    set_projects_dir(cx, &app, dir.clone());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-open");
    let before = cx
        .read_entity(&app, |a, _| a.debug_save_as_buffer())
        .expect("打开弹窗应存在");
    cx.simulate_keystrokes("left right up down");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_save_as_buffer()),
        Some(before),
        "方向键不应把按键名写进路径输入框"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn opening_a_missing_file_reports_an_error(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("open-missing");
    set_projects_dir(cx, &app, dir.clone());
    let before_bars = cx.read_entity(&app, |a, _| a.debug_song_bars());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-open");
    let prefilled = cx
        .read_entity(&app, |a, _| a.debug_save_as_buffer())
        .unwrap();
    for _ in 0..prefilled.chars().count() {
        cx.simulate_keystrokes("backspace");
    }
    cx.run_until_parked();
    type_text(cx, &dir.join("nope.sonora").display().to_string());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("打开失败"),
        "打开不存在的文件应有明确提示"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bars()),
        before_bars,
        "打开失败不应破坏当前工程"
    );
    assert!(cx.read_entity(&app, |a, _| a.debug_save_path()).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn closing_with_unsaved_changes_asks_first(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("close");
    set_projects_dir(cx, &app, dir.clone());
    assert!(
        call(cx, &app, |a| a.debug_on_close_requested()),
        "没有未保存改动时应允许关窗"
    );
    assert!(!cx.read_entity(&app, |a, _| a.debug_exit_prompt_open()));
    assert!(!call(cx, &app, |a| {
        a.debug_set_dirty(true);
        a.debug_on_close_requested()
    }));
    assert!(
        cx.read_entity(&app, |a, _| a.debug_exit_prompt_open()),
        "有未保存改动时应弹出确认"
    );
    assert!(!call(cx, &app, |a| a.debug_apply_exit(ExitAction::Cancel)));
    assert!(!cx.read_entity(&app, |a, _| a.debug_exit_prompt_open()));
    assert!(
        std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 0,
        "取消不应写文件"
    );
    assert!(call(cx, &app, |a| a.debug_apply_exit(ExitAction::SaveAndExit)));
    assert!(!cx.read_entity(&app, |a, _| a.debug_is_dirty()));
    assert!(
        std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 1,
        "保存并退出应写出一份工程文件"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn discard_and_exit_does_not_write(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("discard");
    set_projects_dir(cx, &app, dir.clone());
    call(cx, &app, |a| a.debug_set_dirty(true));
    assert!(call(cx, &app, |a| {
        a.debug_apply_exit(ExitAction::DiscardAndExit)
    }));
    assert!(
        std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 0,
        "选择不保存退出不应写文件"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn autosave_writes_a_backup_next_to_the_project(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("autosave");
    let path = dir.join("work.sonora");
    set_path(cx, &app, path.clone());
    call(cx, &app, |a| a.debug_set_dirty(true));
    call(cx, &app, |a| a.debug_run_autosave());
    let backup = dir.join("work.autosave.sonora");
    assert!(
        backup.exists(),
        "自动备份应写在工程旁边：{}",
        backup.display()
    );
    assert!(
        cx.read_entity(&app, |a, _| a.debug_is_dirty()),
        "自动备份不能把工程标记成已保存"
    );
    let loaded = Project::load(&backup).expect("备份应能重新载入");
    assert_eq!(
        loaded.song.total_notes(),
        cx.read_entity(&app, |a, _| a.debug_total_notes())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn save_as_dialog_writes_where_typed(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("saveas");
    set_projects_dir(cx, &app, dir.clone());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-save-as");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_save_as_open()),
        "应打开另存为对话框"
    );
    let prefilled = cx
        .read_entity(&app, |a, _| a.debug_save_as_buffer())
        .expect("对话框里应有预填路径");
    for _ in 0..prefilled.chars().count() {
        cx.simulate_keystrokes("backspace");
    }
    cx.run_until_parked();
    let target = dir.join("手工保存.sonora");
    type_text(cx, &target.display().to_string());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!cx.read_entity(&app, |a, _| a.debug_save_as_open()));
    assert!(
        target.exists(),
        "应写到用户输入的路径：{}",
        target.display()
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_save_path()),
        Some(target.clone())
    );
    assert!(!cx.read_entity(&app, |a, _| a.debug_is_dirty()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn save_as_adds_extension_when_missing(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("ext");
    set_projects_dir(cx, &app, dir.clone());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-save-as");
    let prefilled = cx
        .read_entity(&app, |a, _| a.debug_save_as_buffer())
        .unwrap();
    for _ in 0..prefilled.chars().count() {
        cx.simulate_keystrokes("backspace");
    }
    cx.run_until_parked();
    type_text(cx, &dir.join("无扩展名").display().to_string());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let expected = dir.join("无扩展名.sonora");
    assert!(
        expected.exists(),
        "没写扩展名时应自动补 .sonora：{}",
        expected.display()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn save_as_escape_and_mask_click_cancel(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let dir = temp_dir("cancel");
    set_projects_dir(cx, &app, dir.clone());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-save-as");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.read_entity(&app, |a, _| a.debug_save_as_open()));
    assert!(cx.read_entity(&app, |a, _| a.debug_save_path()).is_none());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-save-as");
    cx.simulate_click(point(px(8.0), px(8.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.read_entity(&app, |a, _| a.debug_save_as_open()));
    assert!(
        std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 0,
        "取消不应写出任何文件"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
