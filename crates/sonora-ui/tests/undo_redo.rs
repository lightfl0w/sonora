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

fn notes(app: &Entity<SonoraApp>, cx: &VisualTestContext) -> usize {
    cx.read_entity(app, |a, _| a.debug_note_count())
}

fn click(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_click(point(px(x), px(y)), Modifiers::none());
    cx.run_until_parked();
}

fn keystrokes(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn bounds_of(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
}

#[gpui::test]
fn creating_note_can_be_undone_and_redone(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = notes(&app, cx);
    click(cx, 500.0, 500.0);
    assert_eq!(notes(&app, cx), before + 1, "点击应新建一个音符");
    assert!(cx.read_entity(&app, |a, _| a.debug_can_undo()));
    keystrokes(cx, "ctrl-z");
    assert_eq!(notes(&app, cx), before, "撤销后音符应消失");
    assert!(cx.read_entity(&app, |a, _| a.debug_can_redo()));
    keystrokes(cx, "ctrl-shift-z");
    assert_eq!(notes(&app, cx), before + 1, "重做后音符应回来");
}

#[gpui::test]
fn dragging_note_is_a_single_undo_step(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click(cx, 500.0, 500.0);
    let created = cx
        .read_entity(&app, |a, _| a.debug_last_note())
        .expect("应有新建的音符");
    let steps_after_create = cx.read_entity(&app, |a, _| a.debug_undo_len());
    cx.simulate_mouse_down(
        point(px(500.0), px(500.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    for x in [524.0, 548.0, 572.0, 596.0] {
        cx.simulate_mouse_move(
            point(px(x), px(474.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(
        point(px(596.0), px(474.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert_ne!(
        cx.read_entity(&app, |a, _| a.debug_last_note()),
        Some(created),
        "拖动后音符位置应改变"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_undo_len()),
        steps_after_create + 1,
        "一次拖拽只能记一步历史（不能每个鼠标移动都记）"
    );
    keystrokes(cx, "ctrl-z");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_last_note()),
        Some(created),
        "撤销拖动应回到原来的位置"
    );
}

#[gpui::test]
fn slider_drag_is_a_single_undo_step(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx.read_entity(&app, |a, _| a.debug_master());
    let steps = cx.read_entity(&app, |a, _| a.debug_undo_len());
    let b = bounds_of(cx, "sb-音量");
    let y = b.center().y;
    let x_at = |t: f32| point(px(b.origin.x.as_f32() + b.size.width.as_f32() * t), y);
    cx.simulate_mouse_down(x_at(0.1), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for t in [0.3, 0.5, 0.7, 0.9] {
        cx.simulate_mouse_move(x_at(t), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(x_at(0.9), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_undo_len()),
        steps + 1,
        "一次拖动条拖动只能记一步历史"
    );
    keystrokes(cx, "ctrl-z");
    let restored = cx.read_entity(&app, |a, _| a.debug_master());
    assert!(
        (restored - before).abs() < 0.02,
        "撤销后总音量应回到 {before}，实际 {restored}"
    );
}

#[gpui::test]
fn edit_menu_undo_redo_works(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = notes(&app, cx);
    click(cx, 500.0, 500.0);
    assert_eq!(notes(&app, cx), before + 1);
    let b = bounds_of(cx, "menu-编辑").center();
    cx.simulate_click(b, Modifiers::none());
    cx.run_until_parked();
    let undo = bounds_of(cx, "mi-undo").center();
    cx.simulate_click(undo, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(notes(&app, cx), before, "菜单里的撤销应生效");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("已撤销")
    );
    let b = bounds_of(cx, "menu-编辑").center();
    cx.simulate_click(b, Modifiers::none());
    cx.run_until_parked();
    let redo = bounds_of(cx, "mi-redo").center();
    cx.simulate_click(redo, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(notes(&app, cx), before + 1, "菜单里的重做应生效");
}

#[gpui::test]
fn undo_with_empty_history_reports_and_keeps_state(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    assert!(!cx.read_entity(&app, |a, _| a.debug_can_undo()));
    keystrokes(cx, "ctrl-z");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("没有可撤销"),
        "空历史时应有提示而不是 panic"
    );
}

#[gpui::test]
fn new_edit_after_undo_clears_redo(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click(cx, 500.0, 500.0);
    keystrokes(cx, "ctrl-z");
    assert!(cx.read_entity(&app, |a, _| a.debug_can_redo()));
    click(cx, 700.0, 500.0);
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_can_redo()),
        "撤销后产生新改动应清空重做栈"
    );
}

#[gpui::test]
fn voice_creation_can_be_undone(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let menu = bounds_of(cx, "menu-视图").center();
    cx.simulate_click(menu, Modifiers::none());
    cx.run_until_parked();
    let item = bounds_of(cx, "mi-toggle-voices").center();
    cx.simulate_click(item, Modifiers::none());
    cx.run_until_parked();
    let new_btn = bounds_of(cx, "pv-new").center();
    cx.simulate_click(new_btn, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_user_voice_count()), 1);
    keystrokes(cx, "escape");
    keystrokes(cx, "ctrl-z");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_user_voice_count()),
        0,
        "撤销「新建音色」应把用户音色也撤掉"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_persisted_voice_count()),
        0,
        "工程里持久化的音色也应回退"
    );
}
