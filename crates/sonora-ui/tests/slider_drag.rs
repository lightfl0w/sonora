use gpui::{AppContext as _, Entity, Modifiers, MouseButton, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    (app, cx)
}

fn bounds_of(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
}

fn drag_across(cx: &mut VisualTestContext, selector: &'static str, from: f32, to: f32) {
    let b = bounds_of(cx, selector);
    let y = b.center().y;
    let x_at = |t: f32| point(px(b.origin.x.as_f32() + b.size.width.as_f32() * t), y);
    cx.simulate_mouse_down(x_at(from), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for step in 1..=4 {
        let t = from + (to - from) * step as f32 / 4.0;
        cx.simulate_mouse_move(x_at(t), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(x_at(to), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn sidebar_master_drag_follows_mouse(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    drag_across(cx, "sb-音量", 0.1, 0.9);
    let master = cx.read_entity(&app, |a, _| a.debug_master());
    assert!(
        master > 0.7,
        "拖到接近右端后总音量应该跟着上去，实际 {master}"
    );
}

#[gpui::test]
fn sidebar_click_jumps_to_position(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let b = bounds_of(cx, "sb-音量");
    let target = point(
        px(b.origin.x.as_f32() + b.size.width.as_f32() * 0.25),
        b.center().y,
    );
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
    let master = cx.read_entity(&app, |a, _| a.debug_master());
    assert!(
        (master - 0.25).abs() < 0.08,
        "点 1/4 处应直接跳到 0.25，实际 {master}"
    );
}

#[gpui::test]
fn track_level_drag_follows_mouse(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    drag_across(cx, "lvl-0", 0.1, 0.8);
    let level = cx.read_entity(&app, |a, _| a.debug_track_level(0));
    assert!(
        level > 0.65,
        "拖到接近右端后轨道音量应该跟着上去，实际 {level}"
    );
}
