use gpui::{AppContext as _, Entity, Modifiers, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::{BPM_MAX, BPM_MIN, SonoraApp};

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

fn type_keys(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

#[gpui::test]
fn clicking_bpm_starts_editing(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let bpm = cx.read_entity(&app, |a, _| a.debug_song_bpm());
    click_element(cx, "tb-bpm");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_bpm_editing()),
        "点击 BPM 字段应进入编辑状态"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_bpm_buffer().to_string()),
        format!("{bpm:.0}"),
        "进入编辑时应把当前速度填进输入框"
    );
}

#[gpui::test]
fn typing_and_enter_sets_the_tempo(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click_element(cx, "tb-bpm");
    type_keys(cx, "backspace backspace backspace backspace");
    type_keys(cx, "1 4 0");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_bpm_buffer().to_string()),
        "140"
    );
    type_keys(cx, "enter");
    assert!(!cx.read_entity(&app, |a, _| a.debug_bpm_editing()));
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bpm()),
        140.0,
        "回车后速度应改成 140"
    );
    assert!(cx.read_entity(&app, |a, _| a.debug_is_dirty()));
}

#[gpui::test]
fn escape_cancels_without_changing_tempo(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx.read_entity(&app, |a, _| a.debug_song_bpm());
    click_element(cx, "tb-bpm");
    type_keys(cx, "backspace backspace 9 9");
    type_keys(cx, "escape");
    assert!(!cx.read_entity(&app, |a, _| a.debug_bpm_editing()));
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bpm()),
        before,
        "Esc 应放弃输入"
    );
}

#[gpui::test]
fn out_of_range_tempo_is_clamped(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click_element(cx, "tb-bpm");
    type_keys(cx, "backspace backspace backspace backspace 9 9 9");
    type_keys(cx, "enter");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_song_bpm()), BPM_MAX);
    click_element(cx, "tb-bpm");
    type_keys(cx, "backspace backspace backspace backspace 1");
    type_keys(cx, "enter");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_song_bpm()), BPM_MIN);
}

#[gpui::test]
fn arrow_keys_nudge_and_apply_immediately(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx.read_entity(&app, |a, _| a.debug_song_bpm());
    click_element(cx, "tb-bpm");
    type_keys(cx, "up up up");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bpm()),
        before + 3.0,
        "上键应每次 +1 并立刻生效"
    );
    type_keys(cx, "down down down down");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bpm()),
        before - 1.0
    );
}

#[gpui::test]
fn clicking_elsewhere_commits_the_input(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click_element(cx, "tb-bpm");
    type_keys(cx, "backspace backspace backspace backspace 1 0 0");
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    cx.simulate_click(point(px(canvas_x + 40.0), px(400.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.read_entity(&app, |a, _| a.debug_bpm_editing()));
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_song_bpm()),
        100.0,
        "点到别处应把输入框里的值提交上去"
    );
}

#[gpui::test]
fn space_while_editing_does_not_toggle_playback(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click_element(cx, "tb-bpm");
    let before = cx.read_entity(&app, |a, _| a.debug_transport().1);
    type_keys(cx, "space");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_transport().1),
        before,
        "编辑 BPM 时空格不应触发播放"
    );
    assert!(cx.read_entity(&app, |a, _| a.debug_bpm_editing()));
}
