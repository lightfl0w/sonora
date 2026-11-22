use gpui::{App, ClickEvent, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, rgb};

use crate::button::{Variant, button};
use crate::layout::TRANSPORT_H;
use crate::theme::{self, FONT_SM};

pub struct TransportState {
    pub playing: bool,
    pub bar: u32,
    pub position_bars: f32,
    pub total_bars: u32,
    pub bpm: f32,
    pub voices: u32,
}

impl TransportState {
    pub fn position_text(&self) -> String {
        let bars = self.total_bars.max(1);
        let pos = self.position_bars.clamp(0.0, bars as f32);
        let bar = (pos.floor() as u32 + 1).clamp(1, bars);
        let frac = (pos - (bar - 1) as f32).clamp(0.0, 0.999);
        let beat = (1 + (frac * 4.0).floor() as u32).min(4);
        format!("{} . {}", bar, beat)
    }
}

#[derive(gpui::IntoElement)]
pub struct TransportBar {
    state: TransportState,
    on_play: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_stop: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_rewind: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl TransportBar {
    pub fn new(state: TransportState) -> Self {
        Self {
            state,
            on_play: None,
            on_stop: None,
            on_rewind: None,
        }
    }
    pub fn on_play(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_play = Some(Box::new(handler));
        self
    }
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Box::new(handler));
        self
    }
    pub fn on_rewind(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_rewind = Some(Box::new(handler));
        self
    }
}

fn divider() -> impl IntoElement {
    div()
        .w(px(1.0))
        .h(px(20.0))
        .flex_shrink_0()
        .bg(theme::border())
}

fn readout(label: &'static str, value: String, accent: bool) -> impl IntoElement {
    let p = theme::palette();
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(1.0))
        .px(px(9.0))
        .child(
            div()
                .text_size(px(9.0))
                .text_color(rgb(p.text_faint))
                .child(label),
        )
        .child(
            div()
                .text_size(px(if accent { 15.0 } else { 12.0 }))
                .text_color(if accent { rgb(p.accent) } else { rgb(p.text) })
                .child(value),
        )
}

impl RenderOnce for TransportBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let playing = self.state.playing;
        let mut play_btn = button(if playing { "暂停" } else { "播放" })
            .variant(if playing {
                Variant::Primary
            } else {
                Variant::Secondary
            })
            .on_click({
                let handler = self.on_play;
                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    if let Some(h) = &handler {
                        h(window, cx);
                    }
                }
            });
        if playing {
            play_btn = play_btn.selected(true);
        }
        let mut stop_btn = button("停止").ghost();
        stop_btn = stop_btn.on_click({
            let handler = self.on_stop;
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(window, cx);
                }
            }
        });
        let mut rewind = button("复位").ghost();
        rewind = rewind.on_click({
            let handler = self.on_rewind;
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(window, cx);
                }
            }
        });
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(TRANSPORT_H))
            .flex_shrink_0()
            .px(px(10.0))
            .gap(px(8.0))
            .bg(rgb(p.panel))
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(3.0))
                    .child(rewind)
                    .child(play_btn)
                    .child(stop_btn),
            )
            .child(divider())
            .child(readout("位置", self.state.position_text(), true))
            .child(divider())
            .child(readout(
                "小节",
                format!("{} / {}", self.state.bar, self.state.total_bars),
                false,
            ))
            .child(divider())
            .child(readout("速度", format!("{:.0} BPM", self.state.bpm), false))
            .child(divider())
            .child(readout("发声", format!("{}", self.state.voices), false))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .child(div().w(px(7.0)).h(px(7.0)).rounded(px(4.0)).bg(if playing {
                        rgb(p.live)
                    } else {
                        rgb(p.card_hi)
                    }))
                    .child(
                        div()
                            .text_size(px(FONT_SM))
                            .text_color(rgb(if playing { p.live } else { p.text_faint }))
                            .child(if playing { "播放中" } else { "已停止" }),
                    ),
            )
    }
}

pub fn transport_bar(state: TransportState) -> TransportBar {
    TransportBar::new(state)
}

pub fn position_text(bar: u32, position_bars: f32, total_bars: u32) -> String {
    TransportState {
        playing: false,
        bar,
        position_bars,
        total_bars,
        bpm: 120.0,
        voices: 0,
    }
    .position_text()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(bar: u32, pos: f32, total: u32) -> TransportState {
        TransportState {
            playing: false,
            bar,
            position_bars: pos,
            total_bars: total,
            bpm: 120.0,
            voices: 0,
        }
    }
    #[test]
    fn position_shows_bar_and_beat() {
        assert_eq!(state(1, 0.0, 8).position_text(), "1 . 1");
        assert_eq!(state(1, 0.5, 8).position_text(), "1 . 3");
        assert_eq!(state(2, 1.0, 8).position_text(), "2 . 1");
    }
    #[test]
    fn position_clamps_to_total_bars() {
        assert_eq!(state(99, 98.0, 8).position_text(), "8 . 4");
        assert_eq!(state(1, -5.0, 8).position_text(), "1 . 1");
    }
    #[test]
    fn beat_stays_within_one_to_four() {
        for i in 0..100 {
            let pos = i as f32 * 0.1;
            let text = state(1, pos, 8).position_text();
            let beat: u32 = text.split(" . ").nth(1).unwrap().parse().unwrap();
            assert!((1..=4).contains(&beat), "beat {beat} 越界，输入 {pos}");
        }
    }
    #[test]
    fn free_function_matches_struct() {
        assert_eq!(position_text(3, 2.5, 8), state(3, 2.5, 8).position_text());
    }
    #[test]
    fn zero_total_bars_does_not_divide() {
        assert!(!state(1, 0.0, 0).position_text().is_empty());
    }
}
