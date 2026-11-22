use gpui::{AppContext as _, Entity, Modifiers, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
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

fn type_text(cx: &mut VisualTestContext, text: &str) {
    let keys: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    cx.simulate_keystrokes(&keys.join(" "));
    cx.run_until_parked();
}

fn clear_buffer(cx: &mut VisualTestContext, app: &Entity<SonoraApp>) {
    let len = cx
        .read_entity(app, |a, _| a.debug_rename_buffer())
        .chars()
        .count();
    for _ in 0..len {
        cx.simulate_keystrokes("backspace");
    }
    cx.run_until_parked();
}

#[gpui::test]
fn rename_fills_buffer_and_commits_on_enter(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx
        .read_entity(&app, |a, _| a.debug_track_name(0))
        .expect("示例工程应有轨道");
    click_element(cx, "ren0");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_renaming()), Some(0));
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_rename_buffer()),
        before,
        "进入改名时应把当前名字填进输入框"
    );
    clear_buffer(cx, &app);
    type_text(cx, "主旋律");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_rename_buffer()),
        "主旋律"
    );
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_renaming()), None);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_name(0)),
        Some("主旋律".to_string()),
        "回车后轨道名应更新"
    );
    assert!(cx.read_entity(&app, |a, _| a.debug_is_dirty()));
}

#[gpui::test]
fn rename_cancels_on_escape(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx.read_entity(&app, |a, _| a.debug_track_name(0)).unwrap();
    click_element(cx, "ren0");
    clear_buffer(cx, &app);
    type_text(cx, "不要了");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_renaming()), None);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_name(0)),
        Some(before),
        "Esc 应放弃改名"
    );
}

#[gpui::test]
fn rename_can_be_undone(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx.read_entity(&app, |a, _| a.debug_track_name(0)).unwrap();
    click_element(cx, "ren0");
    clear_buffer(cx, &app);
    type_text(cx, "临时名字");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-z");
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_name(0)),
        Some(before),
        "撤销应把名字改回去"
    );
    let _ = point(px(0.0), px(0.0));
}
