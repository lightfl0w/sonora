use std::rc::Rc;

use gpui::{
    App, ClickEvent, Context, Decorations, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, WindowControlArea, WindowOptions, div, px,
    rgb,
};

use crate::layout::TITLEBAR_H;
use crate::theme::{self, FONT_SM, RADIUS_SM};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Menu {
    File,
    Edit,
    Play,
    View,
    Help,
}

impl Menu {
    pub const ALL: [Menu; 5] = [Menu::File, Menu::Edit, Menu::Play, Menu::View, Menu::Help];
    pub fn label(self) -> &'static str {
        match self {
            Menu::File => "文件",
            Menu::Edit => "编辑",
            Menu::Play => "播放",
            Menu::View => "视图",
            Menu::Help => "帮助",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub action: String,
}

pub fn menu_items(menu: Menu) -> Vec<MenuItem> {
    let items: &[(&str, &str)] = match menu {
        Menu::File => &[
            ("open", "打开工程…"),
            ("import-midi", "导入 MIDI…"),
            ("export-midi", "导出 MIDI…"),
            ("export-wav", "导出 WAV…"),
            ("save", "保存工程  Ctrl+S"),
            ("save-as", "另存为…"),
            ("new-empty", "新建工程"),
            ("load-demo", "载入示例工程"),
        ],
        Menu::Edit => &[("undo", "撤销  Ctrl+Z"), ("redo", "重做  Ctrl+Shift+Z")],
        Menu::Play => &[
            ("play-pause", "播放 / 暂停  Space"),
            ("stop", "停止"),
            ("rewind", "回到开头  Home"),
            ("menu-loop", "循环播放  L"),
            ("menu-metronome", "节拍器"),
            ("menu-countin", "预卷一小节"),
        ],
        Menu::View => &[
            ("toggle-wave", "波形侧栏"),
            ("toggle-voices", "音色管理"),
            ("toggle-theme", "切换明暗主题"),
        ],
        Menu::Help => &[("about", "关于 Sonora")],
    };
    items
        .iter()
        .map(|(a, l)| MenuItem {
            action: (*a).to_string(),
            label: (*l).to_string(),
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowAction {
    Minimize,
    Maximize,
    Close,
}

struct TitleBarState {
    should_move: bool,
}

impl Render for TitleBarState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(gpui::IntoElement)]
pub struct TitleBar {
    title: String,
    subtitle: Option<String>,
    open_menu: Option<Menu>,
    dark: bool,
    on_menu: Option<Rc<dyn Fn(Menu, &mut Window, &mut App)>>,
    on_theme: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    menu_action: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
}

impl TitleBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            open_menu: None,
            dark: false,
            on_menu: None,
            on_theme: None,
            menu_action: None,
        }
    }
    pub fn window_options() -> WindowOptions {
        WindowOptions {
            titlebar: Some(gpui::TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: None,
            }),
            app_owns_titlebar_drag: true,
            ..Default::default()
        }
    }
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }
    pub fn open_menu(mut self, menu: Option<Menu>) -> Self {
        self.open_menu = menu;
        self
    }
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }
    pub fn on_menu(mut self, h: impl Fn(Menu, &mut Window, &mut App) + 'static) -> Self {
        self.on_menu = Some(Rc::new(h));
        self
    }
    pub fn on_theme(mut self, h: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_theme = Some(Rc::new(h));
        self
    }
    pub fn on_menu_action(mut self, h: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.menu_action = Some(Rc::new(h));
        self
    }
}

fn control_icon(action: WindowAction, dark_bg: bool) -> impl IntoElement {
    let p = theme::palette();
    let (id, area, is_close) = match action {
        WindowAction::Minimize => ("win-min", WindowControlArea::Min, false),
        WindowAction::Maximize => ("win-max", WindowControlArea::Max, false),
        WindowAction::Close => ("win-close", WindowControlArea::Close, true),
    };
    let hover_bg = if is_close {
        rgb(p.danger)
    } else {
        theme::alpha(p.border_base, p.border_a * 3.0)
    };
    let mut btn = div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .w(px(TITLEBAR_H))
        .h_full()
        .flex_shrink_0()
        .content_center()
        .text_color(if is_close {
            rgb(p.text_muted)
        } else {
            rgb(p.text_muted)
        })
        .hover(move |style| style.bg(hover_bg))
        .window_control_area(area);
    if cfg!(target_os = "linux") {
        btn = btn.on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        });
        btn = btn.on_click(move |_: &ClickEvent, window, _cx| match action {
            WindowAction::Minimize => window.minimize_window(),
            WindowAction::Maximize => window.zoom_window(),
            WindowAction::Close => window.remove_window(),
        });
    }
    let _ = dark_bg;
    btn.child(div().text_size(px(11.0)).child(match action {
        WindowAction::Minimize => "—",
        WindowAction::Maximize => "□",
        WindowAction::Close => "✕",
    }))
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let is_client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });
        let open_menu = self.open_menu;
        let menu_handler = self.on_menu.clone();
        let menu_action = self.menu_action.clone();
        let mut menus = div().flex().flex_row().items_center().gap(px(1.0)).child(
            div()
                .px(px(10.0))
                .text_size(px(FONT_SM))
                .text_color(rgb(p.text))
                .child(self.title.clone()),
        );
        if let Some(sub) = &self.subtitle {
            menus = menus.child(
                div()
                    .px(px(4.0))
                    .text_size(px(FONT_SM))
                    .text_color(rgb(p.text_faint))
                    .child(sub.clone()),
            );
        }
        menus = menus.child(div().w(px(10.0)));
        for menu in Menu::ALL {
            let active = open_menu == Some(menu);
            let mut entry = div()
                .id(ElementId::Name(format!("menu-{}", menu.label()).into()))
                .debug_selector(|| format!("menu-{}", menu.label()))
                .px(px(8.0))
                .py(px(3.0))
                .rounded(px(RADIUS_SM))
                .text_size(px(FONT_SM))
                .cursor_pointer()
                .text_color(if active {
                    rgb(p.text)
                } else {
                    rgb(p.text_muted)
                });
            if active {
                entry = entry.bg(theme::alpha(p.border_base, p.border_a * 2.0));
            } else {
                let hover = theme::alpha(p.border_base, p.border_a * 1.5);
                entry = entry.hover(move |style| style.bg(hover));
            }
            if let Some(h) = menu_handler.clone() {
                entry = entry.on_mouse_down(
                    MouseButton::Left,
                    move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                        cx.stop_propagation();
                        h(menu, window, cx);
                    },
                );
            }
            let mut panel_wrap = div().relative().child(entry.child(menu.label()));
            if active {
                let items = menu_items(menu);
                let mut panel = div()
                    .absolute()
                    .top(px(24.0))
                    .left(px(0.0))
                    .w(px(168.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(theme::border())
                    .bg(rgb(p.panel))
                    .shadow_lg()
                    .p(px(4.0))
                    .flex()
                    .flex_col()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
                for item in items {
                    let action = item.action.clone();
                    let selector = format!("mi-{}", item.action);
                    let mut row = div()
                        .id(ElementId::Name(format!("mi-{}", item.action).into()))
                        .debug_selector(move || selector.clone())
                        .px(px(8.0))
                        .py(px(5.0))
                        .rounded(px(5.0))
                        .cursor_pointer()
                        .text_size(px(11.0))
                        .text_color(rgb(p.text_muted))
                        .hover({
                            let ov = theme::alpha(p.border_base, p.border_a * 2.0);
                            move |style| style.bg(ov)
                        })
                        .child(item.label);
                    if let Some(h) = menu_action.clone() {
                        row = row.on_mouse_down(
                            MouseButton::Left,
                            move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                                cx.stop_propagation();
                                h(action.clone(), window, cx);
                            },
                        );
                    }
                    panel = panel.child(row);
                }
                panel_wrap = panel_wrap.child(gpui::deferred(panel).with_priority(100));
            }
            menus = menus.child(panel_wrap);
        }
        let theme_glyph = if self.dark { "亮" } else { "暗" };
        let mut theme_btn = div()
            .id("theme-toggle")
            .flex()
            .items_center()
            .justify_center()
            .w(px(26.0))
            .h(px(22.0))
            .rounded(px(RADIUS_SM))
            .text_size(px(FONT_SM))
            .cursor_pointer()
            .text_color(rgb(p.text_muted))
            .hover(move |style| style.bg(theme::alpha(p.border_base, p.border_a * 2.0)));
        if let Some(h) = self.on_theme.clone() {
            theme_btn = theme_btn
                .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| h(window, cx));
        }
        let mut controls = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_shrink_0()
            .child(theme_btn.child(theme_glyph));
        if !cfg!(target_os = "macos") && !cfg!(target_family = "wasm") {
            #[cfg(target_os = "linux")]
            let show_controls = matches!(window.window_decorations(), Decorations::Client { .. });
            #[cfg(not(target_os = "linux"))]
            let show_controls = true;
            if show_controls {
                let supported = window.window_controls();
                if supported.minimize {
                    controls = controls.child(control_icon(WindowAction::Minimize, self.dark));
                }
                if supported.maximize {
                    controls = controls.child(control_icon(WindowAction::Maximize, self.dark));
                }
                controls = controls.child(control_icon(WindowAction::Close, self.dark));
            }
        }
        let drag_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h_full()
            .flex_1()
            .flex_shrink_0()
            .window_control_area(WindowControlArea::Drag);
        let title_bar = div()
            .id("title-bar")
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(TITLEBAR_H))
            .flex_shrink_0()
            .pl(px(12.0))
            .bg(rgb(p.panel))
            .border_b_1()
            .border_color(theme::border())
            .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                state.should_move = false;
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _: &MouseDownEvent, window, _cx| {
                    state.should_move = !window.default_prevented();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _: &MouseUpEvent, _, _| {
                    state.should_move = false;
                }),
            )
            .on_mouse_move(window.listener_for(
                &state,
                |state, event: &MouseMoveEvent, window, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        state.should_move = false;
                        return;
                    }
                    if state.should_move {
                        state.should_move = false;
                        cx.stop_active_drag(window);
                        window.start_window_move();
                    }
                },
            ))
            .child(drag_bar.child(menus))
            .child(controls);
        let _ = is_client_decorated;
        div().flex_shrink_0().child(title_bar)
    }
}

pub fn title_bar(title: impl Into<String>) -> TitleBar {
    TitleBar::new(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_labels_are_unique() {
        let mut seen = Vec::new();
        for m in Menu::ALL {
            assert!(!seen.contains(&m.label()));
            seen.push(m.label());
        }
        assert_eq!(seen.len(), Menu::ALL.len());
    }
    #[test]
    fn menu_items_are_non_empty_for_all_menus() {
        for m in Menu::ALL {
            let items = menu_items(m);
            assert!(!items.is_empty(), "{:?} 菜单不应为空", m);
            for item in items {
                assert!(!item.label.is_empty());
                assert!(!item.action.is_empty());
            }
        }
    }
    #[test]
    fn file_menu_has_expected_actions() {
        let actions: Vec<String> = menu_items(Menu::File)
            .into_iter()
            .map(|i| i.action)
            .collect();
        assert!(actions.contains(&"new-empty".to_string()));
        assert!(actions.contains(&"load-demo".to_string()));
        assert!(actions.contains(&"export-wav".to_string()));
    }
    #[test]
    fn builder_sets_fields() {
        let bar = TitleBar::new("Sonora")
            .subtitle("demo")
            .dark(true)
            .open_menu(Some(Menu::File));
        assert_eq!(bar.title, "Sonora");
        assert!(bar.dark);
        assert_eq!(bar.open_menu, Some(Menu::File));
    }
    #[test]
    fn window_options_own_titlebar_drag() {
        let opts = TitleBar::window_options();
        assert!(opts.app_owns_titlebar_drag);
    }
    #[test]
    fn actions_are_comparable() {
        assert_eq!(WindowAction::Close, WindowAction::Close);
        assert_ne!(WindowAction::Minimize, WindowAction::Maximize);
    }
}
