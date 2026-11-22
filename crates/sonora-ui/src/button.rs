use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use crate::theme::{self, FONT_SM, Palette, RADIUS_MD};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Sm,
    Md,
}

#[derive(gpui::IntoElement)]
pub struct Button {
    id: Option<ElementId>,
    selector: Option<String>,
    label: SharedString,
    variant: Variant,
    size: Size,
    disabled: bool,
    selected: bool,
    full_width: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl Button {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            id: None,
            selector: None,
            label: label.into(),
            variant: Variant::Secondary,
            size: Size::Md,
            disabled: false,
            selected: false,
            full_width: false,
            on_click: None,
        }
    }
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn selector(mut self, name: impl Into<String>) -> Self {
        self.selector = Some(name.into());
        self
    }
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
    pub fn primary(self) -> Self {
        self.variant(Variant::Primary)
    }
    pub fn ghost(self) -> Self {
        self.variant(Variant::Ghost)
    }
    pub fn danger(self) -> Self {
        self.variant(Variant::Danger)
    }
    pub fn small(self) -> Self {
        self.size(Size::Sm)
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

struct Colors {
    bg: Option<u32>,
    bg_hover: Option<u32>,
    fg: u32,
    border: Option<u32>,
}

fn colors_for(p: &Palette, variant: Variant, selected: bool) -> Colors {
    match variant {
        Variant::Primary => Colors {
            bg: Some(p.primary),
            bg_hover: None,
            fg: p.primary_fg,
            border: None,
        },
        Variant::Secondary => {
            if selected {
                Colors {
                    bg: Some(p.card_hi),
                    bg_hover: None,
                    fg: p.text,
                    border: Some(p.accent),
                }
            } else {
                Colors {
                    bg: Some(p.panel),
                    bg_hover: Some(p.card_hi),
                    fg: p.text,
                    border: Some(p.border_base),
                }
            }
        }
        Variant::Ghost => Colors {
            bg: None,
            bg_hover: Some(p.card_hi),
            fg: p.text_muted,
            border: None,
        },
        Variant::Danger => Colors {
            bg: Some(p.danger),
            bg_hover: None,
            fg: 0xffffff,
            border: None,
        },
    }
}

impl Button {
    fn padding(&self) -> (f32, f32) {
        match self.size {
            Size::Sm => (8.0, 4.0),
            Size::Md => (12.0, 7.0),
        }
    }
    fn font_size(&self) -> f32 {
        match self.size {
            Size::Sm => FONT_SM,
            Size::Md => theme::FONT_MD,
        }
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let colors = colors_for(p, self.variant, self.selected);
        let (pad_x, pad_y) = self.padding();
        let mut base = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .px(px(pad_x))
            .py(px(pad_y))
            .rounded(px(RADIUS_MD))
            .text_size(px(self.font_size()))
            .text_color(rgb(colors.fg));
        if let Some(bg) = colors.bg {
            base = base.bg(rgb(bg));
        }
        if let Some(border) = colors.border {
            let a = if self.selected { 0.6 } else { p.border_a };
            base = base.border_1().border_color(theme::alpha(border, a));
        }
        if self.full_width {
            base = base.w_full();
        }
        if self.disabled {
            return base.opacity(0.45).into_any_element();
        }
        if let Some(hover) = colors.bg_hover {
            base = base.hover(move |style| style.bg(rgb(hover)));
        }
        base = base.cursor_pointer();
        let id = self
            .id
            .unwrap_or_else(|| ElementId::Name(self.label.clone()));
        let mut clickable = base.id(id).active(|style| style.opacity(0.72));
        if let Some(sel) = self.selector {
            clickable = clickable.debug_selector(move || sel.clone());
        }
        if let Some(handler) = self.on_click {
            clickable = clickable.on_click(handler);
        }
        clickable.child(self.label).into_any_element()
    }
}

pub fn button(label: impl Into<SharedString>) -> Button {
    Button::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{DARK, LIGHT};
    #[test]
    fn variants_have_distinct_roles() {
        let primary = colors_for(&LIGHT, Variant::Primary, false);
        let secondary = colors_for(&LIGHT, Variant::Secondary, false);
        assert_ne!(primary.bg, secondary.bg);
        assert_eq!(primary.fg, LIGHT.primary_fg);
    }
    #[test]
    fn selected_state_changes_appearance() {
        let off = colors_for(&LIGHT, Variant::Secondary, false);
        let on = colors_for(&LIGHT, Variant::Secondary, true);
        assert_ne!(off.bg, on.bg);
        assert_eq!(on.border, Some(LIGHT.accent));
    }
    #[test]
    fn ghost_has_no_border() {
        assert!(colors_for(&LIGHT, Variant::Ghost, false).border.is_none());
        assert!(colors_for(&DARK, Variant::Ghost, false).border.is_none());
    }
    #[test]
    fn ghost_has_no_background_so_it_inherits_parent() {
        for p in [&LIGHT, &DARK] {
            let c = colors_for(p, Variant::Ghost, false);
            assert!(c.bg.is_none(), "幽灵按钮不应有底色，否则亮色模式会显脏");
            assert!(c.bg_hover.is_some(), "悬停仍需反馈");
        }
    }
    #[test]
    fn ghost_hover_uses_theme_card_not_hardcoded() {
        let light = colors_for(&LIGHT, Variant::Ghost, false);
        let dark = colors_for(&DARK, Variant::Ghost, false);
        assert_eq!(light.bg_hover, Some(LIGHT.card_hi));
        assert_eq!(dark.bg_hover, Some(DARK.card_hi));
    }
    #[test]
    fn primary_button_contrasts_in_dark_mode() {
        let c = colors_for(&DARK, Variant::Primary, false);
        assert_eq!(c.bg, Some(DARK.primary));
        assert_eq!(c.fg, DARK.primary_fg);
        assert_ne!(Some(c.fg), c.bg);
    }
    #[test]
    fn sizes_scale_padding_and_font() {
        let sm = Button::new("x").small();
        let md = Button::new("x");
        assert!(sm.font_size() < md.font_size());
        assert!(sm.padding().0 < md.padding().0);
    }
    #[test]
    fn builder_chains() {
        let b = Button::new("x")
            .primary()
            .small()
            .disabled(true)
            .full_width()
            .selected(true);
        assert_eq!(b.variant, Variant::Primary);
        assert_eq!(b.size, Size::Sm);
        assert!(b.disabled);
        assert!(b.full_width);
        assert!(b.selected);
    }
}
