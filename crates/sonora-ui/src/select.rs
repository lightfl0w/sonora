use std::sync::Arc;

use gpui::{
    div, px, rgb, AnimationExt as _, App, ClickEvent, ElementId, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window,
};

use crate::theme::{self, FONT_MD, FONT_SM, RADIUS_MD, RADIUS_SM};

#[derive(Clone, Debug, PartialEq)]
pub struct Option_ {
    pub value: u32,
    pub label: String,
}

impl Option_ {
    pub fn new(value: u32, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

#[derive(gpui::IntoElement)]
pub struct Select {
    id: ElementId,
    options: Vec<Option_>,
    selected: Option<u32>,
    placeholder: String,
    width: Option<f32>,
    disabled: bool,
    open: bool,
    on_select: Option<std::rc::Rc<dyn Fn(u32, &mut Window, &mut App)>>,
    on_toggle: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Select {
    pub fn new(id: impl Into<ElementId>, options: Vec<Option_>) -> Self {
        Self {
            id: id.into(),
            options,
            selected: None,
            placeholder: "请选择".to_string(),
            width: None,
            disabled: false,
            open: false,
            on_select: None,
            on_toggle: None,
        }
    }

    pub fn selected(mut self, value: u32) -> Self {
        self.selected = Some(value);
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(handler));
        self
    }

    pub fn on_toggle(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn current_label(&self) -> String {
        self.selected
            .and_then(|v| self.options.iter().find(|o| o.value == v))
            .map(|o| o.label.clone())
            .unwrap_or_else(|| self.placeholder.clone())
    }

    pub fn has_selection(&self) -> bool {
        self.selected
            .map(|v| self.options.iter().any(|o| o.value == v))
            .unwrap_or(false)
    }
}

impl RenderOnce for Select {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let disabled = self.disabled;
        let open = self.open;
        let current = self.current_label();
        let muted = !self.has_selection();

        let width = self.width;

        let arrow = div()
            .text_size(px(9.0))
            .text_color(rgb(p.text_faint))
            .child(if open { "▲" } else { "▼" });

        let mut head = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(6.0))
            .px(px(9.0))
            .py(px(5.0))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(if open {
                theme::alpha(p.accent, 0.7)
            } else {
                theme::border()
            })
            .bg(rgb(p.panel))
            .text_size(px(FONT_MD))
            .text_color(if muted {
                rgb(p.text_faint)
            } else {
                rgb(p.text)
            })
            .child(div().truncate().child(current))
            .child(arrow);

        if let Some(w) = width {
            head = head.w(px(w));
        }

        let mut el = head
            .id(self.id.clone())
            .cursor_pointer()
            .active(|style| style.opacity(0.72));

        if let Some(handler) = self.on_toggle {
            el = el.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                handler(window, cx);
            });
        }

        let head = el;

        let mut menu = div()
            .absolute()
            .top(px(34.0))
            .left(px(0.0))
            .flex()
            .flex_col()
            .py(px(4.0))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(theme::alpha(p.border_base, p.border_a * 4.0))
            .bg(rgb(p.popover))
            .shadow_lg()
            .min_w(px(width.unwrap_or(150.0)));

        let select_handler = self.on_select.clone();

        for option in self.options {
            let is_selected = Some(option.value) == self.selected;
            let value = option.value;
            let label = option.label;
            let handler = select_handler.clone();

            let hover_bg = theme::alpha(p.border_base, p.border_a * 3.0);

            let mut item = div()
                .id(ElementId::Name(format!("opt-{}", value).into()))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .px(px(9.0))
                .py(px(6.0))
                .mx(px(4.0))
                .rounded(px(RADIUS_SM))
                .text_size(px(FONT_SM))
                .cursor_pointer()
                .text_color(if is_selected {
                    rgb(p.accent)
                } else {
                    rgb(p.text)
                })
                .hover(move |style| style.bg(hover_bg))
                .child(div().truncate().child(label))
                .child(if is_selected {
                    div()
                        .flex_shrink_0()
                        .text_size(px(10.0))
                        .text_color(rgb(p.accent))
                        .child("✓")
                        .into_any_element()
                } else {
                    div().into_any_element()
                });

            if let Some(handler) = handler {
                item = item.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    handler(value, window, cx);
                });
            }

            menu = menu.child(item);
        }

        let root = div()
            .relative()
            .child(head)
            .child(if open {
                let menu_id = self.id.clone();
                gpui::deferred(
                    menu.with_animation(
                        ElementId::NamedChild(Arc::new(menu_id), "menu".into()),
                        gpui::Animation::new(std::time::Duration::from_millis(140))
                            .with_easing(gpui::ease_out_quint()),
                        |el, t| el.opacity(t),
                    ),
                )
                .with_priority(10)
            } else {
                gpui::deferred(div())
            });

        if disabled {
            return root.opacity(0.45).into_any_element();
        }

        root.into_any_element()
    }
}

pub fn select(id: impl Into<ElementId>, options: Vec<Option_>) -> Select {
    Select::new(id, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Vec<Option_> {
        vec![
            Option_::new(0, "大调"),
            Option_::new(1, "小调"),
            Option_::new(2, "多利亚"),
        ]
    }

    #[test]
    fn label_shows_selected_option() {
        let s = Select::new("t", opts()).selected(1);
        assert_eq!(s.current_label(), "小调");
        assert!(s.has_selection());
    }

    #[test]
    fn placeholder_when_unselected() {
        let s = Select::new("t", opts()).placeholder("选择调式");
        assert_eq!(s.current_label(), "选择调式");
        assert!(!s.has_selection());
    }

    #[test]
    fn unknown_value_falls_back_to_placeholder() {
        let s = Select::new("t", opts()).selected(99).placeholder("无");
        assert_eq!(s.current_label(), "无");
        assert!(!s.has_selection());
    }

    #[test]
    fn empty_options_use_placeholder() {
        let s = Select::new("t", Vec::new()).selected(0).placeholder("空");
        assert_eq!(s.current_label(), "空");
        assert!(!s.has_selection());
    }

    #[test]
    fn builder_sets_fields() {
        let s = Select::new("t", opts())
            .selected(2)
            .width(200.0)
            .open(true)
            .disabled(true);
        assert_eq!(s.selected, Some(2));
        assert_eq!(s.width, Some(200.0));
        assert!(s.open);
        assert!(s.disabled);
    }
}
