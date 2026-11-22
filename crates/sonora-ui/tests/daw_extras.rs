use gpui::{AppContext as _, Entity, Modifiers, MouseButton, VisualTestContext, point, px};
use sonora_core::Scale;
use sonora_core::song::{Note, Song};
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn setup(
    project: Project,
) -> impl FnOnce(&mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    move |cx| {
        let (app, cx) = cx.add_window_view(move |_, cx| SonoraApp::new(project, cx));
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(5.0), px(5.0)), None, Modifiers::none());
        cx.run_until_parked();
        (app, cx)
    }
}

fn empty_setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    setup(Project::with_song(Song::empty(), Scale::default()))(cx)
}

fn example_setup(cx: &mut gpui::TestAppContext) -> (Entity<SonoraApp>, &mut VisualTestContext) {
    setup(Project::with_song(Song::example(), Scale::default()))(cx)
}

fn call<R>(
    cx: &mut VisualTestContext,
    app: &Entity<SonoraApp>,
    f: impl FnOnce(&mut SonoraApp) -> R,
) -> R {
    let out = cx.update(|_window, app_cx| {
        app.update(app_cx, |this, cx| {
            let out = f(this);
            cx.notify();
            out
        })
    });
    cx.run_until_parked();
    out
}

fn click_element(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("找不到元素 {selector}"))
        .center();
    cx.simulate_click(center, Modifiers::none());
    cx.run_until_parked();
}

fn type_text(cx: &mut VisualTestContext, text: &str) {
    let keys: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    cx.simulate_keystrokes(&keys.join(" "));
    cx.run_until_parked();
}

fn drag(cx: &mut VisualTestContext, from: (f32, f32), to: (f32, f32), modifiers: Modifiers) {
    cx.simulate_mouse_down(point(px(from.0), px(from.1)), MouseButton::Left, modifiers);
    cx.run_until_parked();
    for step in 1..=4 {
        let t = step as f32 / 4.0;
        cx.simulate_mouse_move(
            point(
                px(from.0 + (to.0 - from.0) * t),
                px(from.1 + (to.1 - from.1) * t),
            ),
            MouseButton::Left,
            modifiers,
        );
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(point(px(to.0), px(to.1)), MouseButton::Left, modifiers);
    cx.run_until_parked();
}

fn note_center(cx: &VisualTestContext, app: &Entity<SonoraApp>, i: usize) -> (f32, f32) {
    cx.read_entity(app, |a, _| a.debug_note_center(i))
        .expect("音符应能定位")
}

#[gpui::test]
fn marquee_selects_notes_inside_the_rect(cx: &mut gpui::TestAppContext) {
    let (app, cx) = empty_setup(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    cx.simulate_click(point(px(canvas_x + 40.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_click(point(px(canvas_x + 240.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_note_count()), 2);
    let (x0, y0) = note_center(&cx, &app, 0);
    let (x1, y1) = note_center(&cx, &app, 1);
    let left = x0.min(x1) - 20.0;
    let right = x0.max(x1) + 30.0;
    let top = y0.min(y1) - 14.0;
    let bottom = y0.max(y1) + 14.0;
    drag(
        cx,
        (left, top),
        (right, bottom),
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    let mut selected = cx.read_entity(&app, |a, _| a.debug_selected());
    selected.sort_unstable();
    assert_eq!(selected, vec![0, 1], "框选应选中框里的音符");
}

#[gpui::test]
fn group_drag_moves_every_selected_note(cx: &mut gpui::TestAppContext) {
    let (app, cx) = empty_setup(cx);
    let (canvas_x, _, _) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    cx.simulate_click(point(px(canvas_x + 40.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_click(point(px(canvas_x + 240.0), px(500.0)), Modifiers::none());
    cx.run_until_parked();
    let (x0, y0) = note_center(&cx, &app, 0);
    let (x1, y1) = note_center(&cx, &app, 1);
    drag(
        cx,
        (x0.min(x1) - 20.0, y0.min(y1) - 14.0),
        (x0.max(x1) + 30.0, y0.max(y1) + 14.0),
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    assert_eq!(cx.read_entity(&app, |a, _| a.debug_selected()).len(), 2);
    let before0 = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap();
    let before1 = cx.read_entity(&app, |a, _| a.debug_note(1)).unwrap();
    let (gx, gy) = note_center(&cx, &app, 0);
    let bar_w = {
        let (_, _, w) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
        let bars = cx.read_entity(&app, |a, _| a.debug_song_bars()) as f32;
        let zoom = cx.read_entity(&app, |a, _| a.debug_zoom());
        w / (bars / zoom)
    };
    drag(cx, (gx, gy), (gx + bar_w, gy), Modifiers::none());
    let after0 = cx.read_entity(&app, |a, _| a.debug_note(0)).unwrap();
    let after1 = cx.read_entity(&app, |a, _| a.debug_note(1)).unwrap();
    assert_ne!(after0, before0, "被拖的音符应移动");
    assert_ne!(after1, before1, "另一个选中的音符应跟着一起移动");
    assert_eq!(
        after0.0 as i32 - before0.0 as i32,
        after1.0 as i32 - before1.0 as i32,
        "两个音符移动的小节数应一致"
    );
}

#[gpui::test]
fn duplicate_track_copies_notes_and_selects_it(cx: &mut gpui::TestAppContext) {
    let (app, cx) = example_setup(cx);
    let before_tracks = cx.read_entity(&app, |a, _| a.debug_track_count());
    let notes = cx.read_entity(&app, |a, _| a.debug_note_count());
    click_element(cx, "dup0");
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_track_count()),
        before_tracks + 1,
        "复制轨道后轨道数应 +1"
    );
    assert_eq!(
        cx.read_entity(&app, |a, _| a.debug_note_count()),
        notes,
        "新轨道应带着同样的音符"
    );
    let name = cx
        .read_entity(&app, |a, _| a.debug_track_name(1))
        .expect("应复制到第 2 条");
    assert!(name.contains("副本"), "副本名字应能看出来：{name}");
}

#[gpui::test]
fn dragging_loop_edge_moves_that_edge(cx: &mut gpui::TestAppContext) {
    let (app, cx) = empty_setup(cx);
    call(cx, &app, |a| a.debug_set_loop(Some((1.0, 3.0)), true));
    let (canvas_x, _, w) = cx.read_entity(&app, |a, _| a.debug_canvas_rect());
    let bars = cx.read_entity(&app, |a, _| a.debug_song_bars()) as f32;
    let zoom = cx.read_entity(&app, |a, _| a.debug_zoom());
    let bar_w = w / (bars / zoom);
    let ruler = cx.debug_bounds("roll-ruler").expect("标尺");
    let y = ruler.center().y.as_f32();
    let _ = canvas_x;
    let edge_x = ruler.origin.x.as_f32() + 3.0 * bar_w;
    drag(cx, (edge_x, y), (edge_x + bar_w, y), Modifiers::none());
    let (a, b) = cx
        .read_entity(&app, |a, _| a.debug_loop())
        .expect("循环区间还在");
    assert!((a - 1.0).abs() < 0.3, "起点不应变，实际 {a}");
    assert!(b > 3.4, "右边界应被拖到更右边，实际 {b}");
}

#[gpui::test]
fn play_menu_toggles_metronome_and_count_in(cx: &mut gpui::TestAppContext) {
    let (app, cx) = empty_setup(cx);
    click_element(cx, "menu-播放");
    click_element(cx, "mi-menu-metronome");
    assert!(cx.read_entity(&app, |a, _| a.debug_metronome_on()));
    click_element(cx, "menu-播放");
    click_element(cx, "mi-menu-countin");
    assert!(cx.read_entity(&app, |a, _| a.debug_count_in_on()));
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sonora-extras-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("创建临时目录");
    dir
}

fn set_projects_dir(cx: &mut VisualTestContext, app: &Entity<SonoraApp>, dir: std::path::PathBuf) {
    cx.update(|_window, app_cx| {
        app.update(app_cx, |this, _| this.debug_set_projects_dir(dir));
    });
    cx.run_until_parked();
}

fn clear_prompt_buffer(cx: &mut VisualTestContext, app: &Entity<SonoraApp>) {
    let len = cx
        .read_entity(app, |a, _| a.debug_save_as_buffer())
        .unwrap_or_default()
        .chars()
        .count();
    for _ in 0..len {
        cx.simulate_keystrokes("backspace");
    }
    cx.run_until_parked();
}

#[gpui::test]
fn export_midi_writes_a_playable_file(cx: &mut gpui::TestAppContext) {
    let (app, cx) = example_setup(cx);
    let dir = temp_dir("midi");
    set_projects_dir(cx, &app, dir.clone());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-export-midi");
    clear_prompt_buffer(cx, &app);
    let target = dir.join("out.mid");
    type_text(cx, &target.display().to_string());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(target.exists(), "应写出 MIDI 文件");
    let parsed = sonora_midi::read_file(&target).expect("导出的 MIDI 应能解析");
    assert_eq!(
        parsed.notes.len(),
        cx.read_entity(&app, |a, _| a.debug_total_notes()),
        "导出的音符数应与工程一致"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn import_midi_replaces_the_song(cx: &mut gpui::TestAppContext) {
    let dir = temp_dir("midi-in");
    let source = dir.join("song.mid");
    let mut song = Song::empty();
    song.bpm = 100.0;
    song.bars = 8;
    song.tracks[0].notes.push(Note {
        bar: 1,
        beat: 1.0,
        midi: 67,
        len: 1.0,
        vel: 0.8,
    });
    sonora_midi::write_file(&source, &song).expect("先造一个 MIDI");
    let (app, cx) = empty_setup(cx);
    set_projects_dir(cx, &app, dir.clone());
    let before = cx.read_entity(&app, |a, _| a.debug_total_notes());
    click_element(cx, "menu-文件");
    click_element(cx, "mi-import-midi");
    clear_prompt_buffer(cx, &app);
    type_text(cx, &source.display().to_string());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_ne!(
        cx.read_entity(&app, |a, _| a.debug_total_notes()),
        before,
        "导入后歌曲内容应被替换"
    );
    assert!(
        cx.read_entity(&app, |a, _| a.debug_status().to_string())
            .contains("已导入"),
        "状态栏应提示导入成功"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[gpui::test]
fn export_wav_writes_a_riff_file(cx: &mut gpui::TestAppContext) {
    let (app, cx) = empty_setup(cx);
    let dir = temp_dir("wav");
    set_projects_dir(cx, &app, dir.clone());
    call(cx, &app, |a| {
        a.debug_add_note(1, 1.0, 60, 0.5, 0.8);
    });
    click_element(cx, "menu-文件");
    click_element(cx, "mi-export-wav");
    clear_prompt_buffer(cx, &app);
    let target = dir.join("out.wav");
    type_text(cx, &target.display().to_string());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let bytes = std::fs::read(&target).expect("应写出 WAV");
    assert_eq!(&bytes[0..4], b"RIFF", "应是合法的 WAV 头");
    assert!(bytes.len() > 44, "WAV 应有音频数据");
    let _ = std::fs::remove_dir_all(&dir);
}
