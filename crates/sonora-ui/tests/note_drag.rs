use gpui::{AppContext as _, Entity, Modifiers, MouseButton, VisualTestContext, point, px};
use sonora_core::{Scale, Song};
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::empty(), Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    cx.simulate_mouse_move(point(px(5.0), px(5.0)), None, Modifiers::none());
    cx.run_until_parked();
    (app, cx)
}

fn canvas(app: &Entity<SonoraApp>, cx: &VisualTestContext) -> (f32, f32) {
    let (x, _, w) = cx.read_entity(app, |a, _| a.debug_canvas_rect());
    let bars = cx.read_entity(app, |a, _| a.debug_song_bars()) as f32;
    let zoom = cx.read_entity(app, |a, _| a.debug_zoom());
    (x, w / (bars / zoom).max(0.001))
}

fn last_note(cx: &VisualTestContext, app: &Entity<SonoraApp>) -> (u32, f32, u8, f32) {
    cx.read_entity(app, |a, _| a.debug_last_note())
        .expect("轨道上应有音符")
}

fn create_note(
    cx: &mut VisualTestContext,
    app: &Entity<SonoraApp>,
    x: f32,
    y: f32,
) -> (u32, f32, u8, f32) {
    cx.simulate_click(point(px(x), px(y)), Modifiers::none());
    cx.run_until_parked();
    last_note(cx, app)
}

fn drag_note(cx: &mut VisualTestContext, app: &Entity<SonoraApp>, index: usize, dx: f32) {
    let (gx, gy) = cx
        .read_entity(app, |a, _| a.debug_note_center(index))
        .expect("音符应能定位到窗口坐标");
    let grab = point(px(gx), px(gy));
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for step in 1..=4 {
        let target = point(px(gx + dx * step as f32 / 4.0), px(gy));
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }
    let target = point(px(gx + dx), px(gy));
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn grabbing_a_note_does_not_jump(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, _) = canvas(&app, cx);
    let y = 520.0;
    let created = create_note(cx, &app, canvas_x + 60.0, y);
    drag_note(cx, &app, 0, 2.0);
    assert_eq!(
        last_note(cx, &app),
        created,
        "按下音符时不应产生位移（旧实现会按拍数把小节位置算错 4 倍）"
    );
}

#[gpui::test]
fn dragging_one_bar_moves_exactly_one_bar(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, bar_w) = canvas(&app, cx);
    let y = 520.0;
    let created = create_note(cx, &app, canvas_x + 60.0, y);
    let (bar, beat, midi, len) = created;
    drag_note(cx, &app, 0, bar_w);
    let (bar2, beat2, midi2, len2) = last_note(cx, &app);
    assert_eq!(
        (bar2, beat2),
        (bar + 1, beat),
        "鼠标右移一个小节，音符应正好移动一个小节（{bar}/{beat} → {bar2}/{beat2}）"
    );
    assert_eq!(midi2, midi, "纵向没有位移，音高不应改变");
    assert!((len2 - len).abs() < 1e-4, "移动不应改变音长");
}

#[gpui::test]
fn dragging_one_beat_moves_exactly_one_beat(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, bar_w) = canvas(&app, cx);
    let y = 520.0;
    let created = create_note(cx, &app, canvas_x + 60.0, y);
    let (bar, beat, midi, _len) = created;
    drag_note(cx, &app, 0, bar_w / 4.0);
    let (bar2, beat2, midi2, _) = last_note(cx, &app);
    assert_eq!(
        (bar2, beat2, midi2),
        (bar, beat + 1.0, midi),
        "右移 1/4 小节应只前进一拍（{bar}/{beat} → {bar2}/{beat2}）"
    );
}
