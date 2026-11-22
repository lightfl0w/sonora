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

fn toggle_from_menu(cx: &mut VisualTestContext) {
    click_element(cx, "menu-视图");
    click_element(cx, "mi-toggle-wave");
}

#[gpui::test]
fn wave_sidebar_is_visible_by_default(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    assert!(
        cx.read_entity(&app, |a, _| a.debug_wave_sidebar_open()),
        "波形侧栏默认展开"
    );
    assert!(cx.debug_bounds("wave-sidebar").is_some(), "应画出波形侧栏");
    assert!(
        cx.debug_bounds("wave-state").is_some(),
        "侧栏里应有状态文字（播放中/待机/音频不可用）"
    );
}

#[gpui::test]
fn view_menu_toggles_wave_sidebar(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    toggle_from_menu(cx);
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_wave_sidebar_open()),
        "菜单应能收起波形侧栏"
    );
    assert!(
        cx.debug_bounds("wave-sidebar").is_none(),
        "收起后不应再占位"
    );
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("波形侧栏已收起")
    );
    toggle_from_menu(cx);
    assert!(
        cx.read_entity(&app, |a, _| a.debug_wave_sidebar_open()),
        "再点一次应重新展开"
    );
    assert!(cx.debug_bounds("wave-sidebar").is_some());
}

#[gpui::test]
fn roll_still_works_next_to_sidebar(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    let before = cx.read_entity(&app, |a, _| a.debug_note_count());
    cx.simulate_click(point(px(canvas_x + 30.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.read_entity(&app, |a, _| a.debug_note_count()) > before,
        "波形侧栏展开时钢琴窗仍应可编辑"
    );
}
