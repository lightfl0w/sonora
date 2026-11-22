use gpui::{AppContext as _, Entity, Modifiers, Pixels, Point, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;
use sonora_ui::titlebar::Menu;

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    (app, cx)
}

fn set_popups(
    cx: &mut VisualTestContext,
    app: &Entity<SonoraApp>,
    menu: Option<Menu>,
    add_menu: bool,
    voice_pick: Option<usize>,
) {
    cx.update(|_window, app_cx| {
        app.update(app_cx, |this, cx| {
            this.debug_set_popups(menu, add_menu, voice_pick, cx)
        });
    });
    cx.run_until_parked();
}

fn click(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_click(point(px(x), px(y)), Modifiers::none());
    cx.run_until_parked();
}

fn click_element(cx: &mut VisualTestContext, selector: &'static str) {
    let center: Point<Pixels> = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

fn click_outside(cx: &mut VisualTestContext) {
    click(cx, 700.0, 500.0);
}

#[gpui::test]
fn clicking_outside_closes_titlebar_menu(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    set_popups(cx, &app, Some(Menu::File), false, None);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_menu()),
        Some(Menu::File),
        "前置条件：菜单应处于展开状态"
    );
    click_outside(cx);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_menu()),
        None,
        "点到别处后标题栏菜单必须自动收起"
    );
}

#[gpui::test]
fn clicking_outside_closes_add_menu(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    set_popups(cx, &app, None, true, None);
    click_outside(cx);
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_add_menu_open()),
        "点到别处后「添加乐器」浮层必须自动收起"
    );
}

#[gpui::test]
fn clicking_outside_closes_voice_pick(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    set_popups(cx, &app, None, false, Some(0));
    click_outside(cx);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_pick()),
        None,
        "点到别处后音色挑选浮层必须自动收起"
    );
}

#[gpui::test]
fn clicking_outside_closes_every_overlay_at_once(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    set_popups(cx, &app, Some(Menu::Edit), true, Some(0));
    click_outside(cx);
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_open_menu()), None);
    assert!(!cx.read_entity(&app, |a, _| a.debug_add_menu_open()));
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_voice_pick()), None);
}

#[gpui::test]
fn escape_closes_popups(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    set_popups(cx, &app, Some(Menu::File), true, None);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_open_menu()), None);
    assert!(!cx.read_entity(&app, |a, _| a.debug_add_menu_open()));
}

#[gpui::test]
fn titlebar_menu_toggles_switches_and_runs_item(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click_element(cx, "menu-文件");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_menu()),
        Some(Menu::File),
        "点菜单标签应展开菜单"
    );
    click_element(cx, "menu-编辑");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_menu()),
        Some(Menu::Edit),
        "点另一个菜单标签应切到那个菜单"
    );
    click_element(cx, "mi-undo");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_menu()),
        None,
        "选中菜单项后应自动收起"
    );
    click_element(cx, "menu-文件");
    click_element(cx, "menu-文件");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_menu()),
        None,
        "再点一次同一个菜单标签应收起"
    );
}

#[gpui::test]
fn add_menu_opens_toggles_and_adds_voice(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let tracks_before = cx.read_entity(&app, |a, _| a.debug_track_count());
    click_element(cx, "add-track");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_add_menu_open()),
        "点「添加乐器」应展开浮层"
    );
    click_element(cx, "add-track");
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_add_menu_open()),
        "再点一次「添加乐器」应收起浮层"
    );
    click_element(cx, "add-track");
    click_element(cx, "add-voice-0");
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_add_menu_open()),
        "选中乐器后应自动收起"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_count()),
        tracks_before + 1,
        "选中乐器应新增一条轨道"
    );
}

#[gpui::test]
fn voice_pick_opens_toggles_and_picks(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    click_element(cx, "vsel0");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_pick()),
        Some(0),
        "点「音」应展开音色挑选浮层"
    );
    click_element(cx, "vsel0");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_pick()),
        None,
        "再点一次「音」应收起浮层"
    );
    click_element(cx, "vsel0");
    click_element(cx, "voice-menu-0-0");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_pick()),
        None,
        "选中音色后应自动收起"
    );
}

#[gpui::test]
fn toolbar_dropdown_picks_option_and_closes(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let before = cx.read_entity(&app, |a, _| a.debug_grid_beats());
    click_element(cx, "dd-tb-grid");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_open_dropdown())
            .is_some(),
        "点网格下拉框应展开列表"
    );
    click_element(cx, "dd-opt-tb-grid-1");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_open_dropdown()),
        None,
        "选中下拉项后应自动收起"
    );
    assert_ne!(
        cx.read_entity(&app, |a, _| a.debug_grid_beats()),
        before,
        "点第二项应真的改掉网格值（而不是被「点别处收起」吃掉这次点击）"
    );
}
