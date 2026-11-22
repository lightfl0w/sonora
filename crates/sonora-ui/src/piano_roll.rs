use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Bounds, DragMoveEvent, ElementId, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Pixels, RenderOnce, ScrollEvent,
    StatefulInteractiveElement, Styled, Window, div, point, px, rgb, size,
};

use crate::layout::{HIGH_PITCH, KEY_COL_W, LOW_PITCH, ROW_H_MAX, ROW_H_MIN, is_black_key};
use crate::theme::{self, FONT_XS};

pub const RESIZE_EDGE: f32 = 7.0;
pub const RULER_H: f32 = 22.0;
pub const ZOOM_MIN: f32 = 1.0;
pub const ZOOM_MAX: f32 = 32.0;
const DRAG_THRESHOLD: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct NoteView {
    pub bar: u32,
    pub beat: f32,
    pub midi: u8,
    pub len: f32,
    pub vel: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DragMode {
    Pan {
        start_x: f32,
        start_scroll: f32,
    },
    MoveNote {
        index: usize,
        grab_offset_bars: f32,
    },
    ResizeNote {
        index: usize,
        start_bar: f32,
        orig_len: f32,
    },
    DrawNote {
        index: usize,
        start_beat: f32,
    },
    LoopSelect {
        anchor: f32,
    },
    LoopEdge {
        start: bool,
    },
    Marquee {
        anchor_bar: f32,
        anchor_row: f32,
    },
    Scrub,
}

#[derive(Clone, Debug)]
pub enum RollAction {
    Seek(f32),
    ScrollBars(f32),
    ScrollPitch(isize),
    Zoom {
        zoom: f32,
        scroll: f32,
    },
    Create {
        bar: u32,
        beat: f32,
        midi: u8,
        len: f32,
    },
    Delete {
        index: usize,
    },
    Select {
        index: usize,
        additive: bool,
    },
    ClearSelection,
    SetLoop {
        a: f32,
        b: f32,
    },
    MarqueeStart {
        bar: f32,
        row: f32,
        additive: bool,
    },
    MarqueeTo {
        bar: f32,
        row: f32,
    },
    Move {
        index: usize,
        bar: u32,
        beat: f32,
        midi: u8,
    },
    Resize {
        index: usize,
        len: f32,
    },
    DragStart(DragMode),
    DragEnd,
}

#[derive(Clone, Debug)]
pub struct RollState {
    pub notes: Vec<NoteView>,
    pub selected: Vec<usize>,
    pub bars: u32,
    pub zoom: f32,
    pub scroll: f32,
    pub pitch_scroll: usize,
    pub row_h: f32,
    pub grid_beats: f32,
    pub playhead: f32,
    pub color: u32,
    pub drums: bool,
    pub scale_root: u8,
    pub in_scale: Vec<u8>,
    pub snap_to_scale: bool,
    pub drag: Option<DragMode>,
    pub loop_bars: Option<(f32, f32)>,
    pub loop_enabled: bool,
    pub marquee: Option<(f32, f32, f32, f32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteRect {
    pub index: usize,
    pub x: f32,
    pub w: f32,
    pub pitch: u8,
}

#[derive(gpui::IntoElement)]

pub struct PianoRoll {
    state: RollState,
    on_action: Option<Rc<dyn Fn(RollAction, &mut Window, &mut App)>>,
    view_h: Option<Rc<Cell<f32>>>,
    canvas_rect: Option<Rc<Cell<(f32, f32, f32)>>>,
}

impl PianoRoll {
    pub fn new(state: RollState) -> Self {
        Self {
            state,
            on_action: None,
            view_h: None,
            canvas_rect: None,
        }
    }
    pub fn on_action(mut self, h: impl Fn(RollAction, &mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(h));
        self
    }
    pub fn view_h_cell(mut self, cell: Rc<Cell<f32>>) -> Self {
        self.view_h = Some(cell);
        self
    }
    pub fn canvas_rect_cell(mut self, cell: Rc<Cell<(f32, f32, f32)>>) -> Self {
        self.canvas_rect = Some(cell);
        self
    }
}

pub fn visible_rows(drums: bool) -> Vec<u8> {
    if drums {
        vec![49, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36]
    } else {
        (LOW_PITCH..=HIGH_PITCH).rev().collect()
    }
}

pub fn note_rects(notes: &[NoteView]) -> Vec<NoteRect> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.midi >= LOW_PITCH && n.midi <= HIGH_PITCH)
        .map(|(index, n)| NoteRect {
            index,
            x: (n.bar.saturating_sub(1) as f32) + (n.beat - 1.0) / 4.0,
            w: (n.len / 4.0).max(1.0 / 32.0),
            pitch: n.midi,
        })
        .collect()
}

pub fn hit_note(
    rects: &[NoteRect],
    bar_w: f32,
    scroll: f32,
    row_h: f32,
    rows: &[u8],
    x: f32,
    y: f32,
) -> Option<(usize, bool)> {
    for r in rects.iter().rev() {
        let nx = (r.x - scroll) * bar_w;
        let nw = (r.w * bar_w).max(4.0);
        let ry = rows.iter().position(|&p| p == r.pitch)?;
        let ny = ry as f32 * row_h;
        if x >= nx && x <= nx + nw && y >= ny && y <= ny + row_h {
            let on_edge = x >= nx + nw - RESIZE_EDGE && nw > RESIZE_EDGE * 2.5;
            return Some((r.index, on_edge));
        }
    }
    None
}

pub fn hit_note_any(
    rects: &[NoteRect],
    bar_w: f32,
    scroll: f32,
    row_h: f32,
    rows: &[u8],
    pitch_scroll: usize,
    x: f32,
    y: f32,
) -> Option<(usize, bool)> {
    for r in rects.iter().rev() {
        let nx = (r.x - scroll) * bar_w;
        let nw = (r.w * bar_w).max(4.0);
        let Some(abs_row) = rows.iter().position(|&p| p == r.pitch) else {
            continue;
        };
        if abs_row < pitch_scroll {
            continue;
        }
        let ny = (abs_row - pitch_scroll) as f32 * row_h;
        if x >= nx && x <= nx + nw && y >= ny && y <= ny + row_h {
            let on_edge = x >= nx + nw - RESIZE_EDGE && nw > RESIZE_EDGE * 2.5;
            return Some((r.index, on_edge));
        }
    }
    None
}

pub fn snap_beats(beats: f32, grid: f32) -> f32 {
    if grid <= 0.0 {
        return beats;
    }
    (beats / grid).round() * grid
}

pub fn next_gap_beats(notes: &[NoteView], exclude: usize, start: f32, midi: u8) -> Option<f32> {
    let mut best: Option<f32> = None;
    for (i, n) in notes.iter().enumerate() {
        if i == exclude || n.midi != midi {
            continue;
        }
        let s = (n.bar.saturating_sub(1)) as f32 * 4.0 + (n.beat - 1.0);
        if s <= start + 1e-4 {
            continue;
        }
        let gap = s - start;
        if best.map_or(true, |b| gap < b) {
            best = Some(gap);
        }
    }
    best
}

pub fn pitch_name(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let pc = (pitch % 12) as usize;
    let octave = pitch as i32 / 12 - 1;
    format!("{}{}", NAMES[pc], octave)
}

pub fn clamp_zoom(z: f32) -> f32 {
    z.clamp(ZOOM_MIN, ZOOM_MAX)
}

pub fn clamp_row_h(h: f32) -> f32 {
    h.clamp(ROW_H_MIN, ROW_H_MAX)
}

pub fn visible_count(view_h: f32, row_h: f32) -> usize {
    ((view_h / row_h).ceil() as usize).max(1)
}

pub fn max_pitch_scroll(rows: usize, view_h: f32, row_h: f32) -> usize {
    rows.saturating_sub(visible_count(view_h, row_h))
}

pub fn bar_width(view_w: f32, bars: u32, zoom: f32) -> f32 {
    let visible = (bars.max(1) as f32 / clamp_zoom(zoom)).max(0.001);
    (view_w / visible).max(1.0)
}

pub fn pitch_at_row(rows: &[u8], pitch_scroll: usize, y: f32, row_h: f32) -> Option<u8> {
    if row_h <= 0.0 || rows.is_empty() {
        return None;
    }
    let vis = (y / row_h).floor().max(0.0) as usize;
    let idx = (pitch_scroll + vis).min(rows.len() - 1);
    rows.get(idx).copied()
}

pub fn note_start_beats(n: &NoteView) -> f32 {
    (n.bar.saturating_sub(1)) as f32 * 4.0 + (n.beat - 1.0)
}

pub fn note_overlaps(notes: &[NoteView], midi: u8, start: f32, end: f32) -> bool {
    notes.iter().any(|n| {
        if n.midi != midi {
            return false;
        }
        let s = note_start_beats(n);
        let e = s + n.len;
        start < e - 1e-4 && s < end - 1e-4
    })
}

pub fn snap_pitch_to_scale(pitch: u8, drums: bool, snap: bool, in_scale: &[u8]) -> u8 {
    if drums || !snap || in_scale.is_empty() || in_scale.contains(&pitch) {
        return pitch;
    }
    for d in 1..=6u8 {
        let up = pitch.saturating_add(d);
        if up <= HIGH_PITCH && in_scale.contains(&up) {
            return up;
        }
        let down = pitch.saturating_sub(d);
        if down >= LOW_PITCH && in_scale.contains(&down) {
            return down;
        }
    }
    pitch
}

fn key_column(state: &RollState, view_h: f32) -> impl IntoElement {
    let p = theme::palette();
    let rows = visible_rows(state.drums);
    let row_h = clamp_row_h(state.row_h);
    let count = visible_count(view_h, row_h);
    let start = state.pitch_scroll.min(rows.len());
    let end = (start + count).min(rows.len());
    let in_scale = state.in_scale.clone();
    let scale_root = state.scale_root;
    let drums = state.drums;
    let light_keys = theme::mode() == crate::ThemeMode::Light;
    let mut col = div()
        .flex()
        .flex_col()
        .w(px(KEY_COL_W))
        .flex_shrink_0()
        .h_full()
        .min_h_0()
        .overflow_hidden()
        .border_r_1()
        .border_color(theme::border())
        .bg(rgb(p.panel));
    for &pitch in &rows[start..end] {
        let pc = pitch % 12;
        let black = is_black_key(pitch);
        let on_scale = drums || in_scale.is_empty() || in_scale.contains(&pitch);
        let is_root = !drums && pc == scale_root;
        let is_octave = !drums && pc == 0;
        let label_color = if is_root {
            rgb(p.track_color(0))
        } else if is_octave {
            rgb(p.text)
        } else {
            rgb(p.text_faint)
        };
        let key_bg = if drums {
            theme::alpha(p.blackkey, p.blackkey_a * 2.5)
        } else {
            theme::alpha(0x000000, 0.0)
        };
        let black_key_bg = theme::alpha(p.blackkey, if light_keys { 0.86 } else { 0.55 });
        let mut row = div()
            .relative()
            .h(px(row_h))
            .flex_shrink_0()
            .w_full()
            .border_b_1()
            .border_color(theme::alpha(p.border_base, 0.10))
            .bg(key_bg);
        if black {
            row = row.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .right(px(0.0))
                    .h_full()
                    .w(px(KEY_COL_W * 0.62))
                    .bg(black_key_bg),
            );
        }
        col = col.child(
            row.child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .pr(px(5.0))
                    .text_size(px(FONT_XS - 2.0))
                    .text_color(label_color)
                    .opacity(if !drums && !on_scale { 0.45 } else { 1.0 })
                    .child(if black {
                        String::new()
                    } else {
                        pitch_name(pitch)
                    }),
            ),
        );
    }
    col
}

impl RenderOnce for PianoRoll {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let p = theme::palette();
        let state = self.state.clone();
        let handler = self.on_action.clone();
        let bounds: Rc<RefCell<Bounds<Pixels>>> = Rc::new(RefCell::new(Bounds::default()));
        let view_h: Option<Rc<Cell<f32>>> = self.view_h.clone();
        let bounds_pre = bounds.clone();
        let bounds_down = bounds.clone();
        let bounds_mid = bounds.clone();
        let bounds_move = bounds.clone();
        let bounds_right = bounds.clone();
        let bounds_scroll = bounds.clone();
        let view_pre = view_h.clone();
        let drag_mode = state.drag;
        let snap = state.snap_to_scale;
        let loop_bars = state.loop_bars;
        let loop_enabled = state.loop_enabled;
        let marquee = state.marquee;
        let ruler_bounds: Rc<RefCell<Bounds<Pixels>>> = Rc::new(RefCell::new(Bounds::default()));
        let ruler_pre = ruler_bounds.clone();
        let ruler_down = ruler_bounds.clone();
        let rows = visible_rows(state.drums);
        let row_h = clamp_row_h(state.row_h);
        let bars = state.bars.max(1);
        let zoom = clamp_zoom(state.zoom);
        let scroll = state.scroll;
        let pitch_scroll = state.pitch_scroll;
        let playhead = state.playhead;
        let grid_beats = state.grid_beats;
        let color = state.color;
        let drums = state.drums;
        let scale_root = state.scale_root;
        let in_scale = state.in_scale.clone();
        let in_scale_down = in_scale.clone();
        let in_scale_drag = in_scale.clone();
        let rects = note_rects(&state.notes);
        let notes = state.notes.clone();
        let selected_notes = state.selected.clone();
        let panel = rgb(p.panel);
        let light = theme::mode() == crate::ThemeMode::Light;
        let band_black = theme::alpha(0x000000, if light { 0.055 } else { 0.20 });
        let band_off = theme::alpha(0x000000, if light { 0.03 } else { 0.12 });
        let line_bar = theme::alpha(p.border_base, 0.16);
        let line_beat = theme::alpha(p.border_base, 0.06);
        let line_grid = theme::alpha(p.border_base, 0.02);
        let gloss = theme::alpha(0xffffff, p.note_gloss);
        let root_mark = theme::alpha(p.track_color(0), 0.75);
        let head = rgb(p.text);
        let faint = rgb(p.text_faint);
        let rows_canvas = rows.clone();
        let rows_down_v = rows.clone();
        let rows_down_c = rows.clone();
        let rows_move_v = rows.clone();
        let rows_right_v = rows.clone();
        let notes_down = notes.clone();
        let notes_move = notes.clone();
        let rects_down = rects.clone();
        let rects_right = rects.clone();
        let act_down = handler.clone();
        let act_mid_down = handler.clone();
        let act_move = handler.clone();
        let act_up = handler.clone();
        let act_up_out = handler.clone();
        let act_up_mid = handler.clone();
        let act_up_out_mid = handler.clone();
        let act_right = handler.clone();
        let act_scroll = handler.clone();
        let act_ruler = handler.clone();
        let canvas_rect_cell = self.canvas_rect.clone();
        let canvas = gpui::canvas(
            move |b, _, _| {
                *bounds_pre.borrow_mut() = b;
                if let Some(cell) = &view_pre {
                    cell.set(b.size.height.as_f32());
                }
                if let Some(cell) = &canvas_rect_cell {
                    cell.set((
                        b.origin.x.as_f32(),
                        b.origin.y.as_f32(),
                        b.size.width.as_f32(),
                    ));
                }
                b.size
            },
            move |b, _, window, _| {
                let ox = b.origin.x.as_f32();
                let oy = b.origin.y.as_f32();
                let w = b.size.width.as_f32();
                let h = b.size.height.as_f32();
                let bar_w = bar_width(w, bars, zoom);
                let vis = visible_count(h, row_h);
                let start = pitch_scroll.min(rows_canvas.len());
                let end = (start + vis).min(rows_canvas.len());
                let view_rows = &rows_canvas[start..end];
                window.paint_quad(gpui::fill(b, panel));
                for (row, &pitch) in view_rows.iter().enumerate() {
                    let y = oy + row as f32 * row_h;
                    let band = Bounds::new(point(px(ox), px(y)), size(px(w), px(row_h)));
                    let pc = pitch % 12;
                    let black = !drums && is_black_key(pitch);
                    if black {
                        window.paint_quad(gpui::fill(band, band_black));
                    } else if !drums && !in_scale.is_empty() && !in_scale.contains(&pitch) {
                        window.paint_quad(gpui::fill(band, band_off));
                    }
                    if !drums && pc == scale_root {
                        window.paint_quad(gpui::fill(
                            Bounds::new(point(px(ox), px(y)), size(px(3.0), px(row_h))),
                            root_mark,
                        ));
                    }
                }
                let first = scroll.floor().max(0.0) as usize;
                let last = (scroll + w / bar_w).ceil() as usize;
                for bar in first..=last {
                    let x = ox + (bar as f32 - scroll) * bar_w;
                    if x < ox - 2.0 || x > ox + w + 2.0 {
                        continue;
                    }
                    let in_range = (bar as u32) < bars;
                    window.paint_quad(gpui::fill(
                        Bounds::new(point(px(x), px(oy)), size(px(1.0), px(h))),
                        if !in_range {
                            theme::alpha(p.border_base, 0.03)
                        } else if bar % 4 == 0 {
                            line_bar
                        } else {
                            line_beat
                        },
                    ));
                }
                for bar in first..last {
                    let in_range = (bar as u32) < bars;
                    for beat in 1..4 {
                        let x = ox + (bar as f32 + beat as f32 / 4.0 - scroll) * bar_w;
                        if x >= ox && x <= ox + w {
                            window.paint_quad(gpui::fill(
                                Bounds::new(point(px(x), px(oy)), size(px(1.0), px(h))),
                                theme::alpha(p.border_base, if in_range { 0.04 } else { 0.015 }),
                            ));
                        }
                    }
                }
                let end_x = ox + (bars as f32 - scroll) * bar_w;
                if end_x >= ox && end_x <= ox + w {
                    window.paint_quad(gpui::fill(
                        Bounds::new(point(px(end_x), px(oy)), size(px(1.5), px(h))),
                        theme::alpha(p.border_base, 0.16),
                    ));
                }
                let grid_bars = (grid_beats / 4.0).max(1.0 / 32.0);
                if bar_w * grid_bars > 6.0 {
                    let mut pos = (scroll / grid_bars).floor() * grid_bars;
                    let mut guard = 0;
                    while pos < scroll + w / bar_w && guard < 4096 {
                        let x = ox + (pos - scroll) * bar_w;
                        if x >= ox && x <= ox + w {
                            window.paint_quad(gpui::fill(
                                Bounds::new(point(px(x), px(oy)), size(px(1.0), px(h))),
                                line_grid,
                            ));
                        }
                        pos += grid_bars;
                        guard += 1;
                    }
                }
                for r in &rects {
                    let nx = ox + (r.x - scroll) * bar_w;
                    let nw = (r.w * bar_w).max(4.0);
                    if nx + nw < ox || nx > ox + w {
                        continue;
                    }
                    let Some(ry) = view_rows.iter().position(|&q| q == r.pitch) else {
                        continue;
                    };
                    let ny = oy + ry as f32 * row_h;
                    let clip = (ox - nx).max(0.0);
                    let visible_w = ((w - (nx - ox)).min(nw) - clip).max(0.0);
                    if visible_w <= 1.0 {
                        continue;
                    }
                    let rect = Bounds::new(
                        point(px(nx + clip), px(ny + 0.5)),
                        size(px(visible_w), px((row_h - 1.0).max(2.0))),
                    );
                    let selected = selected_notes.contains(&r.index);
                    let vel = notes
                        .get(r.index)
                        .map(|n| n.vel.clamp(0.0, 1.0))
                        .unwrap_or(0.8);
                    window.paint_quad(gpui::quad(
                        rect,
                        px(3.0),
                        if selected {
                            theme::alpha(color, 1.0)
                        } else {
                            theme::alpha(color, 0.28 + 0.62 * vel)
                        },
                        if selected { px(1.5) } else { px(0.0) },
                        if selected {
                            rgb(p.text)
                        } else {
                            theme::alpha(0x000000, 0.0)
                        },
                        gpui::BorderStyle::default(),
                    ));
                    if rect.size.height.as_f32() > 5.0 {
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(
                                    px(rect.origin.x.as_f32() + 0.5),
                                    px(rect.origin.y.as_f32() + 0.5),
                                ),
                                size(px((visible_w - 1.0).max(1.0)), px(1.0)),
                            ),
                            gloss,
                        ));
                    }
                }
                let hx = ox + (playhead - scroll) * bar_w;
                if hx >= ox && hx <= ox + w {
                    window.paint_quad(gpui::fill(
                        Bounds::new(point(px(hx), px(oy)), size(px(1.5), px(h))),
                        head,
                    ));
                }
                if let Some((bx0, by0, bx1, by1)) = marquee {
                    let x0 = ox + (bx0.min(bx1) - scroll) * bar_w;
                    let x1 = ox + (bx0.max(bx1) - scroll) * bar_w;
                    let y0 = oy + (by0.min(by1) - pitch_scroll as f32) * row_h;
                    let y1 = oy + (by0.max(by1) - pitch_scroll as f32) * row_h + row_h;
                    let left = x0.max(ox);
                    let right = x1.min(ox + w);
                    let top = y0.max(oy);
                    let bottom = y1.min(oy + h);
                    if right > left && bottom > top {
                        window.paint_quad(gpui::quad(
                            Bounds::new(
                                point(px(left), px(top)),
                                size(px(right - left), px(bottom - top)),
                            ),
                            px(2.0),
                            theme::alpha(p.accent, 0.12),
                            px(1.0),
                            theme::alpha(p.accent, 0.7),
                            gpui::BorderStyle::default(),
                        ));
                    }
                }
                let _ = faint;
            },
        );
        let ruler_w = Rc::new(RefCell::new(0.0f32));
        let ruler_w_canvas = ruler_w.clone();
        let ruler_w_click = ruler_w.clone();
        let ruler_canvas = gpui::canvas(
            move |b, _, _| {
                *ruler_pre.borrow_mut() = b;
                *ruler_w_canvas.borrow_mut() = b.size.width.as_f32();
                b.size
            },
            move |b, _, window, _| {
                let ox = b.origin.x.as_f32();
                let w = b.size.width.as_f32();
                let visible = (bars as f32 / zoom).max(0.001);
                let bw = (w / visible).max(1.0);
                if let Some((a, b_end)) = loop_bars {
                    let x0 = ox + (a - scroll) * bw;
                    let x1 = ox + (b_end - scroll) * bw;
                    let left = x0.max(ox);
                    let right = x1.min(ox + w);
                    if right > left {
                        let inside = playhead >= a && playhead < b_end;
                        let alpha = if !loop_enabled {
                            0.08
                        } else if inside {
                            0.34
                        } else {
                            0.20
                        };
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(px(left), px(b.origin.y.as_f32())),
                                size(px(right - left), px(RULER_H)),
                            ),
                            theme::alpha(p.accent, alpha),
                        ));
                    }
                    for edge in [x0, x1] {
                        if edge >= ox && edge <= ox + w {
                            window.paint_quad(gpui::fill(
                                Bounds::new(
                                    point(px(edge), px(b.origin.y.as_f32())),
                                    size(px(1.5), px(RULER_H)),
                                ),
                                theme::alpha(p.accent, if loop_enabled { 0.9 } else { 0.35 }),
                            ));
                        }
                    }
                }
                let mut step = 1u32;
                while (step as f32) * bw < 40.0 && step < 64 {
                    step *= 2;
                }
                let first = (scroll / step as f32).floor() as u32 * step;
                let mut bar = first;
                let mut guard = 0;
                while guard < 512 {
                    let x = ox + (bar as f32 - scroll) * bw;
                    if x > ox + w {
                        break;
                    }
                    if x >= ox - 1.0 {
                        let in_range = bar < bars;
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(px(x), px(b.origin.y.as_f32())),
                                size(px(1.0), px(RULER_H)),
                            ),
                            theme::alpha(p.border_base, if in_range { 0.18 } else { 0.07 }),
                        ));
                    }
                    bar += step;
                    guard += 1;
                }
                let hx = ox + (playhead - scroll) * bw;
                if hx >= ox && hx <= ox + w {
                    window.paint_quad(gpui::fill(
                        Bounds::new(
                            point(px(hx), px(b.origin.y.as_f32())),
                            size(px(1.5), px(RULER_H)),
                        ),
                        head,
                    ));
                }
            },
        );
        let mut labels = div()
            .relative()
            .flex()
            .flex_row()
            .w_full()
            .h(px(RULER_H))
            .overflow_hidden();
        {
            let visible = (bars as f32 / zoom).max(0.001);
            let label_w = (*ruler_w.borrow()).max(1.0);
            let est_w = if label_w > 1.0 { label_w } else { 900.0 };
            let mut step = 1u32;
            while (step as f32) * (est_w / visible) < 40.0 && step < 64 {
                step *= 2;
            }
            let first = (scroll / step as f32).floor() as u32 * step;
            let mut bar = first;
            let mut guard = 0;
            while guard < 512 {
                let ratio = (bar as f32 - scroll) / visible;
                if ratio > 1.0 {
                    break;
                }
                if ratio >= 0.0 {
                    let in_range = bar < bars;
                    labels = labels.child(
                        div()
                            .absolute()
                            .left(gpui::relative(ratio))
                            .top(px(4.0))
                            .pl(px(4.0))
                            .text_size(px(FONT_XS))
                            .text_color(rgb(p.text_faint))
                            .opacity(if in_range { 1.0 } else { 0.45 })
                            .child(format!("{}", bar + 1)),
                    );
                }
                bar += step;
                guard += 1;
            }
        }
        let ruler = div()
            .relative()
            .flex()
            .flex_row()
            .w_full()
            .h(px(RULER_H))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(panel)
            .border_b_1()
            .border_color(theme::border())
            .child(ruler_canvas.absolute().inset_0())
            .child(labels)
            .id(ElementId::Name("roll-ruler".into()))
            .debug_selector(|| "roll-ruler".into())
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                    let b = *ruler_down.borrow();
                    let w = (*ruler_w_click.borrow()).max(1.0);
                    let visible = (bars as f32 / zoom).max(0.001);
                    let bw = (w / visible).max(1.0);
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    let target = (scroll + x / bw).max(0.0);
                    if let Some(h) = &act_ruler {
                        if let Some((a, b_end)) = loop_bars {
                            let near_start = (x - (a - scroll) * bw).abs() <= 8.0;
                            let near_end = (x - (b_end - scroll) * bw).abs() <= 8.0;
                            if near_start || near_end {
                                h(
                                    RollAction::DragStart(DragMode::LoopEdge { start: near_start }),
                                    window,
                                    cx,
                                );
                                return;
                            }
                        }
                        if ev.modifiers.shift {
                            h(
                                RollAction::SetLoop {
                                    a: target,
                                    b: target,
                                },
                                window,
                                cx,
                            );
                            h(
                                RollAction::DragStart(DragMode::LoopSelect { anchor: target }),
                                window,
                                cx,
                            );
                        } else {
                            h(RollAction::Seek(target), window, cx);
                            h(RollAction::DragStart(DragMode::Scrub), window, cx);
                        }
                    }
                },
            )
            .on_drag(DragMarker, { move |_, _, _, cx| cx.new(|_| DragGhost) });
        let body = div()
            .relative()
            .flex_1()
            .h_full()
            .min_w_0()
            .overflow_hidden()
            .child(canvas.absolute().inset_0())
            .id(ElementId::Name("roll-body".into()))
            .on_mouse_down(
                MouseButton::Left,
                move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                    let Some(act) = &act_down else { return };
                    let b = *bounds_down.borrow();
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    let y = ev.position.y.as_f32() - b.origin.y.as_f32();
                    let bar_w = bar_width(b.size.width.as_f32(), bars, zoom);
                    let pos_bars = x / bar_w + scroll;
                    if ev.modifiers.control || ev.modifiers.platform {
                        act(
                            RollAction::DragStart(DragMode::Pan {
                                start_x: x,
                                start_scroll: scroll,
                            }),
                            window,
                            cx,
                        );
                        return;
                    }
                    if let Some((index, on_edge)) = hit_note_any(
                        &rects_down,
                        bar_w,
                        scroll,
                        row_h,
                        &rows_down_v,
                        pitch_scroll,
                        x,
                        y,
                    ) {
                        act(
                            RollAction::Select {
                                index,
                                additive: ev.modifiers.shift,
                            },
                            window,
                            cx,
                        );
                        let n = &notes_down[index];
                        let note_start = note_start_beats(n);
                        let mode = if on_edge {
                            DragMode::ResizeNote {
                                index,
                                start_bar: pos_bars,
                                orig_len: n.len,
                            }
                        } else {
                            DragMode::MoveNote {
                                index,
                                grab_offset_bars: pos_bars - note_start / 4.0,
                            }
                        };
                        act(RollAction::DragStart(mode), window, cx);
                        return;
                    }
                    if pos_bars < 0.0 {
                        return;
                    }
                    if ev.modifiers.shift {
                        let row = y / row_h + pitch_scroll as f32;
                        act(
                            RollAction::MarqueeStart {
                                bar: pos_bars,
                                row,
                                additive: true,
                            },
                            window,
                            cx,
                        );
                        act(
                            RollAction::DragStart(DragMode::Marquee {
                                anchor_bar: pos_bars,
                                anchor_row: row,
                            }),
                            window,
                            cx,
                        );
                        return;
                    }
                    act(RollAction::ClearSelection, window, cx);
                    let start_beats = snap_beats((pos_bars * 4.0).max(0.0), grid_beats);
                    let bar = (start_beats / 4.0).floor() as u32 + 1;
                    let beat = (start_beats % 4.0) + 1.0;
                    let Some(pitch_raw) = pitch_at_row(&rows_down_c, pitch_scroll, y, row_h) else {
                        return;
                    };
                    let pitch = snap_pitch_to_scale(pitch_raw, drums, snap, &in_scale_down);
                    let len = grid_beats.max(1.0);
                    if note_overlaps(&notes_down, pitch, start_beats, start_beats + len) {
                        return;
                    }
                    act(
                        RollAction::Create {
                            bar,
                            beat: beat.clamp(1.0, 4.0),
                            midi: pitch,
                            len,
                        },
                        window,
                        cx,
                    );
                    act(
                        RollAction::DragStart(DragMode::DrawNote {
                            index: notes_down.len(),
                            start_beat: start_beats,
                        }),
                        window,
                        cx,
                    );
                },
            )
            .on_drag(DragMarker, { move |_, _, _, cx| cx.new(|_| DragGhost) })
            .on_mouse_down(
                MouseButton::Right,
                move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                    let Some(act) = &act_right else { return };
                    let b = *bounds_right.borrow();
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    let y = ev.position.y.as_f32() - b.origin.y.as_f32();
                    let bar_w = bar_width(b.size.width.as_f32(), bars, zoom);
                    if let Some((index, _)) = hit_note_any(
                        &rects_right,
                        bar_w,
                        scroll,
                        row_h,
                        &rows_right_v,
                        pitch_scroll,
                        x,
                        y,
                    ) {
                        act(RollAction::Delete { index }, window, cx);
                    }
                },
            )
            .on_scroll(move |ev: &ScrollEvent, window: &mut Window, cx: &mut App| {
                let Some(act) = &act_scroll else { return };
                let (dx, dy) = match ev.delta {
                    gpui::ScrollDelta::Pixels(p) => (p.x.as_f32(), p.y.as_f32()),
                    gpui::ScrollDelta::Lines(p) => (p.x * 20.0, p.y * 20.0),
                };
                let b = *bounds_scroll.borrow();
                let w = b.size.width.as_f32().max(1.0);
                let zoom_key = ev.modifiers.control || ev.modifiers.platform;
                if zoom_key {
                    let mx = ev.position.x.as_f32() - b.origin.x.as_f32();
                    let anchor_bar = scroll + (mx / w) * bars as f32 / zoom;
                    let factor = (dy * 0.004).exp();
                    let new_zoom = clamp_zoom(zoom * factor);
                    let new_scroll = anchor_bar - (mx / w) * bars as f32 / new_zoom;
                    act(
                        RollAction::Zoom {
                            zoom: new_zoom,
                            scroll: new_scroll,
                        },
                        window,
                        cx,
                    );
                } else if ev.modifiers.shift || dx.abs() > dy.abs() {
                    let d = if dx.abs() > dy.abs() { dx } else { dy };
                    let bar_w = bar_width(w, bars, zoom);
                    act(RollAction::ScrollBars(scroll - d / bar_w), window, cx);
                } else {
                    let step = (-dy / row_h).round() as isize;
                    if step != 0 {
                        act(RollAction::ScrollPitch(step), window, cx);
                    }
                }
            });
        let view_h_now = view_h
            .as_ref()
            .map(|c| c.get())
            .filter(|h| *h > 1.0)
            .unwrap_or(600.0);
        let outer_move_act = act_move.clone();
        let outer_bounds = bounds_move.clone();
        let outer_rows = rows_move_v.clone();
        let outer_notes = notes_move.clone();
        let outer_up = act_up.clone();
        let outer_up_out = act_up_out.clone();
        let outer_pan_act = act_move.clone();
        let outer_pan_bounds = bounds_move.clone();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .min_w_0()
            .on_mouse_down(
                MouseButton::Middle,
                move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                    let b = *bounds_mid.borrow();
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    if let Some(act) = &act_mid_down {
                        act(
                            RollAction::DragStart(DragMode::Pan {
                                start_x: x,
                                start_scroll: scroll,
                            }),
                            window,
                            cx,
                        );
                    }
                },
            )
            .on_mouse_up(
                MouseButton::Middle,
                move |_: &gpui::MouseUpEvent, window, cx| {
                    if let Some(act) = &act_up_mid {
                        act(RollAction::DragEnd, window, cx);
                    }
                },
            )
            .on_mouse_up_out(
                MouseButton::Middle,
                move |_: &gpui::MouseUpEvent, window, cx| {
                    if let Some(act) = &act_up_out_mid {
                        act(RollAction::DragEnd, window, cx);
                    }
                },
            )
            .on_drag_move::<DragMarker>(move |ev: &DragMoveEvent<DragMarker>, window, cx| {
                let Some(mode) = drag_mode else { return };
                let Some(act) = &outer_move_act else { return };
                let b = *outer_bounds.borrow();
                let x = ev.event.position.x.as_f32() - b.origin.x.as_f32();
                let y = ev.event.position.y.as_f32() - b.origin.y.as_f32();
                let bar_w = bar_width(b.size.width.as_f32(), bars, zoom);
                let pos_bars = x / bar_w + scroll;
                match mode {
                    DragMode::Pan {
                        start_x,
                        start_scroll,
                    } => {
                        let dx = x - start_x;
                        if dx.abs() > DRAG_THRESHOLD {
                            act(
                                RollAction::ScrollBars(start_scroll - dx / bar_w),
                                window,
                                cx,
                            );
                        }
                    }
                    DragMode::MoveNote {
                        index,
                        grab_offset_bars,
                    } => {
                        let target = pos_bars - grab_offset_bars;
                        let snapped = snap_beats((target * 4.0).max(0.0), grid_beats);
                        let bar = (snapped / 4.0).floor() as u32 + 1;
                        let beat = (snapped % 4.0) + 1.0;
                        let Some(pitch_raw) = pitch_at_row(&outer_rows, pitch_scroll, y, row_h)
                        else {
                            return;
                        };
                        let pitch = snap_pitch_to_scale(pitch_raw, drums, snap, &in_scale_drag);
                        act(
                            RollAction::Move {
                                index,
                                bar,
                                beat: beat.clamp(1.0, 4.0),
                                midi: pitch,
                            },
                            window,
                            cx,
                        );
                    }
                    DragMode::ResizeNote {
                        index,
                        start_bar,
                        orig_len,
                    } => {
                        let delta_beats = (pos_bars - start_bar) * 4.0;
                        let mut new_len = snap_beats((orig_len + delta_beats).max(0.0), grid_beats);
                        if let Some(n) = outer_notes.get(index) {
                            let start = note_start_beats(n);
                            if let Some(gap) = next_gap_beats(&outer_notes, index, start, n.midi) {
                                new_len = new_len.min(gap);
                            }
                        }
                        let len = new_len.max(grid_beats);
                        act(RollAction::Resize { index, len }, window, cx);
                    }
                    DragMode::DrawNote { index, start_beat } => {
                        let cursor_beats = pos_bars * 4.0;
                        let raw = (cursor_beats - start_beat).max(0.0);
                        let mut len = snap_beats(raw.max(grid_beats), grid_beats);
                        if let Some(n) = outer_notes.get(index) {
                            if let Some(gap) =
                                next_gap_beats(&outer_notes, index, start_beat, n.midi)
                            {
                                len = len.min(gap);
                            }
                        }
                        act(
                            RollAction::Resize {
                                index,
                                len: len.min(64.0),
                            },
                            window,
                            cx,
                        );
                    }
                    DragMode::Scrub => {
                        let w = b.size.width.as_f32().max(1.0);
                        let bw = (w / (bars as f32 / zoom).max(0.001)).max(1.0);
                        let x = ev.event.position.x.as_f32() - b.origin.x.as_f32();
                        let target = (scroll + x / bw).max(0.0);
                        act(RollAction::Seek(target), window, cx);
                    }
                    DragMode::LoopSelect { anchor } => {
                        act(
                            RollAction::SetLoop {
                                a: anchor,
                                b: pos_bars.max(0.0),
                            },
                            window,
                            cx,
                        );
                    }
                    DragMode::LoopEdge { start } => {
                        if let Some((a, b_end)) = loop_bars {
                            let (na, nb) = if start {
                                (pos_bars.max(0.0), b_end)
                            } else {
                                (a, pos_bars.max(0.0))
                            };
                            act(RollAction::SetLoop { a: na, b: nb }, window, cx);
                        }
                    }
                    DragMode::Marquee { .. } => {
                        act(
                            RollAction::MarqueeTo {
                                bar: pos_bars.max(0.0),
                                row: y / row_h + pitch_scroll as f32,
                            },
                            window,
                            cx,
                        );
                    }
                }
            })
            .on_mouse_move(
                move |ev: &gpui::MouseMoveEvent, window: &mut Window, cx: &mut App| {
                    if ev.pressed_button != Some(MouseButton::Middle) {
                        return;
                    }
                    let Some(DragMode::Pan {
                        start_x,
                        start_scroll,
                    }) = drag_mode
                    else {
                        return;
                    };
                    let Some(act) = &outer_pan_act else { return };
                    let b = *outer_pan_bounds.borrow();
                    let x = ev.position.x.as_f32() - b.origin.x.as_f32();
                    let bar_w = bar_width(b.size.width.as_f32(), bars, zoom);
                    let dx = x - start_x;
                    if dx.abs() > DRAG_THRESHOLD {
                        act(
                            RollAction::ScrollBars(start_scroll - dx / bar_w),
                            window,
                            cx,
                        );
                    }
                },
            )
            .on_mouse_up(
                MouseButton::Left,
                move |_: &gpui::MouseUpEvent, window, cx| {
                    if let Some(act) = &outer_up {
                        act(RollAction::DragEnd, window, cx);
                    }
                },
            )
            .on_mouse_up_out(
                MouseButton::Left,
                move |_: &gpui::MouseUpEvent, window, cx| {
                    if let Some(act) = &outer_up_out {
                        act(RollAction::DragEnd, window, cx);
                    }
                },
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h(px(RULER_H))
                    .flex_shrink_0()
                    .child(
                        div()
                            .w(px(KEY_COL_W))
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(theme::border())
                            .bg(panel),
                    )
                    .child(div().flex_1().min_w_0().child(ruler)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(key_column(&state, view_h_now))
                    .child(body),
            )
    }
}

pub fn piano_roll(state: RollState) -> PianoRoll {
    PianoRoll::new(state)
}

pub fn default_pitch_scroll(drums: bool, view_h: f32, row_h: f32) -> usize {
    let rows = visible_rows(drums);
    if drums {
        return 0;
    }
    let c4 = rows.iter().position(|&p| p == 60).unwrap_or(0);
    let vis = visible_count(view_h, row_h);
    c4.saturating_sub(vis / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn note(bar: u32, beat: f32, midi: u8, len: f32) -> NoteView {
        NoteView {
            bar,
            beat,
            midi,
            len,
            vel: 0.8,
        }
    }
    fn rows() -> Vec<u8> {
        visible_rows(false)
    }
    #[test]
    fn rows_descend_from_high_pitch() {
        let r = rows();
        assert_eq!(r.first(), Some(&HIGH_PITCH));
        assert_eq!(r.last(), Some(&LOW_PITCH));
        assert!(r.windows(2).all(|w| w[0] == w[1] + 1));
    }
    #[test]
    fn drum_rows_are_fixed() {
        let r = visible_rows(true);
        assert_eq!(r.len(), 12);
        assert!(r.contains(&36));
        assert!(r.contains(&49));
    }
    #[test]
    fn note_rect_uses_bar_coordinates() {
        let rects = note_rects(&[note(2, 3.0, 60, 2.0)]);
        assert!((rects[0].x - 1.5).abs() < 1e-5);
        assert!((rects[0].w - 0.5).abs() < 1e-5);
    }
    #[test]
    fn hit_note_finds_body_mid_area() {
        let notes = vec![note(1, 1.0, 60, 4.0)];
        let rects = note_rects(&notes);
        let r = rows();
        let ry = r.iter().position(|&p| p == 60).unwrap();
        let cy = ry as f32 * 14.0 + 7.0;
        let hit = hit_note_any(&rects, 100.0, 0.0, 14.0, &r, 0, 20.0, cy);
        assert_eq!(hit.map(|(i, e)| (i, e)), Some((0, false)));
    }
    #[test]
    fn hit_note_reports_edge_near_right() {
        let notes = vec![note(1, 1.0, 60, 4.0)];
        let rects = note_rects(&notes);
        let r = rows();
        let ry = r.iter().position(|&p| p == 60).unwrap();
        let cy = ry as f32 * 14.0 + 7.0;
        let hit = hit_note_any(&rects, 100.0, 0.0, 14.0, &r, 0, 99.0, cy);
        assert_eq!(hit.map(|(i, e)| (i, e)), Some((0, true)));
    }
    #[test]
    fn hit_note_ignores_other_pitch_rows() {
        let notes = vec![note(1, 1.0, 60, 1.0)];
        let rects = note_rects(&notes);
        let r = rows();
        let ry = r.iter().position(|&p| p == 72).unwrap();
        let cy = ry as f32 * 14.0 + 7.0;
        assert!(hit_note_any(&rects, 100.0, 0.0, 14.0, &r, 0, 10.0, cy).is_none());
    }
    #[test]
    fn hit_note_accounts_for_pitch_scroll() {
        let notes = vec![note(1, 1.0, 60, 4.0)];
        let rects = note_rects(&notes);
        let r = rows();
        let abs_row = r.iter().position(|&p| p == 60).unwrap();
        let scroll = abs_row.saturating_sub(5);
        assert!(scroll > 0, "测试前提：滚动量必须大于 0");
        let visible_row = (abs_row - scroll) as f32;
        let cy = visible_row * 14.0 + 7.0;
        let hit = hit_note_any(&rects, 100.0, 0.0, 14.0, &r, scroll, 20.0, cy);
        assert_eq!(
            hit.map(|(i, _)| i),
            Some(0),
            "滚动 {} 行后，音符应落在可视区第 {} 行，命中失败说明坐标未减 pitch_scroll",
            scroll,
            visible_row as usize
        );
    }
    #[test]
    fn hit_note_skips_rows_above_viewport() {
        let notes = vec![note(1, 1.0, 60, 4.0)];
        let rects = note_rects(&notes);
        let r = rows();
        let abs_row = r.iter().position(|&p| p == 60).unwrap();
        let scroll = abs_row + 3;
        assert!(
            hit_note_any(&rects, 100.0, 0.0, 14.0, &r, scroll, 20.0, 7.0).is_none(),
            "音符在视口上方时不应被命中"
        );
    }
    #[test]
    fn hit_note_stacked_returns_topmost() {
        let notes = vec![note(1, 1.0, 60, 1.0), note(1, 1.0, 60, 1.0)];
        let rects = note_rects(&notes);
        let r = rows();
        let ry = r.iter().position(|&p| p == 60).unwrap();
        let cy = ry as f32 * 14.0 + 7.0;
        let hit = hit_note_any(&rects, 100.0, 0.0, 14.0, &r, 0, 10.0, cy).unwrap();
        assert_eq!(hit.0, 1);
    }
    #[test]
    fn snap_beats_respects_grid() {
        assert!((snap_beats(1.4, 1.0) - 1.0).abs() < 1e-5);
        assert!((snap_beats(1.6, 1.0) - 2.0).abs() < 1e-5);
        assert!((snap_beats(1.3, 0.5) - 1.5).abs() < 1e-5);
        assert!((snap_beats(2.3, 0.0) - 2.3).abs() < 1e-5);
    }
    #[test]
    fn next_gap_limits_overlap() {
        let notes = vec![note(1, 1.0, 60, 1.0), note(2, 1.0, 60, 1.0)];
        let gap = next_gap_beats(&notes, 0, 0.0, 60).unwrap();
        assert!((gap - 4.0).abs() < 1e-5);
    }
    #[test]
    fn next_gap_ignores_different_pitches() {
        let notes = vec![note(1, 1.0, 60, 1.0), note(2, 1.0, 62, 1.0)];
        assert!(next_gap_beats(&notes, 0, 0.0, 60).is_none());
    }
    #[test]
    fn pitch_names_use_scientific_notation() {
        assert_eq!(pitch_name(60), "C4");
        assert_eq!(pitch_name(61), "C#4");
        assert_eq!(pitch_name(21), "A0");
        assert_eq!(pitch_name(108), "C8");
    }
    #[test]
    fn zoom_clamps_to_range() {
        assert_eq!(clamp_zoom(0.1), ZOOM_MIN);
        assert_eq!(clamp_zoom(1000.0), ZOOM_MAX);
    }
    #[test]
    fn row_height_clamps() {
        assert_eq!(clamp_row_h(1.0), ROW_H_MIN);
        assert_eq!(clamp_row_h(999.0), ROW_H_MAX);
    }
    #[test]
    fn bar_width_follows_zoom() {
        let a = bar_width(1000.0, 8, 1.0);
        let b = bar_width(1000.0, 8, 2.0);
        assert!((b - a * 2.0).abs() < 1e-4, "放大一倍，小节宽度应翻倍");
    }
    #[test]
    fn pitch_at_row_maps_viewport() {
        let r = rows();
        let pitch = pitch_at_row(&r, 0, 7.0, 14.0).unwrap();
        assert_eq!(pitch, r[0]);
        let p2 = pitch_at_row(&r, 0, 21.0, 14.0).unwrap();
        assert_eq!(p2, r[1]);
        assert_eq!(pitch_at_row(&r, 0, 0.0, 0.0), None);
    }
    #[test]
    fn pitch_scroll_never_exceeds_rows() {
        let r = rows().len();
        let max = max_pitch_scroll(r, 400.0, 14.0);
        assert!(max < r);
        assert_eq!(max_pitch_scroll(10, 5000.0, 14.0), 0);
    }
    #[test]
    fn default_scroll_centers_middle_c() {
        let scroll = default_pitch_scroll(false, 400.0, 14.0);
        let r = rows();
        let c4 = r.iter().position(|&p| p == 60).unwrap();
        let vis = visible_count(400.0, 14.0);
        assert!(c4 >= scroll, "C4 应在可视区内");
        assert!(c4 < scroll + vis, "C4 应在可视区内");
    }
    #[test]
    fn drag_modes_compare() {
        let a = DragMode::Pan {
            start_x: 1.0,
            start_scroll: 2.0,
        };
        let b = a;
        assert_eq!(a, b);
        assert_ne!(
            DragMode::MoveNote {
                index: 0,
                grab_offset_bars: 0.0
            },
            DragMode::ResizeNote {
                index: 0,
                start_bar: 0.0,
                orig_len: 1.0
            }
        );
        assert_ne!(
            DragMode::Scrub,
            DragMode::Pan {
                start_x: 0.0,
                start_scroll: 0.0
            }
        );
    }
    #[test]
    fn pitch_at_row_clamps_to_last_row() {
        let r = rows();
        let beyond = r.len() as f32 * 20.0 + 100.0;
        assert_eq!(pitch_at_row(&r, 0, beyond, 20.0), Some(*r.last().unwrap()));
        assert_eq!(pitch_at_row(&r, 0, -5.0, 20.0), Some(r[0]));
    }
    #[test]
    fn note_overlaps_detects_same_pitch_collisions() {
        let notes = vec![note(1, 1.0, 60, 2.0)];
        assert!(note_overlaps(&notes, 60, 1.0, 2.0), "完全重叠应命中");
        assert!(
            note_overlaps(&notes, 60, 1.5, 3.0),
            "与既有音符区间相交应命中"
        );
        assert!(
            !note_overlaps(&notes, 60, 2.0, 3.0),
            "紧接其后的音符不算重叠"
        );
        assert!(!note_overlaps(&notes, 62, 1.0, 2.0), "不同音高互不影响");
    }
    #[test]
    fn note_start_beats_maps_bar_and_beat() {
        assert!((note_start_beats(&note(1, 1.0, 60, 1.0))).abs() < 1e-5);
        assert!((note_start_beats(&note(2, 3.0, 60, 1.0)) - 6.0).abs() < 1e-5);
    }
    fn major_scale() -> Vec<u8> {
        const MAJOR: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
        (LOW_PITCH..=HIGH_PITCH)
            .filter(|p| MAJOR.contains(&(p % 12)))
            .collect()
    }
    #[test]
    fn snap_pitch_respects_flags_and_scale() {
        let scale = major_scale();
        assert_eq!(
            snap_pitch_to_scale(60, false, true, &scale),
            60,
            "调内音不变"
        );
        assert_eq!(
            snap_pitch_to_scale(61, false, true, &scale),
            62,
            "C#4 优先吸附到 D4"
        );
        assert_eq!(
            snap_pitch_to_scale(60, false, false, &scale),
            60,
            "关闭吸附时不改"
        );
        assert_eq!(
            snap_pitch_to_scale(38, true, true, &scale),
            38,
            "鼓轨不吸附"
        );
        assert_eq!(
            snap_pitch_to_scale(60, false, true, &[]),
            60,
            "空音阶表不改"
        );
    }
    #[test]
    fn snap_pitch_moves_to_nearest_neighbor() {
        let scale = major_scale();
        assert_eq!(
            snap_pitch_to_scale(63, false, true, &scale),
            64,
            "D#4 向上吸附到 E4"
        );
        assert_eq!(
            snap_pitch_to_scale(65, false, true, &scale),
            65,
            "F4 本身就是调内音"
        );
        assert_eq!(snap_pitch_to_scale(66, false, true, &scale), 67);
    }
    #[test]
    fn visible_count_is_at_least_one() {
        assert_eq!(visible_count(0.0, 14.0), 1);
        assert_eq!(visible_count(14.0, 14.0), 1);
        assert_eq!(visible_count(28.0, 14.0), 2);
    }
}

#[derive(Clone, Copy)]
pub struct DragMarker;

pub struct DragGhost;

impl gpui::Render for DragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}
