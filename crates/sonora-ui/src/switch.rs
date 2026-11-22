use std::sync::Arc;

use gpui::{
    AnimationExt as _, App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::theme::{self, FONT_MD};

#[derive(gpui::IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    label: Option<String>,
    disabled: bool,
    on_toggle: Option<Box<dyn Fn(bool, &mut Window, &mut App) + 'static>>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self {
            id: id.into(),
            checked,
            label: None,
            disabled: false,
            on_toggle: None,
        }
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }
}

pub const TRACK_W: f32 = 32.0;
pub const TRACK_H: f32 = 18.0;
pub const KNOB_SIZE: f32 = 14.0;

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let checked = self.checked;
        let switch_id = self.id.clone();
        let disabled = self.disabled;
        let track_bg = if checked {
            rgb(p.accent)
        } else {
            theme::alpha(p.border_base, p.border_a * 3.0)
        };
        let knob = div()
            .absolute()
            .top(px((TRACK_H - KNOB_SIZE) / 2.0))
            .left(px(knob_left(checked)))
            .w(px(KNOB_SIZE))
            .h(px(KNOB_SIZE))
            .rounded(px(KNOB_SIZE / 2.0))
            .bg(rgb(0xffffff))
            .shadow_sm()
            .with_animation(
                ElementId::NamedChild(Arc::new(switch_id.clone()), "knob".into()),
                gpui::Animation::new(std::time::Duration::from_millis(160))
                    .with_easing(gpui::ease_out_quint()),
                move |el, delta| {
                    let from = knob_left(!checked);
                    let to = knob_left(checked);
                    el.left(px(from + (to - from) * delta))
                },
            );
        let track = div()
            .relative()
            .flex_shrink_0()
            .w(px(TRACK_W))
            .h(px(TRACK_H))
            .rounded(px(TRACK_H / 2.0))
            .bg(track_bg)
            .child(knob);
        let mut control = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .active(|style| style.opacity(0.72))
            .child(track);
        if let Some(label) = self.label {
            control = control.child(
                div()
                    .text_size(px(FONT_MD))
                    .text_color(rgb(p.text))
                    .child(label),
            );
        }
        if disabled {
            return control.opacity(0.45).into_any_element();
        }
        if let Some(handler) = self.on_toggle {
            let next = !checked;
            control =
                control.on_click(move |_ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(next, window, cx);
                });
        }
        control.into_any_element()
    }
}

pub fn switch(id: impl Into<ElementId>, checked: bool) -> Switch {
    Switch::new(id, checked)
}

pub fn knob_left(checked: bool) -> f32 {
    if checked {
        TRACK_W - KNOB_SIZE - 2.0
    } else {
        2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::FONT_SM;
    #[test]
    fn knob_moves_between_ends() {
        let off = knob_left(false);
        let on = knob_left(true);
        assert!(on > off);
        assert!(off >= 0.0);
        assert!(on + KNOB_SIZE <= TRACK_W);
    }
    #[test]
    fn knob_is_vertically_centered() {
        let top = (TRACK_H - KNOB_SIZE) / 2.0;
        assert!(top >= 0.0);
        assert!((top * 2.0 + KNOB_SIZE - TRACK_H).abs() < 1e-6);
    }
    #[test]
    fn track_is_wide_enough_for_knob() {
        assert!(TRACK_W > KNOB_SIZE + 4.0);
    }
    #[test]
    fn label_size_uses_theme_token() {
        assert!(FONT_MD > FONT_SM);
    }
}
