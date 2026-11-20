//! Вкладка «Журнал»: список проверок, легенда загрузок и лог событий.

use super::{progress, theme, CraftApp};
use crate::lang::t;
use crate::manager::CheckStatus;
use crate::model::{LogLevel, TaskState};
use egui::RichText;

pub fn show(ui: &mut egui::Ui, app: &mut CraftApp) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("log.title")).size(22.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(t("log.clear")).clicked() {
                if let Ok(mut logs) = app.state.shared.logs.lock() {
                    logs.clear();
                }
            }
            if ui.button(t("log.open")).clicked() {
                let _ = open::that(crate::config::data_dir().join("craftpull.log"));
            }
        });
    });
    ui.add_space(8.0);

    checks(ui, app);
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    downloads(ui, app);
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    let entries: Vec<_> = app
        .state
        .shared
        .logs
        .lock()
        .map(|l| l.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink(false)
        .show(ui, |ui| {
            if entries.is_empty() {
                ui.label(RichText::new(t("log.empty")).size(12.5).color(theme::DIM));
            }
            for e in entries {
                let color = match e.level {
                    LogLevel::Info => theme::DIM,
                    LogLevel::Ok => theme::OK,
                    LogLevel::Warn => theme::WARN,
                    LogLevel::Error => theme::DANGER,
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(e.time)
                            .size(11.0)
                            .color(theme::DIM)
                            .monospace(),
                    );
                    ui.label(RichText::new(&e.text).size(12.0).color(color));
                });
            }
        });
}

/// Список проверок: что уже проверено, что проверяется, что в очереди.
fn checks(ui: &mut egui::Ui, app: &mut CraftApp) {
    let state = match app.state.shared.checks.lock() {
        Ok(c) => c.clone(),
        Err(_) => return,
    };
    if state.items.is_empty() {
        return;
    }

    ui.label(RichText::new(t("check.title")).size(16.0).strong());
    ui.add_space(4.0);

    // Общая полоса проверки.
    progress::bar(ui, state.fraction(), theme::ACCENT, 16.0);

    let done = state.done();
    let total = state.total();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(crate::lang::tf(
                "check.progress",
                &[("{n}", &done.to_string()), ("{m}", &total.to_string())],
            ))
            .size(11.5)
            .color(theme::DIM),
        );
        if state.running {
            ui.label(
                RichText::new(t("check.running")).size(11.5).color(theme::ACCENT),
            );
        }
    });
    ui.add_space(6.0);

    // Список репозиториев.
    let mut shown = 0usize;
    for item in &state.items {
        // Дальше — только те, что ждут: иначе список разрастается на весь экран.
        if item.status == CheckStatus::Waiting && shown >= 3 {
            let rest = state
                .items
                .iter()
                .filter(|i| i.status == CheckStatus::Waiting)
                .count();
            ui.label(
                RichText::new(crate::lang::tf(
                    "check.rest",
                    &[("{n}", &rest.to_string())],
                ))
                .size(11.0)
                .color(theme::DIM),
            );
            break;
        }
        if item.status == CheckStatus::Waiting {
            shown += 1;
        }

        let (mark, color, tail) = match item.status {
            CheckStatus::Done => ("tick", theme::OK, item.detail.clone()),
            CheckStatus::Failed => ("!", theme::DANGER, item.detail.clone()),
            CheckStatus::Running => ("run", theme::ACCENT, t("check.in_progress").to_string()),
            CheckStatus::Skipped => ("wait", theme::DIM, item.detail.clone()),
            CheckStatus::Waiting => ("wait", theme::DIM, t("check.waiting").to_string()),
        };
        ui.horizontal(|ui| {
            // Значок рисуем линиями: символов ✓ и ⟳ в шрифте нет.
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            let center = rect.center();
            match mark {
                "tick" => progress::tick(ui.painter(), center, 11.0, color),
                "run" => progress::circle(ui.painter(), center, 11.0, color),
                _ => progress::circle(ui.painter(), center, 10.0, color),
            }
            if mark == "!" {
                ui.painter().text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "!",
                    egui::FontId::new(12.0, egui::FontFamily::Proportional),
                    color,
                );
            }
            let name = if item.status == CheckStatus::Running {
                crate::lang::tf("check.running_repo", &[("{s}", &item.slug)])
            } else {
                item.slug.clone()
            };
            ui.label(RichText::new(name).size(11.5).color(theme::TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(tail).size(10.5).color(theme::DIM));
            });
        });
    }
}

/// Легенда загрузок: номер в очереди, имя файла, скорость, полоса с
/// процентом по центру, прошедшее время и оценка остатка.
fn downloads(ui: &mut egui::Ui, app: &mut CraftApp) {
    ui.label(RichText::new(t("downloads.title")).size(16.0).strong());
    ui.add_space(4.0);

    let mut tasks = app.tasks();
    if tasks.is_empty() {
        ui.label(RichText::new(t("downloads.empty")).size(12.0).color(theme::DIM));
        return;
    }

    // Сначала активные, потом завершённые; внутри — по репозиторию.
    tasks.sort_by(|a, b| {
        a.state
            .is_finished()
            .cmp(&b.state.is_finished())
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| a.asset.cmp(&b.asset))
    });

    for task in tasks {
        progress::detailed(ui, &task);
        if task.state == TaskState::Running {
            ui.add_space(2.0);
        }
        ui.add_space(6.0);
    }
}
