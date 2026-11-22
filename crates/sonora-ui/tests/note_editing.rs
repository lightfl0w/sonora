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

fn click(cx: &mut VisualTestContext, x: f32, y: f32, modifiers: Modifiers) {
    cx.simulate_click(point(px(x), px(y)), modifiers);
    cx.run_until_parked();
}

fn keys(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn note_count(cx: &VisualTestContext, app: &Entity<SonoraApp>) -> usize {
    cx.read_entity(app, |a, _| a.debug_note_count())
}

fn create_two_notes(cx: &mut VisualTestContext, app: &Entity<SonoraApp>) -> f32 {
    let (canvas_x, _, _) = cx.read_entity(app, |a, _| a.debug_canvas_rect());
    let y = 500.0;
    click(cx, canvas_x + 40.0, y, Modifiers::none());
    click(cx, canvas_x + 240.0, y, Modifiers::none());
    assert_eq!(note_count(cx, app), 2, "应创建两个音符");
    y
}

fn note_center(cx: &VisualTestContext, app: &Entity<SonoraApp>, i: usize) -> (f32, f32) {
    cx.read_entity(app, |a, _| a.debug_note_center(i))
        .expect("音符应能取到屏幕坐标")
}

#[gpui::test]
fn clicking_selects_and_shift_click_adds(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    create_two_notes(cx, &app);
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()), vec![1]);
    let (x0, y0) = note_center(&cx, &app, 0);
    click(cx, x0, y0, Modifiers::none());
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()), vec![0]);
    let (x1, y1) = note_center(&cx, &app, 1);
    click(
        cx,
        x1,
        y1,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    let mut selected = cx.read_entity(&app, |a, _| a.debug_selected());
    selected.sort_unstable();
    assert_eq!(selected, vec![0, 1], "Shift 点选应追加到选择里");
    keys(cx, "escape");
    assert!(cx.read_entity(&app, |a, _| a.debug_selected()).is_empty());
}

#[gpui::test]
fn delete_and_nudge_and_transpose(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    click(cx, canvas_x + 40.0, 500.0, Modifiers::none());
    let before = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap();
    keys(cx, "right");
    let after = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap();
    assert!(
        after.1 > before.1 && (after.1 - before.1 - 0.25).abs() < 1e-4,
        "右移应前进一个栅格：{} → {}",
        before.1,
        after.1
    );
    keys(cx, "left");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().1,
        before.1
    );
    keys(cx, "up");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().2,
        before.2 + 1
    );
    keys(cx, "down down");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().2,
        before.2 - 1
    );
    keys(cx, "delete");
    assert_eq!(note_count(&cx, &app), 0);
    assert!(cx.read_entity(&app, |a, _| a.debug_selected()).is_empty());
    keys(cx, "ctrl-z");
    assert_eq!(note_count(&cx, &app), 1, "撤销应恢复被删的音符");
}

#[gpui::test]
fn duplicate_and_select_all(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    create_two_notes(cx, &app);
    keys(cx, "ctrl-a");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()).len(), 2);
    keys(cx, "escape");
    let (x0, y0) = note_center(&cx, &app, 0);
    click(cx, x0, y0, Modifiers::none());
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()), vec![0]);
    let original = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap();
    keys(cx, "ctrl-d");
    assert_eq!(note_count(&cx, &app), 3, "Ctrl+D 应复制出 1 个音符");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_selected()),
        vec![2],
        "复制后应选中新副本"
    );
    let copy = cx.read_entity(&app, |a, _| a.debug_note(2)).unwrap();
    assert!(copy.1 > original.1, "副本应在原音符之后");
    assert_eq!(copy.2, original.2);
}

#[gpui::test]
fn velocity_slider_sets_selected_notes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    click(cx, canvas_x + 40.0, 500.0, Modifiers::none());
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()), vec![0]);
    let slider = cx
        .debug_bounds("sel-velocity")
        .expect("选中音符后底栏应有力度拖动条");
    let target = point(
        px(slider.origin.x.as_f32() + slider.size.width.as_f32() * 0.5),
        slider.center().y,
    );
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
    let vel = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().4;
    assert!(
        (vel - 0.525).abs() < 0.08,
        "点力度条中间应把力度设到约 0.52，实际 {vel}"
    );
}

#[gpui::test]
fn dragging_does_not_move_unselected_notes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    create_two_notes(cx, &app);
    let (x0, y0) = note_center(&cx, &app, 0);
    click(cx, x0, y0, Modifiers::none());
    let (canvas_x, bar_w) = {
        let (x, _, w) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
        let bars = cx.read_entity(&app, |a, _| a.debug_song_bars()) as f32;
        let zoom = cx.read_entity(&app, |a, _| a.debug_zoom());
        (x, w / (bars / zoom))
    };
    let _ = canvas_x;
    let second_before = cx.read_entity(&app, |a, _| a.debug_note(1)).unwrap();
    cx.simulate_mouse_down(point(px(x0), px(y0)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for step in 1..=4 {
        cx.simulate_mouse_move(
            point(px(x0 + bar_w * step as f32 / 4.0), px(y0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(
        point(px(x0 + bar_w), px(y0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_ne!(
        cx.read_entity(&app, |a, _| a.debug_note(0)),
        None,
        "被拖的音符应仍在"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_note(1)),
        Some(second_before),
        "没被按住的音符不应跟着移动（当前只拖动按下的那一个）"
    );
}
