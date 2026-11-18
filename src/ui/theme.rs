//! Современная тёмная тема оформления.

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const BG: Color32 = Color32::from_rgb(0x0E, 0x11, 0x16);
pub const PANEL: Color32 = Color32::from_rgb(0x14, 0x19, 0x22);
pub const CARD: Color32 = Color32::from_rgb(0x18, 0x1E, 0x28);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x2F, 0x3D);
pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xED, 0xF3);
pub const DIM: Color32 = Color32::from_rgb(0x8B, 0x98, 0xA9);
pub const ACCENT: Color32 = Color32::from_rgb(0x4C, 0x8D, 0xFF);
pub const OK: Color32 = Color32::from_rgb(0x34, 0xD3, 0x99);
pub const WARN: Color32 = Color32::from_rgb(0xFB, 0xBF, 0x24);
pub const DANGER: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);
pub const EDGE: Color32 = Color32::from_rgb(0xC0, 0x84, 0xFC);

/// Применить тему к контексту egui.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.interact_size.y = 30.0;
    style.spacing.scroll = egui::style::ScrollStyle {
        bar_width: 8.0_f32,
        floating_allocated_width: 8.0_f32,
        ..style.spacing.scroll
    };
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::new(19.0, egui::FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::new(14.0, egui::FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::new(14.0, egui::FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::new(12.0, egui::FontFamily::Proportional));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::new(13.0, egui::FontFamily::Monospace));
    ctx.set_style(style);
}

/// Палитра визуалов.
pub fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.faint_bg_color = Color32::from_rgb(0x11, 0x15, 0x1C);
    v.extreme_bg_color = Color32::from_rgb(0x0A, 0x0D, 0x11);
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = WARN;
    v.error_fg_color = DANGER;
    v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.noninteractive.bg_fill = CARD;

    v.widgets.inactive.bg_fill = Color32::from_rgb(0x1B, 0x22, 0x2E);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x1B, 0x22, 0x2E);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);

    v.widgets.hovered.bg_fill = Color32::from_rgb(0x24, 0x2E, 0x3D);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x24, 0x2E, 0x3D);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.6));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);

    v.widgets.active.bg_fill = ACCENT.gamma_multiply(0.85);
    v.widgets.active.weak_bg_fill = ACCENT.gamma_multiply(0.7);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

    v.widgets.open.bg_fill = Color32::from_rgb(0x1B, 0x22, 0x2E);
    v.widgets.open.weak_bg_fill = Color32::from_rgb(0x1B, 0x22, 0x2E);

    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v
}

/// Цвет акцента для карточки репозитория — детерминированный по slug.
pub fn accent_for(slug: &str) -> Color32 {
    let palette = [
        Color32::from_rgb(0x4C, 0x8D, 0xFF),
        Color32::from_rgb(0x34, 0xD3, 0x99),
        Color32::from_rgb(0xC0, 0x84, 0xFC),
        Color32::from_rgb(0xFB, 0xBF, 0x24),
        Color32::from_rgb(0xF8, 0x71, 0x71),
        Color32::from_rgb(0x2D, 0xD4, 0xBF),
        Color32::from_rgb(0xF9, 0x73, 0x16),
        Color32::from_rgb(0x60, 0xA5, 0xFA),
    ];
    let mut hash: u32 = 2_166_136_261;
    for b in slug.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    palette[(hash % palette.len() as u32) as usize]
}

/// Скругление карточки.
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(CornerRadius::same(14))
        .stroke(Stroke::new(1.0_f32, BORDER))
        .inner_margin(egui::Margin::same(14))
}
