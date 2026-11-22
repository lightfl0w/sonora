use gpui::{AppContext as _, Entity, Modifiers, MouseButton, VisualTestContext, point, px};
use sonora_core::{Scale, Song};
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    (app, cx)
}

fn keys(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(keys);
    cx.run_until_parked();
}

fn click_element(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn shift_drag_on_ruler_sets_the_loop_region(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let ruler = cx
        .debug_bounds("roll-ruler")
        .expect("标尺应能定位（首帧后）");
    let y = ruler.center().y;
    let x_at = |t: f32| {
        point(
            px(ruler.origin.x.as_f32() + ruler.size.width.as_f32() * t),
            y,
        )
    };
    cx.simulate_mouse_down(
        x_at(0.20),
        MouseButton::Left,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    cx.run_until_parked();
    for t in [0.30, 0.40, 0.55] {
        cx.simulate_mouse_move(
            x_at(t),
            MouseButton::Left,
            Modifiers {
                shift: true,
                ..Modifiers::none()
            },
        );
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(
        x_at(0.55),
        MouseButton::Left,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    cx.run_until_parked();
    let (a, b) = cx
        .read_entity(&app, |a, _| a.debug_loop())
        .expect("Shift 拖标尺应划出循环区间");
    assert!(b > a, "区间应是正的：{a}..{b}");
    let bars = cx.read_entity(&app, |a, _| a.debug_song_bars()) as f32;
    let zoom = cx.read_entity(&app, |a, _| a.debug_zoom());
    let visible = bars / zoom;
    let expected_a = 0.20 * visible;
    let expected_b = 0.55 * visible;
    assert!(
        (a - expected_a).abs() < 0.3 && (b - expected_b).abs() < 0.3,
        "循环区间应大致等于拖拽范围：期望 {expected_a:.2}..{expected_b:.2}，实际 {a}..{b}"
    );
}

fn setup_empty(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::empty(), Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    cx.simulate_mouse_move(point(px(5.0), px(5.0)), None, Modifiers::none());
    cx.run_until_parked();
    (app, cx)
}

#[gpui::test]
fn loop_switch_and_l_key_toggle(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    assert!(!cx.read_entity(&app, |a, _| a.debug_loop_enabled()));
    click_element(cx, "tb-loop");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_loop_enabled()),
        "点循环开关应打开循环"
    );
    assert!(
        cx.read_entity(&app, |a, _| a.debug_loop()).is_some(),
        "没有区间时应自动给一个默认区间"
    );
    keys(cx, "l");
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_loop_enabled()),
        "再按 L 应关掉循环"
    );
}

#[gpui::test]
fn copy_and_paste_notes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup_empty(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    cx.simulate_click(point(px(canvas_x + 40.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()), vec![0]);
    keys(cx, "ctrl-c");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_clipboard_len()), 1);
    keys(cx, "ctrl-v");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_note_count()),
        2,
        "粘贴后应多一个音符"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_selected()),
        vec![1],
        "粘贴出的音符应被选中"
    );
    let original = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap();
    let pasted = cx.read_entity(&app, |a, _| a.debug_note(1)).unwrap();
    assert_eq!(pasted.2, original.2, "粘贴应保持音高");
    assert_eq!(pasted.3, original.3, "粘贴应保持音长");
}

#[gpui::test]
fn quantize_snaps_selected_notes_to_the_grid(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup_empty(cx);
    cx.update(|_window, app_cx| {
        app.update(app_cx, |this, _| this.debug_add_note(1, 1.1, 60, 0.5, 0.8));
    });
    cx.run_until_parked();
    assert!((cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().1 - 1.1).abs() < 1e-5);
    keys(cx, "ctrl-q");
    let beat = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().1;
    assert!(
        (beat - 1.0).abs() < 1e-4,
        "量化应把 1.1 拍对齐到 1.0 拍，实际 {beat}"
    );
    keys(cx, "ctrl-z");
    let back = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap().1;
    assert!((back - 1.1).abs() < 1e-4, "量化应可撤销");
}
