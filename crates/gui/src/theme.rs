//! Colours and global egui style.

use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x19);
pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1b, 0x22);
pub const CARD: Color32 = Color32::from_rgb(0x20, 0x24, 0x2d);
pub const CARD_HI: Color32 = Color32::from_rgb(0x2a, 0x2f, 0x3a);
pub const BORDER: Color32 = Color32::from_rgb(0x32, 0x38, 0x46);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xea, 0xf0);
pub const MUTED: Color32 = Color32::from_rgb(0x8a, 0x92, 0xa6);
pub const ACCENT: Color32 = Color32::from_rgb(0x8b, 0x7b, 0xff);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x3a, 0x34, 0x6e);
pub const OK: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x97);
pub const WARN: Color32 = Color32::from_rgb(0xff, 0xb4, 0x54);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x6b, 0x6b);

pub const KEYCAP: Color32 = Color32::from_rgb(0x2c, 0x31, 0x3d);
pub const KEYCAP_HOVER: Color32 = Color32::from_rgb(0x35, 0x3b, 0x49);
pub const KEYCAP_TOP: Color32 = Color32::from_rgb(0x37, 0x3d, 0x4b);
pub const DEVICE: Color32 = Color32::from_rgb(0x0d, 0x0f, 0x13);

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.global_style_mut(|style| {
        use egui::FontFamily::{Monospace, Proportional};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, Proportional)),
            (TextStyle::Body, FontId::new(14.0, Proportional)),
            (TextStyle::Button, FontId::new(14.0, Proportional)),
            (TextStyle::Small, FontId::new(11.5, Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 30.0;

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = PANEL;
        v.window_fill = CARD;
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.window_corner_radius = CornerRadius::same(12);
        v.extreme_bg_color = BG;
        v.faint_bg_color = CARD;
        v.override_text_color = None;
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = ACCENT_DIM;
        v.selection.stroke = Stroke::new(1.0, ACCENT);

        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = CARD;
        w.noninteractive.weak_bg_fill = CARD;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = CARD_HI;
        w.inactive.weak_bg_fill = CARD_HI;
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = KEYCAP_HOVER;
        w.hovered.weak_bg_fill = KEYCAP_HOVER;
        w.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
        w.hovered.fg_stroke = Stroke::new(1.5, TEXT);
        w.hovered.corner_radius = r;
        w.active.bg_fill = ACCENT_DIM;
        w.active.weak_bg_fill = ACCENT_DIM;
        w.active.bg_stroke = Stroke::new(1.0, ACCENT);
        w.active.fg_stroke = Stroke::new(1.5, TEXT);
        w.active.corner_radius = r;
        w.open = w.active;
    });
}
