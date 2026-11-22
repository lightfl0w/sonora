use gpui::{
    AnimationExt as _, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement, Styled, Window, div, point, px, rgb, size,
};

use crate::app::{GRID_OPTIONS, grid_label};
use crate::theme::{self, FONT_SM, RADIUS_MD, RADIUS_SM};
use sonora_core::NOTE_NAMES;

pub const TOOLBAR_H: f32 = 46.0;

#[derive(Clone, Debug)]
pub struct ToolbarState {
    pub playing: bool,
    pub bpm: String,
    pub bpm_editing: bool,
    pub grid_label: String,
    pub root_label: String,
    pub mode_label: String,
    pub row_h: f32,
    pub snap: bool,
    pub loop_on: bool,
    pub open_dropdown: Option<Dropdown>,
    pub grid_beats: f32,
    pub root_index: usize,
    pub mode_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dropdown {
    Grid,
    Root,
    Mode,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ToolbarAction {
    TogglePlay,
    Rewind,
    EditBpm,
    RowHeightSmaller,
    RowHeightBigger,
    ToggleSnap,
    ToggleLoop,
    ToggleDropdown(Dropdown),
    PickGrid(usize),
    PickRoot(usize),
    PickMode(usize),
}

#[derive(gpui::IntoElement)]
pub struct Toolbar {
    state: ToolbarState,
    on_action: Option<std::rc::Rc<dyn Fn(ToolbarAction, &mut Window, &mut App)>>,
}

impl Toolbar {
    pub fn new(state: ToolbarState) -> Self {
        Self {
            state,
            on_action: None,
        }
    }
    pub fn on_action(mut self, h: impl Fn(ToolbarAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(std::rc::Rc::new(h));
        self
    }
}

fn field_label(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(9.5))
        .text_color(rgb(theme::palette().text_faint))
        .child(text)
}

fn group(children: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(1.0))
        .p(px(2.0))
        .rounded(px(7.0))
        .bg(theme::alpha(
            theme::palette().border_base,
            theme::palette().border_a * 1.2,
        ))
        .child(children)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconKind {
    Play,
    Pause,
    Rewind,
    Minus,
    Plus,
    None,
}

fn icon_btn(
    id: &'static str,
    label: &'static str,
    icon: IconKind,
    handler: Option<std::rc::Rc<dyn Fn(ToolbarAction, &mut Window, &mut App)>>,
    action: ToolbarAction,
    active: bool,
) -> impl IntoElement {
    let p = theme::palette();
    let mut btn = div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(if icon == IconKind::None { 28.0 } else { 28.0 }))
        .h(px(26.0))
        .px(px(6.0))
        .rounded(px(RADIUS_SM))
        .text_size(px(FONT_SM))
        .cursor_pointer()
        .text_color(if active {
            rgb(p.text)
        } else {
            rgb(p.text_muted)
        });
    if active {
        btn = btn.bg(rgb(p.panel));
    } else {
        btn = btn.hover({
            let hover = rgb(p.panel);
            move |style| style.bg(hover)
        });
    }
    if let Some(h) = handler {
        btn = btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            h(action.clone(), window, cx);
        });
    }
    if icon != IconKind::None {
        let color = if active { p.text } else { p.text_muted };
        let kind = icon;
        btn = btn.child(
            gpui::canvas(
                move |b, _, _| b.size,
                move |b, _, window, _cx| {
                    let w = b.size.width.as_f32();
                    let h = b.size.height.as_f32();
                    let s = h.min(12.0);
                    let x = b.origin.x.as_f32() + (w - s) / 2.0;
                    let y = b.origin.y.as_f32() + (h - s) / 2.0;
                    let c = hsla_from_u32(color);
                    match kind {
                        IconKind::Play => crate::icons::paint_play(window, x, y, s, c),
                        IconKind::Pause => crate::icons::paint_pause(window, x, y, s, c),
                        IconKind::Rewind => crate::icons::paint_skip_back(window, x, y, s, c),
                        IconKind::Minus => window.paint_quad(gpui::fill(
                            gpui::Bounds::new(
                                point(px(x), px(y + s / 2.0 - 0.75)),
                                size(px(s), px(1.5)),
                            ),
                            c,
                        )),
                        IconKind::Plus => {
                            window.paint_quad(gpui::fill(
                                gpui::Bounds::new(
                                    point(px(x), px(y + s / 2.0 - 0.75)),
                                    size(px(s), px(1.5)),
                                ),
                                c,
                            ));
                            window.paint_quad(gpui::fill(
                                gpui::Bounds::new(
                                    point(px(x + s / 2.0 - 0.75), px(y)),
                                    size(px(1.5), px(s)),
                                ),
                                c,
                            ));
                        }
                        IconKind::None => {}
                    }
                },
            )
            .size_full(),
        );
        return btn.into_any_element();
    }
    btn.child(label).into_any_element()
}

pub fn hsla_from_u32(hex: u32) -> gpui::Hsla {
    gpui::rgb(hex).into()
}

fn dropdown_field(
    id: &'static str,
    label: String,
    width: f32,
    options: Vec<String>,
    selected: usize,
    open: bool,
    handler: Option<std::rc::Rc<dyn Fn(ToolbarAction, &mut Window, &mut App)>>,
    dropdown: Dropdown,
    pick: fn(usize) -> ToolbarAction,
) -> impl IntoElement {
    let p = theme::palette();
    let mut head = div()
        .id(ElementId::Name(id.into()))
        .debug_selector(move || format!("dd-{id}"))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(5.0))
        .w(px(width))
        .h(px(24.0))
        .px(px(7.0))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(if open {
            theme::alpha(p.accent, 0.7)
        } else {
            theme::border()
        })
        .bg(rgb(if open { p.card } else { p.panel }))
        .text_size(px(10.0))
        .cursor_pointer()
        .text_color(rgb(p.text_muted))
        .hover({
            let hover = theme::alpha(p.border_base, p.border_a * 2.0);
            move |style| style.bg(hover)
        })
        .child(div().truncate().child(label))
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(7.5))
                .text_color(rgb(p.text_faint))
                .child(if open { "^" } else { "v" }),
        );
    if let Some(h) = handler.clone() {
        head = head
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                h(ToolbarAction::ToggleDropdown(dropdown), window, cx);
            });
    }
    if !open {
        return div().relative().child(head).into_any_element();
    }
    let mut menu = div()
        .absolute()
        .top(px(26.0))
        .left(px(0.0))
        .flex()
        .flex_col()
        .w(px(width.max(120.0)))
        .py(px(4.0))
        .rounded(px(RADIUS_MD))
        .border_1()
        .border_color(theme::alpha(p.border_base, p.border_a * 4.0))
        .bg(rgb(p.popover))
        .shadow_lg()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
    for (i, opt) in options.into_iter().enumerate() {
        let is_selected = i == selected;
        let mut item = div()
            .id(ElementId::Name(format!("{id}-opt-{i}").into()))
            .debug_selector(move || format!("dd-opt-{id}-{i}"))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(10.0))
            .mx(px(4.0))
            .px(px(7.0))
            .py(px(5.0))
            .rounded(px(RADIUS_SM))
            .text_size(px(FONT_SM))
            .cursor_pointer()
            .text_color(if is_selected {
                rgb(p.accent)
            } else {
                rgb(p.text)
            })
            .hover({
                let hover = theme::alpha(p.border_base, p.border_a * 3.0);
                move |style| style.bg(hover)
            })
            .child(div().truncate().child(opt))
            .child(if is_selected {
                div()
                    .flex_shrink_0()
                    .text_size(px(9.0))
                    .text_color(rgb(p.accent))
                    .child(" ")
                    .into_any_element()
            } else {
                div().into_any_element()
            });
        if let Some(h) = handler.clone() {
            item = item.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                h(pick(i), window, cx);
            });
        }
        menu = menu.child(item);
    }
    div()
        .relative()
        .child(head)
        .child(
            gpui::deferred(
                menu.with_animation(
                    ElementId::Name(format!("{id}-menu").into()),
                    gpui::Animation::new(std::time::Duration::from_millis(120))
                        .with_easing(gpui::ease_out_quint()),
                    |el, t| el.opacity(t),
                ),
            )
            .with_priority(10),
        )
        .into_any_element()
}

impl RenderOnce for Toolbar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let s = self.state.clone();
        let h = self.on_action.clone();
        let play_btn = icon_btn(
            "tb-play",
            "",
            if s.playing {
                IconKind::Pause
            } else {
                IconKind::Play
            },
            h.clone(),
            ToolbarAction::TogglePlay,
            s.playing,
        );
        let rewind_btn = icon_btn(
            "tb-rewind",
            "",
            IconKind::Rewind,
            h.clone(),
            ToolbarAction::Rewind,
            false,
        );
        let mut bpm_field = div()
            .id(ElementId::Name("tb-bpm".into()))
            .flex()
            .items_center()
            .justify_center()
            .w(px(56.0))
            .h(px(24.0))
            .rounded(px(RADIUS_SM))
            .border_1()
            .border_color(if s.bpm_editing {
                theme::alpha(p.accent, 0.8)
            } else {
                theme::border()
            })
            .bg(rgb(if s.bpm_editing { p.panel } else { p.card }))
            .text_size(px(FONT_SM))
            .cursor_pointer()
            .text_color(rgb(if s.bpm_editing { p.accent } else { p.text }))
            .debug_selector(|| "tb-bpm".into())
            .child(div().truncate().child(if s.bpm_editing {
                format!("{}|", s.bpm)
            } else {
                s.bpm.clone()
            }));
        if let Some(handler) = h.clone() {
            bpm_field =
                bpm_field.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(ToolbarAction::EditBpm, window, cx);
                });
        }
        let smaller = icon_btn(
            "tb-row-sm",
            "",
            IconKind::Minus,
            h.clone(),
            ToolbarAction::RowHeightSmaller,
            false,
        );
        let bigger = icon_btn(
            "tb-row-lg",
            "",
            IconKind::Plus,
            h.clone(),
            ToolbarAction::RowHeightBigger,
            false,
        );
        let mut snap_switch = div()
            .id(ElementId::Name("tb-snap".into()))
            .relative()
            .w(px(32.0))
            .h(px(18.0))
            .rounded(px(9.0))
            .cursor_pointer()
            .bg(if s.snap {
                rgb(p.accent)
            } else {
                theme::alpha(p.border_base, p.border_a * 3.0)
            })
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(px(if s.snap { 16.0 } else { 2.0 }))
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded(px(7.0))
                    .bg(gpui::rgb(0xffffff))
                    .shadow_sm(),
            );
        if let Some(handler) = h.clone() {
            snap_switch =
                snap_switch.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(ToolbarAction::ToggleSnap, window, cx);
                });
        }
        let mut loop_switch = div()
            .id(ElementId::Name("tb-loop".into()))
            .debug_selector(|| "tb-loop".into())
            .relative()
            .w(px(32.0))
            .h(px(18.0))
            .rounded(px(9.0))
            .cursor_pointer()
            .bg(if s.loop_on {
                rgb(p.accent)
            } else {
                theme::alpha(p.border_base, p.border_a * 3.0)
            })
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(px(if s.loop_on { 16.0 } else { 2.0 }))
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded(px(7.0))
                    .bg(gpui::rgb(0xffffff))
                    .shadow_sm(),
            );
        if let Some(handler) = h.clone() {
            loop_switch =
                loop_switch.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(ToolbarAction::ToggleLoop, window, cx);
                });
        }
        let root =
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(14.0))
                .px(px(12.0))
                .h(px(TOOLBAR_H))
                .flex_shrink_0()
                .border_b_1()
                .border_color(theme::border())
                .bg(rgb(p.panel))
                .id(ElementId::Name("toolbar".into()))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.0))
                        .child(group(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(2.0))
                                .child(play_btn)
                                .child(rewind_btn),
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .child(field_label("BPM"))
                                .child(bpm_field),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(5.0))
                                .child(div().w(px(5.0)).h(px(5.0)).rounded(px(3.0)).bg(
                                    if s.playing {
                                        rgb(p.live)
                                    } else {
                                        rgb(p.text_faint)
                                    },
                                ))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(rgb(p.text_faint))
                                        .child(if s.playing { "播放" } else { "待机" }),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(field_label("网格"))
                        .child(dropdown_field(
                            "tb-grid",
                            s.grid_label.clone(),
                            76.0,
                            GRID_OPTIONS.iter().map(|g| grid_label(*g)).collect(),
                            GRID_OPTIONS
                                .iter()
                                .position(|g| (g - s.grid_beats).abs() < 1e-6)
                                .unwrap_or(0),
                            s.open_dropdown == Some(Dropdown::Grid),
                            h.clone(),
                            Dropdown::Grid,
                            ToolbarAction::PickGrid,
                        ))
                        .child(field_label("调"))
                        .child(dropdown_field(
                            "tb-root",
                            s.root_label.clone(),
                            62.0,
                            NOTE_NAMES.iter().map(|n| n.to_string()).collect(),
                            s.root_index,
                            s.open_dropdown == Some(Dropdown::Root),
                            h.clone(),
                            Dropdown::Root,
                            ToolbarAction::PickRoot,
                        ))
                        .child(field_label("音阶"))
                        .child(dropdown_field(
                            "tb-mode",
                            s.mode_label.clone(),
                            96.0,
                            sonora_core::Mode::ALL
                                .iter()
                                .map(|m| m.label().to_string())
                                .collect(),
                            s.mode_index,
                            s.open_dropdown == Some(Dropdown::Mode),
                            h.clone(),
                            Dropdown::Mode,
                            ToolbarAction::PickMode,
                        ))
                        .child(field_label("行高"))
                        .child(group(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(1.0))
                                .child(smaller)
                                .child(
                                    div()
                                        .w(px(28.0))
                                        .text_size(px(9.5))
                                        .text_color(rgb(p.text_muted))
                                        .child(format!("{:.0}", s.row_h)),
                                )
                                .child(bigger),
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(5.0))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(rgb(p.text_faint))
                                        .child("调内吸附"),
                                )
                                .child(snap_switch),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(5.0))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(rgb(p.text_faint))
                                        .child("循环"),
                                )
                                .child(loop_switch),
                        ),
                );
        root
    }
}

pub fn toolbar(state: ToolbarState) -> Toolbar {
    Toolbar::new(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toolbar_height_matches_old_project() {
        assert_eq!(TOOLBAR_H, 46.0);
    }
    #[test]
    fn actions_are_comparable() {
        assert_eq!(ToolbarAction::TogglePlay, ToolbarAction::TogglePlay);
        assert_ne!(ToolbarAction::TogglePlay, ToolbarAction::Rewind);
    }
    #[test]
    fn state_holds_display_values() {
        let s = ToolbarState {
            playing: true,
            bpm: "96".into(),
            bpm_editing: false,
            grid_label: "1/4".into(),
            root_label: "C".into(),
            mode_label: "大调".into(),
            row_h: 26.0,
            snap: true,
            loop_on: false,
            open_dropdown: None,
            grid_beats: 1.0,
            root_index: 0,
            mode_index: 0,
        };
        assert!(s.playing);
        assert_eq!(s.bpm, "96");
        assert!(s.snap);
    }
    #[test]
    fn dropdown_actions_are_distinct() {
        assert_ne!(
            ToolbarAction::ToggleDropdown(Dropdown::Grid),
            ToolbarAction::ToggleDropdown(Dropdown::Root)
        );
        assert_ne!(
            ToolbarAction::ToggleDropdown(Dropdown::Grid),
            ToolbarAction::PickGrid(1)
        );
    }
    #[test]
    fn pick_actions_carry_option_index() {
        assert_eq!(ToolbarAction::PickGrid(3), ToolbarAction::PickGrid(3));
        assert_ne!(ToolbarAction::PickMode(1), ToolbarAction::PickMode(2));
    }
}
