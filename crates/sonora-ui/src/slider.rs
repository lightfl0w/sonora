use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnimationExt as _, App, AppContext as _, Bounds, ElementId, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::theme::{self, FONT_SM, RADIUS_SM};

type ChangeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

pub const KNOB_SIZE: f32 = 12.0;
pub const BAR_H: f32 = 24.0;

#[derive(Clone)]
pub struct SliderDrag {
    pub(crate) id: ElementId,
}

impl SliderDrag {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self { id: id.into() }
    }
}

pub struct EmptyDrag;

impl gpui::Render for EmptyDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(gpui::IntoElement)]
pub struct Slider {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    label: Option<String>,
    show_value: bool,
    suffix: &'static str,
    disabled: bool,
    height: f32,
    on_change: Option<ChangeHandler>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            min: 0.0,
            max: 1.0,
            step: None,
            label: None,
            show_value: false,
            suffix: "",
            disabled: false,
            height: 6.0,
            on_change: None,
        }
    }
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = if max > min { max } else { min + 1.0 };
        self
    }
    pub fn step(mut self, step: f32) -> Self {
        self.step = if step > 0.0 { Some(step) } else { None };
        self
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }
    pub fn suffix(mut self, suffix: &'static str) -> Self {
        self.suffix = suffix;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn track_height(mut self, height: f32) -> Self {
        self.height = height.max(2.0);
        self
    }
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

pub fn fraction(value: f32, min: f32, max: f32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

pub fn value_from_x(x: f32, width: f32, min: f32, max: f32) -> f32 {
    if width <= 0.0 {
        return min;
    }
    let t = (x / width).clamp(0.0, 1.0);
    min + t * (max - min)
}

pub fn quantize(value: f32, step: Option<f32>, min: f32, max: f32) -> f32 {
    let raw = match step {
        Some(s) if s > 0.0 => (value / s).round() * s,
        _ => value,
    };
    raw.clamp(min, max)
}

fn display(value: f32, suffix: &str) -> String {
    if suffix.is_empty() {
        format!("{value:.2}")
    } else {
        format!("{value:.2}{suffix}")
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let t = fraction(self.value, self.min, self.max);
        let disabled = self.disabled;
        let min = self.min;
        let max = self.max;
        let step = self.step;
        let on_change = self.on_change.clone();
        let active = rgb(p.accent);
        let inactive = theme::alpha(p.border_base, p.border_a * 1.8);
        let fill = div()
            .h_full()
            .w(gpui::relative(t))
            .bg(active)
            .rounded(px(RADIUS_SM))
            .with_animation(
                ElementId::NamedChild(Arc::new(self.id.clone()), "fill".into()),
                gpui::Animation::new(std::time::Duration::from_millis(120))
                    .with_easing(gpui::ease_out_quint()),
                move |el, delta| el.w(gpui::relative(t * delta)),
            );
        let knob = div()
            .absolute()
            .left(gpui::relative(t))
            .ml(px(-(KNOB_SIZE / 2.0)))
            .top(px((BAR_H - KNOB_SIZE) / 2.0))
            .w(px(KNOB_SIZE))
            .h(px(KNOB_SIZE))
            .rounded(px(KNOB_SIZE / 2.0))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(theme::alpha(p.border_base, p.border_a * 2.0))
            .shadow_md();
        let track = div()
            .flex()
            .items_center()
            .h(px(self.height))
            .w_full()
            .rounded(px(self.height / 2.0))
            .bg(inactive)
            .child(fill);
        let bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));
        let bounds_canvas = bounds.clone();
        let bounds_down = bounds.clone();
        let bounds_drag = bounds.clone();
        let change_down = on_change.clone();
        let change_drag = on_change.clone();
        let bar = div()
            .relative()
            .h(px(BAR_H))
            .w_full()
            .flex()
            .items_center()
            .child(
                gpui::canvas(
                    move |b, _, _| {
                        *bounds_canvas.borrow_mut() = Some(b);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(track)
            .child(knob);
        let slider_id = self.id.clone();
        let drag_payload_id = self.id.clone();
        let slider_id_drag = self.id.clone();
        let bar = bar
            .id(slider_id)
            .on_mouse_down(gpui::MouseButton::Left, move |ev, window, cx| {
                if let Some(b) = *bounds_down.borrow() {
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    let w = b.size.width.as_f32();
                    let v = quantize(value_from_x(x, w, min, max), step, min, max);
                    if let Some(h) = &change_down {
                        h(v, window, cx);
                    }
                }
            })
            .on_drag(
                SliderDrag {
                    id: drag_payload_id,
                },
                |_, _offset, _, cx| cx.new(|_| EmptyDrag),
            )
            .on_drag_move(move |ev: &gpui::DragMoveEvent<SliderDrag>, window, cx| {
                if ev.drag(cx).id != slider_id_drag {
                    return;
                }
                if let Some(b) = *bounds_drag.borrow() {
                    let x = ev.event.position.x.as_f32() - b.origin.x.as_f32();
                    let w = b.size.width.as_f32();
                    let v = quantize(value_from_x(x, w, min, max), step, min, max);
                    if let Some(h) = &change_drag {
                        h(v, window, cx);
                    }
                }
            });
        let bar = if disabled {
            bar
        } else {
            bar.cursor_pointer().active(|style| style.opacity(0.85))
        };
        let mut row = div().flex().flex_col().gap(px(3.0)).w_full();
        let header = match (&self.label, self.show_value) {
            (Some(label), true) => Some((label.clone(), display(self.value, self.suffix))),
            (Some(label), false) => Some((label.clone(), String::new())),
            (None, true) => Some((String::new(), display(self.value, self.suffix))),
            (None, false) => None,
        };
        if let Some((label, value)) = header {
            let mut head = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .text_size(px(FONT_SM))
                .text_color(rgb(p.text_muted));
            if !label.is_empty() {
                head = head.child(label);
            }
            if !value.is_empty() {
                head = head.child(div().text_color(rgb(p.text_faint)).child(value));
            }
            row = row.child(head);
        }
        row = row.child(bar);
        if disabled {
            return row.opacity(0.45).into_any_element();
        }
        row.into_any_element()
    }
}

pub fn slider(id: impl Into<ElementId>, value: f32) -> Slider {
    Slider::new(id, value)
}

pub const MINI_H: f32 = 16.0;
pub const MINI_TRACK_H: f32 = 4.0;

#[derive(gpui::IntoElement)]
pub struct MiniSlider {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    selector: Option<String>,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl MiniSlider {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            min: 0.0,
            max: 1.0,
            step: None,
            selector: None,
            disabled: false,
            on_change: None,
        }
    }
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = if max > min { max } else { min + 1.0 };
        self
    }
    pub fn step(mut self, step: f32) -> Self {
        self.step = if step > 0.0 { Some(step) } else { None };
        self
    }
    pub fn selector(mut self, name: impl Into<String>) -> Self {
        self.selector = Some(name.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MiniSlider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let t = fraction(self.value, self.min, self.max);
        let min = self.min;
        let max = self.max;
        let step = self.step;
        let disabled = self.disabled;
        let on_change = self.on_change.clone();
        let id = self.id.clone();
        let selector = self.selector.clone();
        let group = selector
            .clone()
            .unwrap_or_else(|| "mini-slider".to_string());
        let accent = rgb(p.accent);
        let track_color = theme::alpha(p.border_base, p.border_a * 1.8);
        let knob_border = theme::alpha(p.border_base, p.border_a * 2.4);
        let track = div()
            .h(px(MINI_TRACK_H))
            .w_full()
            .rounded(px(MINI_TRACK_H / 2.0))
            .bg(track_color)
            .child(
                div()
                    .h_full()
                    .w(gpui::relative(t))
                    .rounded(px(MINI_TRACK_H / 2.0))
                    .bg(accent),
            );
        let hover_group = group.clone();
        let thumb = div()
            .absolute()
            .left(gpui::relative(t))
            .ml(px(-(KNOB_SIZE / 2.0)))
            .top(px((MINI_H - KNOB_SIZE) / 2.0))
            .w(px(KNOB_SIZE))
            .h(px(KNOB_SIZE))
            .rounded(px(KNOB_SIZE / 2.0))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(knob_border)
            .shadow_sm()
            .group_hover(hover_group, move |style| style.border_color(accent));
        let bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));
        let bounds_canvas = bounds.clone();
        let mut bar = div()
            .id(id.clone())
            .relative()
            .h(px(MINI_H))
            .w_full()
            .flex()
            .items_center()
            .group(group)
            .child(
                gpui::canvas(
                    move |b, _, _| {
                        *bounds_canvas.borrow_mut() = Some(b);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(track)
            .child(thumb);
        if let Some(sel) = selector {
            bar = bar.debug_selector(move || sel.clone());
        }
        if let Some(h) = on_change {
            let change_down = h.clone();
            let change_drag = h.clone();
            let bounds_down = bounds.clone();
            let bounds_drag = bounds.clone();
            let id_drag = id.clone();
            let value_at = move |x: f32, b: Bounds<Pixels>| -> f32 {
                let w = b.size.width.as_f32();
                let raw = value_from_x(x, w, min, max);
                quantize(raw, step, min, max)
            };
            bar = bar
                .on_mouse_down(gpui::MouseButton::Left, move |ev, window, cx| {
                    if let Some(b) = *bounds_down.borrow() {
                        let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                        change_down(value_at(x, b), window, cx);
                    }
                })
                .on_drag(SliderDrag::new(id_drag.clone()), |_, _, _, cx| {
                    cx.new(|_| EmptyDrag)
                })
                .on_drag_move(move |ev: &gpui::DragMoveEvent<SliderDrag>, window, cx| {
                    if ev.drag(cx).id != id_drag {
                        return;
                    }
                    if let Some(b) = *bounds_drag.borrow() {
                        let x = ev.event.position.x.as_f32() - b.origin.x.as_f32();
                        change_drag(value_at(x, b), window, cx);
                    }
                });
        }
        if disabled {
            bar.opacity(0.45).into_any_element()
        } else {
            bar.cursor_pointer()
                .active(|style| style.opacity(0.85))
                .into_any_element()
        }
    }
}

pub fn mini_slider(id: impl Into<ElementId>, value: f32) -> MiniSlider {
    MiniSlider::new(id, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fraction_maps_range() {
        assert!((fraction(0.0, 0.0, 1.0) - 0.0).abs() < 1e-6);
        assert!((fraction(0.5, 0.0, 1.0) - 0.5).abs() < 1e-6);
        assert!((fraction(1.0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((fraction(50.0, 20.0, 80.0) - 0.5).abs() < 1e-6);
    }
    #[test]
    fn fraction_clamps_out_of_range() {
        assert_eq!(fraction(-10.0, 0.0, 1.0), 0.0);
        assert_eq!(fraction(10.0, 0.0, 1.0), 1.0);
    }
    #[test]
    fn degenerate_range_does_not_divide_by_zero() {
        assert_eq!(fraction(5.0, 3.0, 3.0), 0.0);
    }
    #[test]
    fn value_from_x_maps_position() {
        assert!((value_from_x(0.0, 100.0, 0.0, 1.0) - 0.0).abs() < 1e-6);
        assert!((value_from_x(50.0, 100.0, 0.0, 1.0) - 0.5).abs() < 1e-6);
        assert!((value_from_x(100.0, 100.0, 60.0, 180.0) - 180.0).abs() < 1e-6);
    }
    #[test]
    fn value_from_x_clamps_and_handles_zero_width() {
        assert!((value_from_x(200.0, 100.0, 0.0, 1.0) - 1.0).abs() < 1e-6);
        assert!((value_from_x(-50.0, 100.0, 0.0, 1.0) - 0.0).abs() < 1e-6);
        assert_eq!(value_from_x(50.0, 0.0, 20.0, 80.0), 20.0);
    }
    #[test]
    fn quantize_snaps_to_step() {
        assert!((quantize(0.37, Some(0.1), 0.0, 1.0) - 0.4).abs() < 1e-6);
        assert!((quantize(0.33, Some(0.1), 0.0, 1.0) - 0.3).abs() < 1e-6);
        assert!((quantize(0.37, Some(1.0), 0.0, 10.0) - 0.0).abs() < 1e-6);
    }
    #[test]
    fn quantize_without_step_is_identity_but_clamped() {
        assert!((quantize(0.37, None, 0.0, 1.0) - 0.37).abs() < 1e-6);
        assert_eq!(quantize(5.0, None, 0.0, 1.0), 1.0);
    }
    #[test]
    fn display_formats_with_suffix() {
        assert_eq!(display(0.5, ""), "0.50");
        assert_eq!(display(120.0, " BPM"), "120.00 BPM");
    }
    #[test]
    fn range_never_collapses() {
        let s = Slider::new("t", 0.5).range(5.0, 5.0);
        assert!(s.max > s.min);
    }
    #[test]
    fn drag_payload_identifies_its_own_slider() {
        let a = SliderDrag {
            id: ElementId::Name("volume".into()),
        };
        let b = SliderDrag {
            id: ElementId::Name("pan".into()),
        };
        assert_ne!(a.id, b.id, "不同滑块必须携带不同标识");
    }
    #[test]
    fn drag_payload_matches_its_source() {
        let id = ElementId::Name("volume".into());
        let payload = SliderDrag { id: id.clone() };
        assert_eq!(
            payload.id, id,
            "拖动载荷的标识需与滑块自身标识一致，否则过滤失效"
        );
    }
}
