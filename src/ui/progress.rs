//! Полоса прогресса с процентом по центру и подробный блок загрузки.
//!
//! Свой виджет вместо `egui::ProgressBar`: в стандартном процент рисуется
//! сбоку, а нам он нужен посередине полосы — как в легенде загрузок.

use super::theme;
use crate::lang::{t, tf};
use crate::model::{human_duration, human_size, human_speed, TaskProgress, TaskState};
use egui::{Color32, FontFamily, FontId, RichText};

/// Цвет полосы по состоянию задачи.
pub fn fill_color(state: TaskState) -> Color32 {
    match state {
        TaskState::Paused => theme::WARN,
        TaskState::Failed => theme::DANGER,
        TaskState::Done => theme::OK,
        TaskState::Cancelled => theme::DIM,
        _ => theme::ACCENT,
    }
}

/// Подпись состояния задачи.
pub fn state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Queued => t("downloads.state_queued"),
        TaskState::Running => t("downloads.state_running"),
        TaskState::Paused => t("downloads.state_paused"),
        TaskState::Done => t("downloads.state_done"),
        TaskState::Failed => t("downloads.state_failed"),
        TaskState::Cancelled => t("downloads.state_cancelled"),
    }
}

/// Полоса прогресса с процентом по центру.
///
/// Ширина берётся у панели: бесконечная ширина ломает раскладку
/// (available_width становится бесконечным).
pub fn bar(ui: &mut egui::Ui, frac: f32, color: Color32, height: f32) -> egui::Response {
    let width = ui.available_width().max(40.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let frac = frac.clamp(0.0, 1.0);
    let painter = ui.painter();
    let radius = egui::CornerRadius::same((height / 2.0).round().clamp(2.0, 12.0) as u8);

    // Дорожка.
    painter.rect_filled(rect, radius, theme::BG);

    // Заливка.
    let fill_w = rect.width() * frac;
    if fill_w > 1.0 {
        let fill_rect = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + fill_w, rect.bottom()),
        );
        painter.rect_filled(fill_rect, radius, color);
    }

    // Процент по центру полосы.
    let text = format!("{}%", (frac * 100.0).round() as u32);
    let font = FontId::new((height - 4.0).clamp(8.0, 13.0), FontFamily::Proportional);
    // Тёмная обводка: без неё процент не читается на светлой заливке.
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        painter.text(
            rect.center() + egui::vec2(dx, dy),
            egui::Align2::CENTER_CENTER,
            text.clone(),
            font.clone(),
            Color32::from_black_alpha(190),
        );
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        Color32::WHITE,
    );
    response
}

/// Ширина строки в пикселях.
pub fn text_w(ui: &egui::Ui, font: &egui::FontId, text: &str) -> f32 {
    ui.fonts(|f| text.chars().map(|c| f.glyph_width(font, c)).sum::<f32>())
}

/// Обрезать строку по ширине, добавив многоточие.
pub fn elide(ui: &egui::Ui, text: &str, font: &egui::FontId, max_w: f32) -> String {
    if max_w <= 0.0 {
        return String::new();
    }
    if text_w(ui, font, text) <= max_w {
        return text.to_string();
    }
    let mut out = String::new();
    let mut w = 0.0_f32;
    for c in text.chars() {
        let cw = ui.fonts(|f| f.glyph_width(font, c));
        if w + cw > max_w - 8.0 {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// Галочка, нарисованная линиями: символа ✓ в стандартном шрифте нет.
pub fn tick(painter: &egui::Painter, center: egui::Pos2, size: f32, color: egui::Color32) {
    let p1 = center + egui::vec2(-size * 0.45, size * 0.02);
    let p2 = center + egui::vec2(-size * 0.12, size * 0.34);
    let p3 = center + egui::vec2(size * 0.45, -size * 0.34);
    let mut path = egui::epaint::PathShape::line(
        vec![p1, p2, p3],
        egui::Stroke::new(1.7_f32, color),
    );
    path.stroke.kind = egui::StrokeKind::Middle;
    painter.add(path);
}

/// Кружок: ожидание или текущая проверка.
pub fn circle(painter: &egui::Painter, center: egui::Pos2, size: f32, color: egui::Color32) {
    painter.circle_stroke(center, size * 0.4, egui::Stroke::new(1.4_f32, color));
}

/// Подробный блок загрузки: номер в очереди, имя файла, скорость, полоса
/// с процентом по центру, прошедшее время, оценка остатка и объём.
pub fn detailed(ui: &mut egui::Ui, task: &TaskProgress) {
    let color = fill_color(task.state);
    let label = state_label(task.state);

    ui.horizontal(|ui| {
        // Номер файла в очереди загрузки.
        ui.label(
            RichText::new(format!("#{}", task.queue.max(1)))
                .size(11.0)
                .monospace()
                .color(theme::DIM),
        );
        // Имя файла.
        ui.label(RichText::new(&task.asset).size(11.5).color(theme::TEXT));
        ui.label(RichText::new(label).size(10.5).color(color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(human_speed(task.speed))
                    .size(10.5)
                    .color(if task.state == TaskState::Running {
                        theme::OK
                    } else {
                        theme::DIM
                    }),
            );
        });
    });

    bar(ui, task.fraction(), color, 16.0);

    // Метаданные: прошло, осталось, объём, место в очереди.
    ui.horizontal_wrapped(|ui| {
        let meta = |ui: &mut egui::Ui, text: String| {
            ui.label(RichText::new(text).size(10.5).color(theme::DIM));
        };

        let started = task.started_at > 0;
        let elapsed = if started {
            human_duration(task.elapsed_secs())
        } else {
            "—".to_string()
        };
        meta(
            ui,
            tf("dl.elapsed", &[("{t}", &elapsed)]),
        );

        if let Some(left) = task.remaining_secs() {
            meta(
                ui,
                tf("dl.remaining", &[("{t}", &format!("~{}", human_duration(left)))]),
            );
        } else {
            meta(ui, tf("dl.remaining", &[("{t}", "—")]));
        }

        if task.total > 0 {
            meta(
                ui,
                tf(
                    "dl.downloaded",
                    &[
                        ("{a}", &human_size(task.downloaded)),
                        ("{b}", &human_size(task.total)),
                    ],
                ),
            );
        } else {
            meta(ui, tf("dl.size", &[("{a}", &human_size(task.total))]));
        }

        if task.queue_total > 0 {
            meta(
                ui,
                tf(
                    "dl.queue",
                    &[
                        ("{n}", &task.queue.max(1).to_string()),
                        ("{m}", &task.queue_total.to_string()),
                    ],
                ),
            );
        }

        if let Some(err) = &task.error {
            ui.label(RichText::new(err).size(10.5).color(theme::DANGER));
        }
    });
}
