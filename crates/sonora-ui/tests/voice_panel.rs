use gpui::{AppContext as _, Entity, Modifiers, Pixels, Point, VisualTestContext, point, px};
use sonora_core::Song;
use sonora_core::USER_ID_BASE;
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    let project = Project::with_song(Song::example(), sonora_core::Scale::default());
    let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
    cx.run_until_parked();
    (app, cx)
}

fn bounds_of(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
}

fn click_element(cx: &mut VisualTestContext, selector: &'static str) {
    let center: Point<Pixels> = bounds_of(cx, selector).center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

fn click_at_fraction(cx: &mut VisualTestContext, selector: &'static str, t: f32) {
    let b = bounds_of(cx, selector);
    let target = point(
        px(b.origin.x.as_f32() + b.size.width.as_f32() * t),
        b.center().y,
    );
    cx.simulate_click(target, Modifiers::none());
    cx.run_until_parked();
}

fn open_panel(cx: &mut VisualTestContext, app: &Entity<SonoraApp>) {
    click_element(cx, "menu-视图");
    click_element(cx, "mi-toggle-voices");
    assert!(
        cx.read_entity(app, |a, _| a.debug_voice_panel_open()),
        "视图 → 音色管理 应展开浮层"
    );
}

#[gpui::test]
fn menu_opens_voice_panel_with_current_track_voice(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    let track_voice = cx
        .read_entity(&app, |a, _| a.debug_track_voice(0))
        .expect("示例工程应有一条轨道");
    open_panel(cx, &app);
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_edit()),
        Some(track_voice),
        "展开时应自动选中当前轨道的音色"
    );
    let draft = cx.read_entity(&app, |a, _| a.debug_voice_draft()).unwrap();
    let lib = cx.read_entity(&app, |a, _| a.debug_library_preset(track_voice));
    assert_eq!(draft.count, lib.count, "草稿应取自音色库里的预设");
    click_element(cx, "voice-close");
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_voice_panel_open()),
        "✕ 应关闭浮层"
    );
}

#[gpui::test]
fn selecting_voice_loads_its_preset(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "vitem-2");
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_voice_edit()), Some(2));
    let draft = cx.read_entity(&app, |a, _| a.debug_voice_draft()).unwrap();
    let lib = cx.read_entity(&app, |a, _| a.debug_library_preset(2));
    assert_eq!(draft.count, lib.count, "切换音色后草稿应换成该音色的预设");
    assert!((draft.attack - lib.attack).abs() < 1e-6);
}

#[gpui::test]
fn clicking_attack_slider_edits_draft(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    let before = cx
        .read_entity(&app, |a, _| a.debug_voice_draft())
        .unwrap()
        .attack;
    click_at_fraction(cx, "vp-起音", 0.9);
    let after = cx
        .read_entity(&app, |a, _| a.debug_voice_draft())
        .unwrap()
        .attack;
    assert!(
        after > before + 1.0,
        "把「起音」点到 90% 应该把 attack 拉高（{before} -> {after}）"
    );
}

#[gpui::test]
fn plus_button_appends_partial_and_cross_removes_it(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "vitem-0");
    let before = cx
        .read_entity(&app, |a, _| a.debug_voice_draft())
        .unwrap()
        .count;
    assert!(
        cx.debug_bounds("pv-add").is_some(),
        "分音区右上角应有 + 按钮"
    );
    click_element(cx, "pv-add");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_draft())
            .unwrap()
            .count,
        before + 1,
        "点 + 应追加一个分音"
    );
    click_element(cx, "pv-del-3");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_draft())
            .unwrap()
            .count,
        before,
        "点 × 应删除该路分音"
    );
}

#[gpui::test]
fn plus_button_stops_at_max_partials(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "vitem-0");
    for _ in 0..sonora_engine::MAX_PARTIALS + 2 {
        if cx.debug_bounds("pv-add").is_none() {
            break;
        }
        click_element(cx, "pv-add");
    }
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_voice_draft())
            .unwrap()
            .count,
        sonora_engine::MAX_PARTIALS,
        "分音数量不能超过上限"
    );
}

#[gpui::test]
fn new_and_save_persist_user_voice(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_user_voice_count()), 0);
    click_element(cx, "pv-new");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_user_voice_count()),
        1,
        "「新建」应增加一个用户音色"
    );
    let id = cx
        .read_entity(&app, |a, _| a.debug_voice_edit())
        .expect("新建后应自动切到新音色");
    assert!(id >= USER_ID_BASE, "用户音色 id 应从 {USER_ID_BASE} 起");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_persisted_voice_count()),
        1,
        "用户音色应写回工程（随工程保存）"
    );
    click_at_fraction(cx, "vp-截止", 0.5);
    let edited = cx
        .read_entity(&app, |a, _| a.debug_voice_draft())
        .unwrap()
        .cutoff;
    click_element(cx, "pv-save");
    let saved = cx.read_entity(&app, |a, _| a.debug_library_preset(id).cutoff);
    assert!(
        (saved - edited).abs() < 1.0,
        "保存后音色库里应是编辑过的值（{saved} vs {edited}）"
    );
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("已保存"),
        "状态栏应提示已保存"
    );
}

#[gpui::test]
fn saving_builtin_is_rejected(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "vitem-0");
    let before = cx.read_entity(&app, |a, _| a.debug_library_preset(0).attack);
    click_at_fraction(cx, "vp-起音", 0.9);
    click_element(cx, "pv-save");
    let after = cx.read_entity(&app, |a, _| a.debug_library_preset(0).attack);
    assert!((after - before).abs() < 1e-6, "内置音色不可被覆盖");
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("内置"),
        "应提示内置音色不可修改"
    );
}

#[gpui::test]
fn save_as_creates_another_user_voice(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "vitem-1");
    click_element(cx, "pv-saveas");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_user_voice_count()),
        1,
        "「另存为」应新建一个用户音色"
    );
}

#[gpui::test]
fn deleting_user_voice_remaps_tracks(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "pv-new");
    let id = cx.read_entity(&app, |a, _| a.debug_voice_edit()).unwrap();
    click_element(cx, "voice-close");
    click_element(cx, "vsel0");
    click_element(cx, "voice-menu-0-100");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_voice(0)),
        Some(id),
        "轨道应已切到用户音色"
    );
    open_panel(cx, &app);
    click_element(cx, "vdel-100");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_user_voice_count()),
        0,
        "用户音色应被删除"
    );
    let editing = cx.read_entity(&app, |a, _| a.debug_voice_edit());
    assert!(
        editing.is_some_and(|id| id != 100),
        "删除后编辑目标应切到仍然存在的音色，实际 {editing:?}"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_voice(0)),
        Some(0),
        "仍在用该音色的轨道应退回内置钢琴"
    );
}

#[gpui::test]
fn escape_closes_voice_panel(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_voice_panel_open()),
        "Esc 应关闭音色管理"
    );
}

#[gpui::test]
fn clicking_mask_closes_panel_but_card_does_not(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    let card = bounds_of(cx, "voice-card");
    let inside = point(
        px(card.origin.x.as_f32() + 200.0),
        px(card.origin.y.as_f32() + card.size.height.as_f32() / 2.0),
    );
    cx.simulate_click(inside, Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.read_entity(&app, |a, _| a.debug_voice_panel_open()),
        "点卡片内部不应该关闭浮层"
    );
    cx.simulate_click(point(px(20.0), px(1060.0)), Modifiers::none());
    cx.run_until_parked();
    assert!(
        !cx.read_entity(&app, |a, _| a.debug_voice_panel_open()),
        "点遮罩应关闭浮层"
    );
}

#[gpui::test]
fn audition_click_reports_result(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    click_element(cx, "pv-audition");
    let status = cx.read_entity(&app, |a, _| a.debug_status().to_string());
    let has_audio = cx.read_entity(&app, |a, _| a.has_audio());
    if has_audio {
        assert!(
            status.contains("试听"),
            "有音频设备时应提示试听中，实际 {status}"
        );
    } else {
        assert!(
            status.contains("音频不可用"),
            "没有音频设备时必须明确提示，而不是假装在试听，实际 {status}"
        );
    }
}

#[gpui::test]
fn panel_warns_when_there_is_no_audio_device(cx: &mut gpui::TestAppContext) {
    let (app, cx) = setup(cx);
    open_panel(cx, &app);
    let has_audio = cx.read_entity(&app, |a, _| a.has_audio());
    let warned = cx.debug_bounds("pv-no-audio").is_some();
    assert_eq!(
        warned, !has_audio,
        "没有音频设备时面板里必须提示「试听不会出声」"
    );
}
