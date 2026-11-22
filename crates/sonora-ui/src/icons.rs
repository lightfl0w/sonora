use gpui::{Hsla, PathBuilder, Window, point, px, size};

pub fn paint_play(window: &mut Window, x: f32, y: f32, s: f32, color: Hsla) {
    let mut b = PathBuilder::fill();
    b.move_to(point(px(x), px(y)));
    b.line_to(point(px(x + s), px(y + s * 0.5)));
    b.line_to(point(px(x), px(y + s)));
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

pub fn paint_pause(window: &mut Window, x: f32, y: f32, s: f32, color: Hsla) {
    let bar = s * 0.28;
    let gap = s * 0.18;
    window.paint_quad(gpui::fill(
        gpui::Bounds::new(point(px(x), px(y)), size(px(bar), px(s))),
        color,
    ));
    window.paint_quad(gpui::fill(
        gpui::Bounds::new(point(px(x + bar + gap), px(y)), size(px(bar), px(s))),
        color,
    ));
}

pub fn paint_skip_back(window: &mut Window, x: f32, y: f32, s: f32, color: Hsla) {
    let bar = s * 0.16;
    window.paint_quad(gpui::fill(
        gpui::Bounds::new(point(px(x), px(y)), size(px(bar), px(s))),
        color,
    ));
    let mut b = PathBuilder::fill();
    let tx = x + bar + s * 0.08;
    b.move_to(point(px(tx), px(y)));
    b.line_to(point(px(tx), px(y + s)));
    b.line_to(point(px(x + s), px(y + s * 0.5)));
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

pub fn paint_chevron_down(window: &mut Window, x: f32, y: f32, s: f32, color: Hsla) {
    let mut b = PathBuilder::stroke(px(1.5));
    b.move_to(point(px(x), px(y)));
    b.line_to(point(px(x + s * 0.5), px(y + s * 0.6)));
    b.line_to(point(px(x + s), px(y)));
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}
