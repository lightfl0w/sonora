use gpui::{
    App, AppContext as _, ClickEvent, DragMoveEvent, ElementId, InteractiveElement, IntoElement,
    MouseButton, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px,
    rgb,
};

use crate::theme::{self, FONT_SM, FONT_XS};

pub const TRACK_PANEL_W: f32 = 200.0;
pub const TRACK_HEADER_H: f32 = 22.0;

#[derive(Clone, Debug)]
pub struct TrackView {
    pub name: String,
    pub color: u32,
    pub level: f32,
    pub muted: bool,
    pub solo: bool,
    pub notes: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrackAction {
    Select(usize),
    ToggleMute(usize),
    ToggleSolo(usize),
    Rename(usize),
    PickVoice(usize),
    SetVoice(usize, u32),
    Delete(usize),
    Duplicate(usize),
    Level(usize, f32),
    OpenAddMenu,
    AddVoice(u32),
    CommitRename,
}

#[derive(Clone, Debug)]
pub struct TrackListState {
    pub tracks: Vec<TrackView>,
    pub selected: usize,
    pub track_row_h: f32,
    pub add_menu_open: bool,
    pub voices: Vec<(u32, String, bool)>,
    pub renaming: Option<usize>,
    pub rename_buffer: String,
    pub voice_pick: Option<usize>,
}

#[derive(gpui::IntoElement)]
pub struct TrackList {
    state: TrackListState,
    on_action: Option<std::rc::Rc<dyn Fn(TrackAction, &mut Window, &mut App)>>,
}

impl TrackList {
    pub fn new(state: TrackListState) -> Self {
        Self {
            state,
            on_action: None,
        }
    }
    pub fn on_action(mut self, h: impl Fn(TrackAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(h));
        self
    }
}

fn tag_button(
    id: String,
    label: &'static str,
    active: bool,
    active_color: u32,
    handler: Option<std::rc::Rc<dyn Fn(TrackAction, &mut Window, &mut App)>>,
    action: TrackAction,
) -> impl IntoElement {
    let p = theme::palette();
    let mut btn = div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .w(px(15.0))
        .h(px(13.0))
        .rounded(px(3.0))
        .text_size(px(8.5))
        .cursor_pointer()
        .text_color(if active {
            rgb(0x101014)
        } else {
            rgb(p.text_faint)
        });
    if active {
        btn = btn.bg(rgb(active_color));
    } else {
        let overlay = theme::alpha(p.border_base, p.overlay_a);
        btn = btn.bg(overlay);
    }
    if let Some(h) = handler {
        btn = btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            h(action.clone(), window, cx);
        });
    }
    btn.child(label)
}

fn tag_action(
    id: String,
    label: &'static str,
    handler: Option<std::rc::Rc<dyn Fn(TrackAction, &mut Window, &mut App)>>,
    action: TrackAction,
) -> gpui::Stateful<gpui::Div> {
    let p = theme::palette();
    let selector = id.clone();
    let mut btn = div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || selector.clone())
        .flex()
        .items_center()
        .justify_center()
        .w(px(14.0))
        .h(px(13.0))
        .rounded(px(3.0))
        .text_size(px(8.0))
        .cursor_pointer()
        .text_color(rgb(p.text_faint))
        .hover({
            let overlay = theme::alpha(p.border_base, p.overlay_a * 3.0);
            move |style| style.bg(overlay)
        });
    if let Some(h) = handler {
        btn = btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            h(action.clone(), window, cx);
        });
    }
    btn.child(label)
}

fn level_bar(
    i: usize,
    level: f32,
    handler: Option<std::rc::Rc<dyn Fn(TrackAction, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let p = theme::palette();
    let t = level.clamp(0.0, 1.0);
    let bounds: std::rc::Rc<std::cell::RefCell<Option<gpui::Bounds<gpui::Pixels>>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let bc = bounds.clone();
    let id = ElementId::Name(format!("lvl-{i}").into());
    let mut bar = div()
        .id(id.clone())
        .debug_selector(move || format!("lvl-{i}"))
        .relative()
        .flex_1()
        .h(px(14.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .child(
            div()
                .relative()
                .w_full()
                .h(px(4.0))
                .rounded(px(2.0))
                .bg(theme::alpha(p.border_base, p.border_a * 1.8))
                .child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .top(px(0.0))
                        .h_full()
                        .w(gpui::relative(t))
                        .rounded(px(2.0))
                        .bg(rgb(p.accent)),
                ),
        )
        .child(
            gpui::canvas(
                move |b, _, _| {
                    *bc.borrow_mut() = Some(b);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );
    if let Some(h) = handler {
        let h_down = h.clone();
        let h_drag = h.clone();
        let bd = bounds.clone();
        let bm = bounds.clone();
        let id_drag = id.clone();
        let level_at = move |x: f32, b: gpui::Bounds<gpui::Pixels>| -> f32 {
            let w = b.size.width.as_f32().max(1.0);
            (x / w).clamp(0.0, 1.0)
        };
        bar = bar
            .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                if let Some(b) = *bd.borrow() {
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    h_down(TrackAction::Level(i, level_at(x, b)), window, cx);
                }
            })
            .on_drag(
                crate::slider::SliderDrag::new(id_drag.clone()),
                |_, _, _, cx| cx.new(|_| crate::slider::EmptyDrag),
            )
            .on_drag_move(
                move |ev: &DragMoveEvent<crate::slider::SliderDrag>, window, cx| {
                    if ev.drag(cx).id != id_drag {
                        return;
                    }
                    if let Some(b) = *bm.borrow() {
                        let x = ev.event.position.x.as_f32() - b.origin.x.as_f32();
                        h_drag(TrackAction::Level(i, level_at(x, b)), window, cx);
                    }
                },
            );
    }
    bar
}

fn voice_menu(
    track_i: usize,
    s: &TrackListState,
    handler: Option<std::rc::Rc<dyn Fn(TrackAction, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let p = theme::palette();
    let top = TRACK_HEADER_H + (track_i as f32 + 1.0) * s.track_row_h + 2.0;
    let mut menu = div()
        .absolute()
        .top(px(top))
        .left(px(8.0))
        .w(px(150.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::border())
        .bg(rgb(p.panel))
        .p(px(3.0))
        .flex()
        .flex_col()
        .shadow_sm()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .px(px(4.0))
                .py(px(3.0))
                .text_size(px(8.5))
                .text_color(rgb(p.text_faint))
                .child(format!("「{}」音色", s.tracks[track_i].name)),
        );
    for (id, name, builtin) in s.voices.iter() {
        let id = *id;
        let mut item = div()
            .id(ElementId::Name(format!("pick-t{track_i}-v{id}").into()))
            .debug_selector(move || format!("voice-menu-{track_i}-{id}"))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(6.0))
            .px(px(7.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_size(px(10.0))
            .text_color(rgb(p.text_muted))
            .hover({
                let ov = theme::alpha(p.border_base, p.overlay_a * 2.0);
                move |style| style.bg(ov)
            })
            .child(name.clone())
            .child(
                div()
                    .text_size(px(7.5))
                    .text_color(rgb(p.text_faint))
                    .child(if *builtin { "" } else { "自定义" }),
            );
        if let Some(h) = handler.clone() {
            item = item.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                h(TrackAction::SetVoice(track_i, id), window, cx);
            });
        }
        menu = menu.child(item);
    }
    menu
}

impl RenderOnce for TrackList {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let s = self.state.clone();
        let h = self.on_action.clone();
        let mut list = div().flex().flex_col();
        for (i, track) in s.tracks.iter().enumerate() {
            let active = i == s.selected;
            let renaming = s.renaming == Some(i);
            let mut name_part = div()
                .id(ElementId::Name(format!("tname{i}").into()))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.0))
                .flex_1()
                .min_w_0()
                .cursor_pointer()
                .child(
                    div()
                        .w(px(3.0))
                        .h(px(10.0))
                        .flex_shrink_0()
                        .rounded(px(2.0))
                        .bg(rgb(track.color)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(FONT_SM))
                        .text_color(if active {
                            rgb(p.text)
                        } else {
                            rgb(p.text_muted)
                        })
                        .child(track.name.clone()),
                );
            if let Some(handler) = h.clone() {
                name_part =
                    name_part.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        handler(TrackAction::Select(i), window, cx);
                    });
            }
            let mut actions = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(tag_button(
                    format!("mute{i}"),
                    "M",
                    track.muted,
                    0xf87171,
                    h.clone(),
                    TrackAction::ToggleMute(i),
                ))
                .child(tag_button(
                    format!("solo{i}"),
                    "S",
                    track.solo,
                    0xfbbf24,
                    h.clone(),
                    TrackAction::ToggleSolo(i),
                ))
                .child(tag_action(
                    format!("ren{i}"),
                    "名",
                    h.clone(),
                    TrackAction::Rename(i),
                ))
                .child(
                    tag_action(
                        format!("vsel{i}"),
                        "音",
                        h.clone(),
                        TrackAction::PickVoice(i),
                    )
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                );
            actions = actions.child(tag_action(
                format!("dup{i}"),
                "复",
                h.clone(),
                TrackAction::Duplicate(i),
            ));
            if s.tracks.len() > 1 {
                actions = actions.child(tag_action(
                    format!("del{i}"),
                    "x",
                    h.clone(),
                    TrackAction::Delete(i),
                ));
            }
            let mut row = div()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .px(px(8.0))
                .py(px(5.0))
                .h(px(s.track_row_h))
                .flex_shrink_0()
                .border_b_1()
                .border_color(theme::border())
                .bg(if active {
                    rgb(p.card_hi)
                } else {
                    theme::alpha(0x000000, 0.0)
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(4.0))
                        .child(name_part)
                        .child(actions),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .child(level_bar(i, track.level, h.clone()))
                        .child(
                            div()
                                .w(px(20.0))
                                .flex_shrink_0()
                                .text_size(px(FONT_XS))
                                .text_color(rgb(p.text_faint))
                                .child(format!("{:.0}", track.level * 100.0)),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(format!("notes{i}").into()))
                                .debug_selector(move || format!("notes-{i}"))
                                .flex_shrink_0()
                                .text_size(px(FONT_XS))
                                .text_color(rgb(p.text_faint))
                                .child(format!("♪{}", track.notes)),
                        ),
                );
            if renaming {
                row = row.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex_1()
                                .h(px(18.0))
                                .px(px(4.0))
                                .flex()
                                .items_center()
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(theme::alpha(p.accent, 0.6))
                                .text_size(px(10.0))
                                .text_color(rgb(p.text))
                                .child(if s.rename_buffer.is_empty() {
                                    track.name.clone()
                                } else {
                                    format!("{}|", s.rename_buffer)
                                }),
                        )
                        .child(tag_action(
                            format!("renok{i}"),
                            "ok",
                            h.clone(),
                            TrackAction::CommitRename,
                        )),
                );
            }
            list = list.child(row);
        }
        let add_open = s.add_menu_open;
        let mut add_button = div()
            .id("add-track")
            .debug_selector(|| "add-track".into())
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .h(px(24.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(theme::border())
            .cursor_pointer()
            .hover({
                let ov = theme::alpha(p.border_base, p.overlay_a * 2.0);
                move |style| style.bg(ov)
            })
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(p.text_faint))
                    .child("+"),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(p.text_muted))
                    .child("添加乐器"),
            )
            .child(
                div()
                    .text_size(px(7.0))
                    .text_color(rgb(p.text_faint))
                    .child("v"),
            );
        if let Some(handler) = h.clone() {
            add_button = add_button
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(TrackAction::OpenAddMenu, window, cx);
                });
        }
        let mut add_area = div()
            .relative()
            .flex_shrink_0()
            .px(px(8.0))
            .py(px(6.0))
            .child(add_button);
        if add_open {
            let mut menu = div()
                .absolute()
                .top(px(26.0))
                .left(px(8.0))
                .w(px(150.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(theme::border())
                .bg(rgb(p.panel))
                .p(px(3.0))
                .flex()
                .flex_col()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
            for (id, name, builtin) in s.voices.iter() {
                let id = *id;
                let mut item = div()
                    .id(ElementId::Name(format!("pick-v{id}").into()))
                    .debug_selector(move || format!("add-voice-{id}"))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(6.0))
                    .px(px(7.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_size(px(10.0))
                    .text_color(rgb(p.text_muted))
                    .hover({
                        let ov = theme::alpha(p.border_base, p.overlay_a * 2.0);
                        move |style| style.bg(ov)
                    })
                    .child(name.clone())
                    .child(
                        div()
                            .text_size(px(7.5))
                            .text_color(rgb(p.text_faint))
                            .child(if *builtin { "" } else { "自定义" }),
                    );
                if let Some(handler) = h.clone() {
                    item =
                        item.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                            handler(TrackAction::AddVoice(id), window, cx);
                        });
                }
                menu = menu.child(item);
            }
            add_area = add_area.child(menu);
        }
        let mut root = div()
            .flex()
            .flex_col()
            .relative()
            .w(px(TRACK_PANEL_W))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .bg(rgb(p.panel))
            .border_r_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(TRACK_HEADER_H))
                    .flex_shrink_0()
                    .px(px(10.0))
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(rgb(p.text_faint))
                            .child("轨道"),
                    )
                    .child(
                        div()
                            .text_size(px(9.5))
                            .text_color(rgb(p.text_faint))
                            .child("音量"),
                    ),
            )
            .child(list)
            .child(add_area);
        if let Some(pick) = s.voice_pick {
            if pick < s.tracks.len() {
                root = root.child(voice_menu(pick, &s, h.clone()));
            }
        }
        root
    }
}

pub fn track_list(state: TrackListState) -> TrackList {
    TrackList::new(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(name: &str, color: u32) -> TrackView {
        TrackView {
            name: name.into(),
            color,
            level: 0.8,
            muted: false,
            solo: false,
            notes: 0,
        }
    }
    #[test]
    fn panel_width_matches_old_project() {
        assert_eq!(TRACK_PANEL_W, 200.0);
        assert_eq!(TRACK_HEADER_H, 22.0);
    }
    #[test]
    fn actions_carry_index() {
        assert_eq!(TrackAction::Select(2), TrackAction::Select(2));
        assert_ne!(TrackAction::ToggleMute(1), TrackAction::ToggleSolo(1));
        assert_ne!(TrackAction::Level(0, 0.5), TrackAction::Level(0, 0.6));
    }
    #[test]
    fn state_preserves_tracks() {
        let s = TrackListState {
            tracks: vec![view("a", 0x3b82f6), view("b", 0x22c55e)],
            selected: 1,
            track_row_h: 58.0,
            add_menu_open: false,
            voices: vec![(0, "钢琴".into(), true)],
            renaming: None,
            rename_buffer: String::new(),
            voice_pick: None,
        };
        assert_eq!(s.tracks.len(), 2);
        assert_eq!(s.selected, 1);
        assert_eq!(s.tracks[0].name, "a");
    }
}
