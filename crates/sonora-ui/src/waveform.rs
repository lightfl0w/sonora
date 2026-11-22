use gpui::{
    App, Bounds, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, div,
    point, px, rgb, size,
};

use crate::theme::{self, FONT_XS};

pub const WAVE_SIDEBAR_W: f32 = 180.0;

#[derive(Clone, Debug, Default)]
pub struct WaveformState {
    pub samples: Vec<f32>,
    pub playing: bool,
    pub bar: u32,
    pub total_bars: u32,
    pub position_bars: f32,
    pub audio_ready: bool,
}

impl WaveformState {
    pub fn peak(&self) -> f32 {
        self.samples
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()))
            .clamp(0.0, 1.0)
    }
    pub fn progress(&self) -> f32 {
        if self.total_bars == 0 {
            return 0.0;
        }
        (self.position_bars / self.total_bars as f32).clamp(0.0, 1.0)
    }
}

#[derive(gpui::IntoElement)]
pub struct Waveform {
    state: WaveformState,
}

impl Waveform {
    pub fn new(state: WaveformState) -> Self {
        Self { state }
    }
}

pub fn waveform(state: WaveformState) -> Waveform {
    Waveform::new(state)
}

impl RenderOnce for Waveform {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let s = self.state;
        let samples = s.samples.clone();
        let playing = s.playing;
        let audio_ready = s.audio_ready;
        let progress = s.progress();
        let peak = s.peak();
        let wave_color = if playing {
            rgb(p.accent)
        } else {
            theme::alpha(p.border_base, p.border_a * 5.0)
        };
        let mid_color = theme::alpha(p.border_base, p.border_a * 2.0);
        let track = theme::alpha(p.border_base, p.border_a * 1.8);
        let accent = rgb(p.accent);
        let state_label = if !audio_ready {
            "音频不可用"
        } else if playing {
            "播放中"
        } else {
            "待机"
        };
        let scope = gpui::canvas(
            |_, _, _| {},
            move |b, _, window, _| {
                let w = b.size.width.as_f32();
                let h = b.size.height.as_f32();
                if w <= 0.0 || h <= 0.0 {
                    return;
                }
                let ox = b.origin.x.as_f32();
                let mid = b.origin.y.as_f32() + h / 2.0;
                window.paint_quad(gpui::fill(
                    Bounds::new(point(px(ox), px(mid)), size(px(w), px(1.0))),
                    mid_color,
                ));
                if samples.is_empty() {
                    return;
                }
                let cols = w.floor().max(1.0) as usize;
                let per = (samples.len() as f32 / cols as f32).max(1.0);
                let amp = h * 0.46;
                for i in 0..cols {
                    let start = ((i as f32 * per) as usize).min(samples.len());
                    let end = (((i + 1) as f32 * per) as usize).min(samples.len());
                    if start >= end {
                        continue;
                    }
                    let mut peak = 0.0f32;
                    for v in &samples[start..end] {
                        if v.abs() > peak {
                            peak = v.abs();
                        }
                    }
                    if peak <= 0.0008 {
                        continue;
                    }
                    let height = (peak.min(1.0) * amp).max(0.5);
                    window.paint_quad(gpui::fill(
                        Bounds::new(
                            point(px(ox + i as f32), px(mid - height)),
                            size(px(1.0), px(height * 2.0)),
                        ),
                        wave_color,
                    ));
                }
            },
        );
        div()
            .id("wave-sidebar")
            .debug_selector(|| "wave-sidebar".into())
            .flex()
            .flex_col()
            .w(px(WAVE_SIDEBAR_W))
            .flex_shrink_0()
            .bg(rgb(p.panel))
            .border_r_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(10.0))
                    .pt(px(8.0))
                    .child(
                        div()
                            .text_size(px(FONT_XS))
                            .text_color(rgb(p.text_faint))
                            .child("波形"),
                    )
                    .child(
                        div()
                            .id("wave-state")
                            .debug_selector(|| "wave-state".into())
                            .text_size(px(FONT_XS))
                            .text_color(rgb(p.text_faint))
                            .child(state_label),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(10.0))
                    .pt(px(2.0))
                    .text_size(px(FONT_XS))
                    .text_color(rgb(p.text_muted))
                    .child(format!("第 {} 小节", s.bar.max(1)))
                    .child(format!("峰值 {:.0}%", peak * 100.0)),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .my(px(8.0))
                    .child(scope.size_full()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .px(px(10.0))
                    .pb(px(10.0))
                    .child(
                        div().h(px(3.0)).w_full().rounded(px(1.5)).bg(track).child(
                            div()
                                .h_full()
                                .w(gpui::relative(progress))
                                .rounded(px(1.5))
                                .bg(accent),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(FONT_XS))
                            .text_color(rgb(p.text_faint))
                            .child(format!(
                                "{:.0}% {} 小节",
                                progress * 100.0,
                                s.total_bars.max(1)
                            )),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn peak_and_progress_are_clamped() {
        let s = WaveformState {
            samples: vec![0.2, -0.7, 0.5],
            total_bars: 8,
            position_bars: 2.0,
            ..Default::default()
        };
        assert!((s.peak() - 0.7).abs() < 1e-6);
        assert!((s.progress() - 0.25).abs() < 1e-6);
    }
    #[test]
    fn empty_or_zero_bars_do_not_divide_by_zero() {
        let s = WaveformState::default();
        assert_eq!(s.progress(), 0.0);
        assert_eq!(s.peak(), 0.0);
        let over = WaveformState {
            samples: vec![2.0],
            total_bars: 1,
            position_bars: 5.0,
            ..Default::default()
        };
        assert_eq!(over.progress(), 1.0);
        assert_eq!(over.peak(), 1.0);
    }
    #[test]
    fn silence_is_zero() {
        let s = WaveformState {
            samples: vec![0.0; 64],
            total_bars: 4,
            position_bars: 1.0,
            ..Default::default()
        };
        assert_eq!(s.peak(), 0.0);
    }
}
