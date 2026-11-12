//! Hand-drawn widgets: keycaps, knobs, pills and cards.

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::*;

/// A titled card section.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(16)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(RichText::new(title.to_uppercase()).small().color(MUTED).strong());
                ui.add_space(4.0);
            }
            add(ui)
        })
        .inner
}

/// Rounded toggle pill.
pub fn pill(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let (fill, stroke, color) = if selected {
        (ACCENT_DIM, Stroke::new(1.0, ACCENT), TEXT)
    } else {
        (CARD_HI, Stroke::new(1.0, BORDER), MUTED)
    };
    ui.add(egui::Button::new(RichText::new(text).color(color)).fill(fill).stroke(stroke).corner_radius(15))
}

/// Prominent action button.
pub fn primary(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let (fill, color) = if enabled { (ACCENT, Color32::WHITE) } else { (CARD_HI, MUTED) };
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(color).strong()).fill(fill).corner_radius(8),
    )
}

/// Quiet toolbar button.
pub fn ghost(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).color(TEXT)).fill(Color32::TRANSPARENT).corner_radius(8),
    )
}

/// Draw text centred in `rect`, shrinking/truncating to fit its width.
fn fit_text(ui: &Ui, rect: Rect, text: &str, max_size: f32, color: Color32) {
    let painter = ui.painter();
    let mut size = max_size;
    loop {
        let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(size), color);
        if galley.size().x <= rect.width() || size <= 10.0 {
            if galley.size().x <= rect.width() {
                painter.galley(rect.center() - galley.size() / 2.0, galley, color);
            } else {
                let mut t: String = text.chars().collect();
                while !t.is_empty() {
                    t.pop();
                    let g = painter.layout_no_wrap(format!("{t}…"), FontId::proportional(size), color);
                    if g.size().x <= rect.width() {
                        painter.galley(rect.center() - g.size() / 2.0, g, color);
                        break;
                    }
                }
            }
            return;
        }
        size -= 1.0;
    }
}

/// A physical-looking keycap.
pub fn keycap(ui: &mut Ui, size: Vec2, index: &str, text: &str, led: Option<Color32>, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let r = 10;
    let pressed = resp.is_pointer_button_down_on();
    let body = if pressed { rect.translate(egui::vec2(0.0, 2.0)) } else { rect };

    // LED glow behind the cap
    if let Some(c) = led.filter(|c| *c != Color32::BLACK) {
        for (i, a) in [(6.0, 18u8), (3.0, 40u8)] {
            painter.rect_filled(rect.expand(i), r + i as u8, Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a));
        }
    }
    // shadow + body + top face
    painter.rect_filled(rect.translate(egui::vec2(0.0, 4.0)), r, Color32::from_black_alpha(140));
    let fill = if resp.hovered() { KEYCAP_HOVER } else { KEYCAP };
    painter.rect_filled(body, r, fill);
    let top = Rect::from_min_max(body.min + egui::vec2(5.0, 4.0), body.max - egui::vec2(5.0, 9.0));
    painter.rect_filled(top, r - 3, if resp.hovered() { KEYCAP_HOVER.gamma_multiply(1.15) } else { KEYCAP_TOP });

    // LED strip at the bottom edge (only when the LED is actually lit)
    if let Some(c) = led.filter(|c| *c != Color32::BLACK) {
        let strip = Rect::from_min_max(
            Pos2::new(body.left() + 14.0, body.bottom() - 6.0),
            Pos2::new(body.right() - 14.0, body.bottom() - 3.0),
        );
        painter.rect_filled(strip, 2, c);
    }
    if selected {
        painter.rect_stroke(body.expand(2.0), r + 2, Stroke::new(2.0, ACCENT), StrokeKind::Outside);
    }

    painter.text(top.left_top() + egui::vec2(7.0, 5.0), Align2::LEFT_TOP, index, FontId::proportional(10.5), MUTED);
    let empty = text.is_empty();
    let label_rect = top.shrink2(egui::vec2(8.0, 0.0)).translate(egui::vec2(0.0, 4.0));
    fit_text(ui, label_rect, if empty { "—" } else { text }, 15.0, if empty { MUTED } else { TEXT });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Width reserved for one knob widget.
pub const KNOB_WIDTH: f32 = 184.0;

/// Which part of a knob was clicked.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KnobPart {
    Left,
    Press,
    Right,
}

/// A rotary knob drawn in one fixed-size rect with three hit zones:
/// ↺ (turn left), the knob itself (press), ↻ (turn right). Returns the clicked part.
pub fn knob(ui: &mut Ui, label: &str, selected: Option<KnobPart>, texts: [&str; 3]) -> Option<KnobPart> {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(KNOB_WIDTH, 136.0), Sense::hover());
    let id = ui.scope_id().with(("knob", label));
    let c = Pos2::new(rect.center().x, rect.top() + 46.0);
    let zones = [
        (KnobPart::Left, Rect::from_center_size(c - egui::vec2(66.0, 0.0), egui::vec2(38.0, 38.0))),
        (KnobPart::Press, Rect::from_center_size(c, egui::vec2(80.0, 80.0))),
        (KnobPart::Right, Rect::from_center_size(c + egui::vec2(66.0, 0.0), egui::vec2(38.0, 38.0))),
    ];
    let mut clicked = None;
    for (part, zone) in zones {
        let resp = ui.interact(zone, id.with(part as u8), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let sel = selected == Some(part);
        let p = ui.painter();
        match part {
            KnobPart::Press => {
                p.circle_filled(c + egui::vec2(0.0, 4.0), 38.0, Color32::from_black_alpha(140));
                p.circle_filled(c, 38.0, if resp.hovered() { KEYCAP_HOVER } else { KEYCAP });
                p.circle_filled(c, 30.0, KEYCAP_TOP);
                for i in 0..24 {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    let d = egui::vec2(a.cos(), a.sin());
                    p.line_segment([c + d * 31.5, c + d * 37.0], Stroke::new(1.0, BORDER));
                }
                p.line_segment([c - egui::vec2(0.0, 26.0), c - egui::vec2(0.0, 16.0)], Stroke::new(3.0, MUTED));
                if sel {
                    p.circle_stroke(c, 42.0, Stroke::new(2.0, ACCENT));
                }
            }
            _ => {
                let glyph = if part == KnobPart::Left { "↺" } else { "↻" };
                let fill = if sel { ACCENT_DIM } else if resp.hovered() { KEYCAP_HOVER } else { CARD_HI };
                p.circle_filled(zone.center(), 17.0, fill);
                p.circle_stroke(zone.center(), 17.0, Stroke::new(1.0, if sel { ACCENT } else { BORDER }));
                p.text(zone.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(17.0), if sel { TEXT } else { MUTED });
            }
        }
        if resp.clicked() {
            clicked = Some(part);
        }
    }
    let p = ui.painter();
    p.text(Pos2::new(c.x, rect.top() + 100.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), MUTED);
    // Only mention the parts that actually do something.
    let line: Vec<String> =
        ["↺", "●", "↻"].iter().zip(texts).filter(|(_, t)| !t.is_empty()).map(|(g, t)| format!("{g} {t}")).collect();
    if !line.is_empty() {
        p.text(Pos2::new(c.x, rect.top() + 120.0), Align2::CENTER_CENTER, line.join("  "), FontId::proportional(11.0), TEXT);
    }
    clicked
}

/// Small coloured dot + text (status chip).
pub fn status_chip(ui: &mut Ui, color: Color32, text: &str) {
    egui::Frame::new().fill(CARD).stroke(Stroke::new(1.0, BORDER)).corner_radius(14).inner_margin(egui::Margin::symmetric(10, 4)).show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, color);
                ui.label(RichText::new(text).small().color(TEXT));
            });
        },
    );
}
