use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window, div, px, rgb,
};
use sonora_core::VoiceId;
use sonora_engine::{MAX_PARTIALS, VoicePreset};

use crate::button::{Size as ButtonSize, Variant as ButtonVariant, button};
use crate::slider::mini_slider;
use crate::theme::{self, FONT_MD, FONT_SM, FONT_XS, RADIUS_LG, RADIUS_MD, RADIUS_SM};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PartialField {
    Ratio,
    Decay,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VoiceParam {
    Attack,
    Release,
    Cutoff,
    Sweep,
    Drift,
    Level,
}

impl VoiceParam {
    pub const ALL: [VoiceParam; 6] = [
        VoiceParam::Attack,
        VoiceParam::Release,
        VoiceParam::Cutoff,
        VoiceParam::Sweep,
        VoiceParam::Drift,
        VoiceParam::Level,
    ];
    pub fn label(self) -> &'static str {
        match self {
            VoiceParam::Attack => "起音",
            VoiceParam::Release => "释放",
            VoiceParam::Cutoff => "截止",
            VoiceParam::Sweep => "下扫",
            VoiceParam::Drift => "漂移",
            VoiceParam::Level => "电平",
        }
    }
    pub fn range(self) -> (f32, f32) {
        match self {
            VoiceParam::Attack => (0.001, 4.0),
            VoiceParam::Release => (0.05, 8.0),
            VoiceParam::Cutoff => (80.0, 16000.0),
            VoiceParam::Sweep => (0.0, 1.0),
            VoiceParam::Drift => (0.0, 20.0),
            VoiceParam::Level => (0.0, 0.5),
        }
    }
    pub fn get(self, p: &VoicePreset) -> f32 {
        match self {
            VoiceParam::Attack => p.attack,
            VoiceParam::Release => p.release,
            VoiceParam::Cutoff => p.cutoff,
            VoiceParam::Sweep => p.cutoff_sweep,
            VoiceParam::Drift => p.drift_depth,
            VoiceParam::Level => p.level,
        }
    }
    pub fn set(self, p: &mut VoicePreset, v: f32) {
        match self {
            VoiceParam::Attack => p.attack = v,
            VoiceParam::Release => p.release = v,
            VoiceParam::Cutoff => p.cutoff = v,
            VoiceParam::Sweep => p.cutoff_sweep = v,
            VoiceParam::Drift => p.drift_depth = v,
            VoiceParam::Level => p.level = v,
        }
    }
    pub fn display(self, v: f32) -> String {
        match self {
            VoiceParam::Attack => format!("{v:.3}s"),
            VoiceParam::Release => format!("{v:.2}s"),
            VoiceParam::Cutoff => format!("{v:.0}Hz"),
            VoiceParam::Sweep => format!("{:.0}%", v * 100.0),
            VoiceParam::Drift => format!("{v:.1}ct"),
            VoiceParam::Level => format!("{:.0}%", v * 200.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VoiceView {
    pub id: VoiceId,
    pub name: String,
    pub builtin: bool,
}

#[derive(Clone, Debug)]
pub struct VoicePanelState {
    pub voices: Vec<VoiceView>,
    pub editing: Option<VoiceId>,
    pub draft: Option<VoicePreset>,
    pub track_name: String,
    pub audio_ready: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum VoiceAction {
    Close,
    Select(VoiceId),
    New,
    Save,
    SaveAs,
    Delete(VoiceId),
    Audition,
    AddPartial,
    RemovePartial(usize),
    BumpPartial(usize, PartialField, f32),
    SetPartialGain(usize, f32),
    SetParam(VoiceParam, f32),
}

type Handler = Rc<dyn Fn(VoiceAction, &mut Window, &mut App)>;

#[derive(gpui::IntoElement)]
pub struct VoicePanel {
    state: VoicePanelState,
    on_action: Option<Handler>,
}

impl VoicePanel {
    pub fn new(state: VoicePanelState) -> Self {
        Self {
            state,
            on_action: None,
        }
    }
    pub fn on_action(
        mut self,
        handler: impl Fn(VoiceAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

pub fn voice_panel(state: VoicePanelState) -> VoicePanel {
    VoicePanel::new(state)
}

fn action_button(
    id: &'static str,
    label: &'static str,
    variant: ButtonVariant,
    action: VoiceAction,
    handler: Option<Handler>,
) -> impl IntoElement {
    let mut btn = button(label)
        .id(ElementId::Name(id.into()))
        .selector(id)
        .variant(variant)
        .size(ButtonSize::Sm);
    if let Some(h) = handler {
        btn = btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            h(action.clone(), window, cx);
        });
    }
    btn
}

fn stepper(
    id: String,
    text: String,
    width: f32,
    dec: VoiceAction,
    inc: VoiceAction,
    handler: Option<Handler>,
) -> impl IntoElement {
    let p = theme::palette();
    let step_btn =
        |side: &'static str, glyph: &'static str, action: VoiceAction, h: Option<Handler>| {
            let mut btn = div()
                .id(ElementId::Name(format!("{id}-{side}").into()))
                .debug_selector({
                    let key = format!("{id}-{side}");
                    move || key.clone()
                })
                .flex()
                .items_center()
                .justify_center()
                .w(px(16.0))
                .h(px(16.0))
                .rounded(px(RADIUS_SM))
                .bg(theme::alpha(p.border_base, p.border_a * 1.6))
                .text_size(px(FONT_XS))
                .text_color(rgb(p.text_muted))
                .cursor_pointer()
                .hover({
                    let hover = theme::alpha(p.border_base, p.border_a * 3.0);
                    move |style| style.bg(hover)
                })
                .child(glyph);
            if let Some(h) = h {
                btn = btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    h(action.clone(), window, cx);
                });
            }
            btn
        };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .child(step_btn("m", "-", dec, handler.clone()))
        .child(
            div()
                .w(px(width))
                .text_size(px(FONT_XS))
                .text_color(rgb(p.text))
                .child(text),
        )
        .child(step_btn("p", "+", inc, handler))
}

impl RenderOnce for VoicePanel {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let h = self.on_action.clone();
        let editing = self.state.editing;
        let draft = self.state.draft;
        let audio_ready = self.state.audio_ready;
        let track_name = self.state.track_name.clone();
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .w(px(160.0))
            .flex_shrink_0();
        for v in self.state.voices.iter() {
            let id = v.id;
            let is_editing = editing == Some(id);
            let mut row = div()
                .id(ElementId::Name(format!("vitem{id}").into()))
                .debug_selector(move || format!("vitem-{id}"))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(6.0))
                .px(px(8.0))
                .h(px(26.0))
                .rounded(px(RADIUS_MD))
                .cursor_pointer()
                .bg(if is_editing {
                    theme::alpha(p.accent, 0.16)
                } else {
                    theme::alpha(p.border_base, 0.0)
                })
                .hover({
                    let hover = theme::alpha(p.border_base, p.border_a * 2.0);
                    move |style| style.bg(hover)
                })
                .child(
                    div()
                        .truncate()
                        .text_size(px(FONT_SM))
                        .text_color(rgb(if is_editing { p.text } else { p.text_muted }))
                        .child(v.name.clone()),
                );
            row = if v.builtin {
                row.child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(8.0))
                        .text_color(rgb(p.text_faint))
                        .child("内置"),
                )
            } else {
                let del = h.clone();
                let mut btn = div()
                    .id(ElementId::Name(format!("vdel{id}").into()))
                    .debug_selector(move || format!("vdel-{id}"))
                    .flex_shrink_0()
                    .text_size(px(8.5))
                    .text_color(rgb(p.text_faint))
                    .cursor_pointer()
                    .hover(move |style| style.text_color(rgb(p.danger)))
                    .child("删除");
                if let Some(del) = del {
                    btn = btn.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                        cx.stop_propagation();
                        del(VoiceAction::Delete(id), window, cx);
                    });
                }
                row.child(btn)
            };
            if let Some(sel) = h.clone() {
                row = row.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    sel(VoiceAction::Select(id), window, cx);
                });
            }
            list = list.child(row);
        }
        list = list.child(
            action_button(
                "pv-new",
                "+ 新建",
                ButtonVariant::Ghost,
                VoiceAction::New,
                h.clone(),
            )
            .into_any_element(),
        );
        let editor: gpui::AnyElement = match draft {
            None => div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_size(px(FONT_SM))
                .text_color(rgb(p.text_faint))
                .child("从左侧选择一个音色")
                .into_any_element(),
            Some(preset) => {
                let mut col = div().flex().flex_col().gap(px(5.0)).flex_1().min_w_0();
                let count = preset.count.min(MAX_PARTIALS);
                col = col.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(FONT_XS))
                                        .text_color(rgb(p.text_faint))
                                        .child(format!("分音 {count}/{MAX_PARTIALS}")),
                                )
                                .child({
                                    let full = count >= MAX_PARTIALS;
                                    let mut add = div()
                                        .id("pv-add")
                                        .debug_selector(|| "pv-add".into())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .w(px(18.0))
                                        .h(px(18.0))
                                        .rounded(px(RADIUS_SM))
                                        .bg(if full {
                                            theme::alpha(p.border_base, 0.0)
                                        } else {
                                            theme::alpha(p.border_base, p.border_a * 2.0)
                                        })
                                        .text_size(px(FONT_MD))
                                        .text_color(rgb(if full { p.text_faint } else { p.text_muted }))
                                        .child("+");
                                    if !full {
                                        add = add.cursor_pointer().hover({
                                            let hover = theme::alpha(p.accent, 0.22);
                                            move |style| style.bg(hover)
                                        });
                                        if let Some(add_h) = h.clone() {
                                            add = add.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                                add_h(VoiceAction::AddPartial, window, cx);
                                            });
                                        }
                                    }
                                    add
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(FONT_XS))
                                .text_color(rgb(p.text_faint))
                                .child(format!("轨道：{track_name}")),
                        ),
                );
                if count == 0 {
                    col = col.child(
                        div()
                            .id("pv-empty")
                            .debug_selector(|| "pv-empty".into())
                            .flex()
                            .items_center()
                            .h(px(20.0))
                            .text_size(px(FONT_XS))
                            .text_color(rgb(p.text_faint))
                            .child("这个音色还没有分音，点右上角 + 添加一个"),
                    );
                }
                for i in 0..count {
                    let partial = preset.partials[i];
                    let badge = div()
                        .id(ElementId::Name(format!("pv-row{i}").into()))
                        .debug_selector(move || format!("pv-row-{i}"))
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(18.0))
                        .h(px(18.0))
                        .rounded(px(RADIUS_SM))
                        .bg(rgb(p.accent))
                        .text_size(px(FONT_XS))
                        .text_color(rgb(0xffffff))
                        .child(format!("{}", i + 1));
                    let mut gain =
                        mini_slider(ElementId::Name(format!("pv-gain{i}").into()), partial.gain)
                            .range(0.0, 1.0)
                            .selector(format!("pv-gain-{i}"));
                    if let Some(set) = h.clone() {
                        gain = gain.on_change(move |v, window, cx| {
                            set(VoiceAction::SetPartialGain(i, v), window, cx)
                        });
                    }
                    col = col.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .h(px(20.0))
                            .child(badge)
                            .child(stepper(
                                format!("pv-ratio{i}"),
                                format!("{:.2}x", partial.ratio),
                                40.0,
                                VoiceAction::BumpPartial(i, PartialField::Ratio, -0.25),
                                VoiceAction::BumpPartial(i, PartialField::Ratio, 0.25),
                                h.clone(),
                            ))
                            .child(div().flex_1().min_w_0().child(gain))
                            .child(stepper(
                                format!("pv-decay{i}"),
                                format!("{:.1}", partial.decay_scale),
                                26.0,
                                VoiceAction::BumpPartial(i, PartialField::Decay, -0.1),
                                VoiceAction::BumpPartial(i, PartialField::Decay, 0.1),
                                h.clone(),
                            ))
                            .child({
                                let mut remove = div()
                                    .id(ElementId::Name(format!("pv-del{i}").into()))
                                    .debug_selector(move || format!("pv-del-{i}"))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .rounded(px(RADIUS_SM))
                                    .text_size(px(FONT_XS))
                                    .text_color(rgb(p.text_faint))
                                    .cursor_pointer()
                                    .hover(move |style| style.text_color(rgb(p.danger)))
                                    .child("×");
                                if let Some(del) = h.clone() {
                                    remove = remove.on_click(
                                        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                            del(VoiceAction::RemovePartial(i), window, cx);
                                        },
                                    );
                                }
                                remove
                            }),
                    );
                }
                col = col.child(div().h(px(1.0)).bg(theme::border()));
                col = col.child(
                    div()
                        .text_size(px(FONT_XS))
                        .text_color(rgb(p.text_faint))
                        .child("包络与滤波"),
                );
                for param in VoiceParam::ALL {
                    let value = param.get(&preset);
                    let (mn, mx) = param.range();
                    let mut bar =
                        mini_slider(ElementId::Name(format!("vp-{param:?}").into()), value)
                            .range(mn, mx)
                            .selector(format!("vp-{}", param.label()));
                    if let Some(set) = h.clone() {
                        bar = bar.on_change(move |v, window, cx| {
                            set(VoiceAction::SetParam(param, v), window, cx)
                        });
                    }
                    col = col.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .w(px(28.0))
                                    .flex_shrink_0()
                                    .text_size(px(FONT_XS))
                                    .text_color(rgb(p.text_muted))
                                    .child(param.label()),
                            )
                            .child(div().flex_1().min_w_0().child(bar))
                            .child(
                                div()
                                    .w(px(46.0))
                                    .flex_shrink_0()
                                    .text_align(gpui::TextAlign::Right)
                                    .text_size(px(FONT_XS))
                                    .text_color(rgb(p.text_faint))
                                    .child(param.display(value)),
                            ),
                    );
                }
                col = col
                    .child(div().flex_1())
                    .child(if audio_ready {
                        div().into_any_element()
                    } else {
                        div()
                            .id("pv-no-audio")
                            .debug_selector(|| "pv-no-audio".into())
                            .text_size(px(FONT_XS))
                            .text_color(rgb(p.danger))
                            .child("没有可用的音频输出设备，试听不会出声")
                            .into_any_element()
                    })
                    .child(div().h(px(1.0)).bg(theme::border()))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(action_button(
                                "pv-audition",
                                "试听",
                                ButtonVariant::Ghost,
                                VoiceAction::Audition,
                                h.clone(),
                            ))
                            .child(action_button(
                                "pv-save",
                                "保存",
                                ButtonVariant::Primary,
                                VoiceAction::Save,
                                h.clone(),
                            ))
                            .child(action_button(
                                "pv-saveas",
                                "另存为",
                                ButtonVariant::Secondary,
                                VoiceAction::SaveAs,
                                h.clone(),
                            ))
                            .child(div().flex_1())
                            .child(action_button(
                                "pv-close",
                                "关闭",
                                ButtonVariant::Ghost,
                                VoiceAction::Close,
                                h.clone(),
                            )),
                    );
                col.into_any_element()
            }
        };
        let mut close_btn = div()
            .id("voice-close")
            .debug_selector(|| "voice-close".into())
            .px(px(8.0))
            .py(px(2.0))
            .rounded(px(RADIUS_SM))
            .cursor_pointer()
            .text_size(px(FONT_SM))
            .text_color(rgb(p.text_faint))
            .hover(move |style| style.text_color(rgb(p.text)))
            .child("✕");
        if let Some(close) = h.clone() {
            close_btn =
                close_btn.on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    close(VoiceAction::Close, window, cx);
                });
        }
        let card = div()
            .id("voice-card")
            .debug_selector(|| "voice-card".into())
            .flex()
            .flex_col()
            .w(px(760.0))
            .h(px(470.0))
            .rounded(px(RADIUS_LG + 2.0))
            .border_1()
            .border_color(theme::border())
            .bg(rgb(p.panel))
            .shadow_lg()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(14.0))
                    .py(px(9.0))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(FONT_MD))
                            .text_color(rgb(p.text))
                            .child("音色管理"),
                    )
                    .child(close_btn),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(14.0))
                    .p(px(14.0))
                    .flex_1()
                    .min_h_0()
                    .child(list)
                    .child(editor),
            );
        let mut mask = div()
            .id("voice-mask")
            .debug_selector(|| "voice-mask".into())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme::alpha(0x000000, 0.45));
        if let Some(close) = h.clone() {
            mask = mask.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                close(VoiceAction::Close, window, cx);
            });
        }
        mask.child(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonora_engine::preset;
    #[test]
    fn params_round_trip_through_preset() {
        let mut p = preset::piano();
        for param in VoiceParam::ALL {
            let (mn, mx) = param.range();
            let mid = (mn + mx) / 2.0;
            param.set(&mut p, mid);
            assert!(
                (param.get(&p) - mid).abs() < 1e-3,
                "{:?} 写入后应能原样读回",
                param
            );
        }
    }
    #[test]
    fn param_ranges_survive_clamp() {
        for param in VoiceParam::ALL {
            let (mn, mx) = param.range();
            for target in [mn, mx] {
                let mut p = preset::piano();
                param.set(&mut p, target);
                p.clamp_params();
                let got = param.get(&p);
                assert!(
                    (got - target).abs() < 1e-3,
                    "{:?} 的端点 {target} 被 clamp 成 {got}",
                    param
                );
            }
        }
    }
    #[test]
    fn param_display_is_readable() {
        assert_eq!(VoiceParam::Sweep.display(0.5), "50%");
        assert_eq!(VoiceParam::Cutoff.display(8000.0), "8000Hz");
        assert_eq!(VoiceParam::Attack.display(0.25), "0.250s");
        assert_eq!(VoiceParam::Level.display(0.5), "100%");
    }
    #[test]
    fn partial_fields_are_distinct() {
        assert_ne!(PartialField::Ratio, PartialField::Decay);
    }
}
