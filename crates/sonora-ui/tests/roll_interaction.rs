use gpui::{AppContext as _, Modifiers, MouseButton, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(cx: &mut gpui::TestAppContext) -> (gpui::Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    (app, cx)
}

fn notes_of(app: &gpui::Entity<SonoraApp>, cx: &VisualTestContext) -> usize {
    cx.read_entity(app, |app, _| app.debug_note_count())
}

#[gpui::test]
fn click_empty_area_creates_note(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = notes_of(&app, cx);
    assert!(before > 0, "示例工程初始应有音符");
    let target = point(px(500.0), px(500.0));
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(notes_of(&app, cx), before + 1, "在空白处点击应创建一个音符");
}

#[gpui::test]
fn drag_note_body_moves_it(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let target = point(px(500.0), px(500.0));
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
    let created = cx.read_entity(&app, |app, _| app.debug_last_note());
    let Some((bar, beat, midi, _len)) = created else {
        panic!("点击未创建音符，无法测试拖动");
    };
    cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        point(px(596.0), px(474.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    cx.simulate_mouse_up(
        point(px(596.0), px(474.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let moved = cx.read_entity(&app, |app, _| app.debug_last_note());
    let Some((bar2, beat2, midi2, _)) = moved else {
        panic!("拖动后音符丢失");
    };
    assert!(
        (bar2, beat2, midi2) != (bar, beat, midi),
        "拖动音符后位置应改变，原来 {bar}/{beat}/{midi}，现在 {bar2}/{beat2}/{midi2}"
    );
}

#[gpui::test]
fn dragging_reports_drag_state(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let target = point(px(500.0), px(500.0));
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let during = cx.read_entity(&app, |app, _| app.debug_drag_active());
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(
        during,
        "左键按住音符后应进入拖拽状态（这是移动/拉长的前提）"
    );
    assert!(
        !cx.read_entity(&app, |app, _| app.debug_drag_active()),
        "松开后拖拽状态应清除"
    );
}

#[gpui::test]
fn click_after_hover_creates_note(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = notes_of(&app, cx);
    let target = point(px(520.0), px(520.0));
    cx.simulate_mouse_move(target, None, Modifiers::none());
    cx.run_until_parked();
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(notes_of(&app, cx), before + 1, "hover 后点击也应创建音符");
}

#[gpui::test]
fn click_at_many_positions_creates_notes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = notes_of(&app, cx);
    let mut created = 0usize;
    for y in [300.0, 380.0, 460.0, 540.0, 620.0] {
        for x in [420.0, 500.0, 580.0, 660.0] {
            let p = point(px(x), px(y));
            let n = notes_of(&app, cx);
            cx.simulate_click(p, Modifiers::none());
            cx.run_until_parked();
            if notes_of(&app, cx) > n {
                created += 1;
            }
        }
    }
    assert!(
        created >= 15,
        "20 个空白位置至少应创建 15 个音符，实际 {created}"
    );
    assert!(notes_of(&app, cx) > before);
}

#[gpui::test]
fn overlapping_tracks_can_still_add_notes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let mut created = 0usize;
    for xi in 0..29 {
        for yi in 0..25 {
            let x = 420.0 + xi as f32 * 20.0;
            let y = 280.0 + yi as f32 * 20.0;
            let n = notes_of(&app, cx);
            cx.simulate_click(point(px(x), px(y)), Modifiers::none());
            cx.run_until_parked();
            if notes_of(&app, cx) > n {
                created += 1;
            }
        }
    }
    assert!(created > 50, "大量空白点击应创建多个音符，实际 {created}");
    let overlaps = cx.read_entity(&app, |app, _| app.debug_same_pitch_overlaps());
    assert_eq!(overlaps, 0, "UI 操作不应制造同音高叠音，实际 {overlaps} 处");
}

#[gpui::test]
fn clicking_black_key_rows_creates_sharp_notes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |app, _| app.debug_canvas_rect());
    let x = canvas_x + 40.0;
    let mut sharps = Vec::new();
    for yi in 0..26 {
        let y = 280.0 + yi as f32 * 7.0;
        let n = notes_of(&app, cx);
        cx.simulate_click(point(px(x), px(y)), Modifiers::none());
        cx.run_until_parked();
        if notes_of(&app, cx) > n {
            if let Some((_, _, midi, _)) = cx.read_entity(&app, |app, _| app.debug_last_note()) {
                if matches!(midi % 12, 1 | 3 | 6 | 8 | 10) {
                    sharps.push(midi);
                }
            }
        }
    }
    assert!(
        !sharps.is_empty(),
        "点击黑键行应能创建半音音符，实际一个都没建出来（黑键被吸附吞掉了）"
    );
}

#[gpui::test]
fn horizontal_scroll_goes_past_song_end(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let song_bars = cx.read_entity(&app, |app, _| app.debug_song_bars()) as f32;
    let zoom = cx.read_entity(&app, |app, _| app.debug_zoom());
    let visible = song_bars / zoom;
    cx.update_entity(&app, |app, _| {
        app.debug_set_scroll_bars(1000.0);
    });
    let scroll = cx.read_entity(&app, |app, _| app.debug_scroll_bars());
    assert!(
        scroll > song_bars - visible + 0.5,
        "应能滚过'末小节贴右边'的位置：scroll={scroll} 阈值={}",
        song_bars - visible
    );
    assert!(
        scroll <= song_bars + 0.01,
        "上限应是工程末小节位于视口左侧：scroll={scroll}"
    );
}

#[gpui::test]
fn clicking_past_song_end_creates_note_and_grows_song(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let bars_before = cx.read_entity(&app, |app, _| app.debug_song_bars());
    let zoom = cx.read_entity(&app, |app, _| app.debug_zoom());
    cx.update_entity(&app, |app, cx| {
        app.debug_set_scroll_bars_notify(bars_before as f32, cx);
    });
    cx.run_until_parked();
    let (ox, _, _) = cx.read_entity(&app, |app, _| app.debug_canvas_rect());
    let local_x = cx.read_entity(&app, |app, _| {
        app.debug_x_for_beats(bars_before as f32 * 4.0 + 4.0)
    });
    let click_x = ox + local_x;
    assert!(
        local_x > 0.0,
        "目标位置应落在画布内（local_x={local_x}，滚动到末尾后工程之外仍在右侧）"
    );
    let notes_before = notes_of(&app, cx);
    let mut created = false;
    for yi in 0..10 {
        let p = point(px(click_x), px(300.0 + yi as f32 * 30.0));
        let n = notes_of(&app, cx);
        cx.simulate_click(p, Modifiers::none());
        cx.run_until_parked();
        if notes_of(&app, cx) > n {
            created = true;
            break;
        }
    }
    assert!(
        created,
        "工程末尾之外的空白区应能创建音符（click_x={click_x}, ox={ox}, local_x={local_x}）"
    );
    assert_eq!(notes_of(&app, cx), notes_before + 1, "应恰好新增一个音符");
    let bars_after = cx.read_entity(&app, |app, _| app.debug_song_bars());
    assert!(
        bars_after > bars_before,
        "在末尾之外建音符后工程应自动延长：{bars_before} -> {bars_after}（zoom={zoom}）"
    );
}
