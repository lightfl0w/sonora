use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, px, rgb,
};

use crate::button::button;
use crate::theme::{self, FONT_MD, FONT_SM, FONT_XS, RADIUS_LG, RADIUS_SM};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathPromptKind {
    Open,
    SaveAs,
    ImportMidi,
    ExportMidi,
    ExportWav,
}

impl PathPromptKind {
    pub fn title(self) -> &'static str {
        match self {
            PathPromptKind::Open => "打开工程",
            PathPromptKind::SaveAs => "另存为",
            PathPromptKind::ImportMidi => "导入 MIDI",
            PathPromptKind::ExportMidi => "导出 MIDI",
            PathPromptKind::ExportWav => "导出 WAV",
        }
    }
    pub fn confirm_label(self) -> &'static str {
        match self {
            PathPromptKind::Open => "打开",
            PathPromptKind::SaveAs => "保存",
            PathPromptKind::ImportMidi => "导入",
            PathPromptKind::ExportMidi => "导出",
            PathPromptKind::ExportWav => "导出",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PathPromptState {
    pub kind: PathPromptKind,
    pub buffer: String,
    pub candidates: Vec<PathBuf>,
    pub warning: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PathPromptAction {
    Confirm,
    Cancel,
    Pick(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExitAction {
    SaveAndExit,
    DiscardAndExit,
    Cancel,
}

type Handler<A> = Rc<dyn Fn(A, &mut Window, &mut App)>;

#[derive(gpui::IntoElement)]
pub struct PathPrompt {
    state: PathPromptState,
    on_action: Option<Handler<PathPromptAction>>,
}

impl PathPrompt {
    pub fn new(state: PathPromptState) -> Self {
        Self {
            state,
            on_action: None,
        }
    }
    pub fn on_action(
        mut self,
        handler: impl Fn(PathPromptAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

pub fn path_prompt(state: PathPromptState) -> PathPrompt {
    PathPrompt::new(state)
}

impl RenderOnce for PathPrompt {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let h = self.on_action.clone();
        let kind = self.state.kind;
        let buffer = self.state.buffer.clone();
        let warning = self.state.warning.clone();
        let mut list = div().flex().flex_col().gap(px(2.0));
        if self.state.candidates.is_empty() {
            list = list.child(
                div()
                    .text_size(px(FONT_XS))
                    .text_color(rgb(p.text_faint))
                    .child("默认目录里还没有 .sonora 工程文件"),
            );
        }
        for (i, path) in self.state.candidates.iter().enumerate() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let parent = path
                .parent()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            let mut row = div()
                .id(ElementId::Name(format!("prompt-file-{i}").into()))
                .debug_selector(move || format!("prompt-file-{i}"))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .px(px(8.0))
                .h(px(22.0))
                .rounded(px(RADIUS_SM))
                .cursor_pointer()
                .hover({
                    let hover = theme::alpha(p.border_base, p.border_a * 2.0);
                    move |style| style.bg(hover)
                })
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(FONT_SM))
                        .text_color(rgb(p.text_muted))
                        .child(name),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(FONT_XS))
                        .text_color(rgb(p.text_faint))
                        .child(parent),
                );
            if let Some(pick) = h.clone() {
                row = row.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    pick(PathPromptAction::Pick(i), window, cx);
                });
            }
            list = list.child(row);
        }
        let mut cancel_btn = button("取消")
            .id("prompt-cancel")
            .selector("prompt-cancel")
            .ghost()
            .small();
        if let Some(cancel) = h.clone() {
            cancel_btn =
                cancel_btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    cancel(PathPromptAction::Cancel, window, cx);
                });
        }
        let mut confirm_btn = button(kind.confirm_label())
            .id("prompt-confirm")
            .selector("prompt-confirm")
            .primary()
            .small();
        if let Some(confirm) = h.clone() {
            confirm_btn =
                confirm_btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    confirm(PathPromptAction::Confirm, window, cx);
                });
        }
        let card = div()
            .id("prompt-card")
            .debug_selector(|| "prompt-card".into())
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w(px(640.0))
            .p(px(16.0))
            .rounded(px(RADIUS_LG + 2.0))
            .border_1()
            .border_color(theme::border())
            .bg(rgb(p.panel))
            .shadow_lg()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(FONT_MD))
                    .text_color(rgb(p.text))
                    .child(kind.title()),
            )
            .child(match warning {
                Some(text) => div()
                    .id("prompt-warning")
                    .debug_selector(|| "prompt-warning".into())
                    .text_size(px(FONT_XS))
                    .text_color(rgb(p.danger))
                    .child(text)
                    .into_any_element(),
                None => div().into_any_element(),
            })
            .child(
                div()
                    .id("prompt-input")
                    .debug_selector(|| "prompt-input".into())
                    .w_full()
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(RADIUS_SM))
                    .border_1()
                    .border_color(theme::alpha(p.accent, 0.8))
                    .bg(rgb(p.panel))
                    .text_size(px(FONT_SM))
                    .text_color(rgb(p.text))
                    .child(div().truncate().child(format!("{buffer}|"))),
            )
            .child(list)
            .child(
                div()
                    .text_size(px(FONT_XS))
                    .text_color(rgb(p.text_faint))
                    .child("回车确认，Esc 取消；没写扩展名会自动补 .sonora"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().flex_1())
                    .child(cancel_btn)
                    .child(confirm_btn),
            );
        let mask_cancel = h.clone();
        let mut mask = div()
            .id("prompt-mask")
            .debug_selector(|| "prompt-mask".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::alpha(0x000000, 0.4));
        if let Some(cancel) = mask_cancel {
            mask = mask.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                cancel(PathPromptAction::Cancel, window, cx);
            });
        }
        mask.child(card)
    }
}

#[derive(gpui::IntoElement)]
pub struct ExitPrompt {
    project: String,
    on_action: Option<Handler<ExitAction>>,
}

impl ExitPrompt {
    pub fn new(project: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            on_action: None,
        }
    }
    pub fn on_action(
        mut self,
        handler: impl Fn(ExitAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

pub fn exit_prompt(project: impl Into<String>) -> ExitPrompt {
    ExitPrompt::new(project)
}

impl RenderOnce for ExitPrompt {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let h = self.on_action.clone();
        let make_button = |id: &'static str,
                           label: &'static str,
                           action: ExitAction,
                           primary: bool,
                           handler: Option<Handler<ExitAction>>| {
            let mut btn = button(label).id(id).selector(id).small();
            btn = if primary { btn.primary() } else { btn.ghost() };
            if let Some(h) = handler {
                btn = btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    h(action, window, cx);
                });
            }
            btn
        };
        let card = div()
            .id("exit-card")
            .debug_selector(|| "exit-card".into())
            .flex()
            .flex_col()
            .gap(px(12.0))
            .w(px(480.0))
            .p(px(16.0))
            .rounded(px(RADIUS_LG + 2.0))
            .border_1()
            .border_color(theme::border())
            .bg(rgb(p.panel))
            .shadow_lg()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(FONT_MD))
                    .text_color(rgb(p.text))
                    .child("有未保存的改动"),
            )
            .child(
                div()
                    .text_size(px(FONT_SM))
                    .text_color(rgb(p.text_muted))
                    .child(format!(
                        "「{}」还没有保存，关闭窗口前要先保存吗？",
                        self.project
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().flex_1())
                    .child(make_button(
                        "exit-cancel",
                        "取消",
                        ExitAction::Cancel,
                        false,
                        h.clone(),
                    ))
                    .child(make_button(
                        "exit-discard",
                        "不保存退出",
                        ExitAction::DiscardAndExit,
                        false,
                        h.clone(),
                    ))
                    .child(make_button(
                        "exit-save",
                        "保存并退出",
                        ExitAction::SaveAndExit,
                        true,
                        h.clone(),
                    )),
            );
        let mask_action = h.clone();
        let mut mask = div()
            .id("exit-mask")
            .debug_selector(|| "exit-mask".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::alpha(0x000000, 0.45));
        if let Some(h) = mask_action {
            mask = mask.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                h(ExitAction::Cancel, window, cx);
            });
        }
        mask.child(card)
    }
}
