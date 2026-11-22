use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::theme::{self, FONT_MD, RADIUS_MD};

#[derive(gpui::IntoElement)]
pub struct NumberInput {
    id: ElementId,
    value: String,
    min: f32,
    max: f32,
    step: f32,
    width: f32,
    disabled: bool,
    suffix: &'static str,
    focused: bool,
    on_commit: Option<Box<dyn Fn(f32, &mut Window, &mut App) + 'static>>,
    on_focus: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_blur: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl NumberInput {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value: format!("{value:.0}"),
            min: 0.0,
            max: 999.0,
            step: 1.0,
            width: 64.0,
            disabled: false,
            suffix: "",
            focused: false,
            on_commit: None,
            on_focus: None,
            on_blur: None,
        }
    }
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = if max > min { max } else { min + 1.0 };
        self
    }
    pub fn step(mut self, step: f32) -> Self {
        self.step = if step > 0.0 { step } else { 1.0 };
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(40.0);
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
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
    pub fn on_commit(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_commit = Some(Box::new(handler));
        self
    }
    pub fn on_focus(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_focus = Some(Box::new(handler));
        self
    }
    pub fn on_blur(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_blur = Some(Box::new(handler));
        self
    }
}

pub fn parse_number(text: &str) -> Option<f32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<f32>().ok().filter(|v| v.is_finite())
}

pub fn clamp_value(value: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    }
}

impl RenderOnce for NumberInput {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let disabled = self.disabled;
        let focused = self.focused;
        let mut field = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(if focused {
                theme::alpha(p.accent, 0.75)
            } else {
                theme::border()
            })
            .bg(rgb(p.panel))
            .text_size(px(FONT_MD))
            .text_color(rgb(p.text))
            .w(px(self.width))
            .id(self.id.clone())
            .child(div().flex_1().truncate().child(self.value.clone()));
        if !self.suffix.is_empty() {
            field = field.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(10.0))
                    .text_color(rgb(p.text_faint))
                    .child(self.suffix),
            );
        }
        if !disabled {
            field = field.cursor_pointer();
        }
        let focus = self.on_focus;
        if let Some(handler) = focus {
            field = field.on_click(
                move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| handler(window, cx),
            );
        }
        if disabled {
            return field.opacity(0.45).into_any_element();
        }
        field.into_any_element()
    }
}

pub fn number_input(id: impl Into<ElementId>, value: f32) -> NumberInput {
    NumberInput::new(id, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_plain_numbers() {
        assert_eq!(parse_number("120"), Some(120.0));
        assert_eq!(parse_number("  96.5  "), Some(96.5));
        assert_eq!(parse_number("-12"), Some(-12.0));
    }
    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("   "), None);
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("12abc"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(parse_number("NaN"), None);
    }
    #[test]
    fn clamps_to_range() {
        assert_eq!(clamp_value(500.0, 20.0, 300.0), 300.0);
        assert_eq!(clamp_value(-5.0, 20.0, 300.0), 20.0);
        assert_eq!(clamp_value(120.0, 20.0, 300.0), 120.0);
    }
    #[test]
    fn non_finite_falls_back_to_min() {
        assert_eq!(clamp_value(f32::NAN, 20.0, 300.0), 20.0);
        assert_eq!(clamp_value(f32::INFINITY, 20.0, 300.0), 20.0);
        assert_eq!(clamp_value(f32::NEG_INFINITY, 20.0, 300.0), 20.0);
    }
    #[test]
    fn builder_keeps_valid_range_and_step() {
        let n = NumberInput::new("t", 120.0).range(300.0, 20.0).step(0.0);
        assert!(n.max > n.min);
        assert_eq!(n.step, 1.0);
    }
    #[test]
    fn initial_value_formats_without_decimals() {
        let n = NumberInput::new("t", 120.0);
        assert_eq!(n.value, "120");
    }
}
