//! Вкладка «Журнал».

use super::{theme, CraftApp};
use crate::lang::t;
use crate::model::{human_eta, human_size, human_speed, LogLevel, TaskState};
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

/// Список загрузок: что качается, сколько осталось и с какой скоростью.
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
        let (color, label) = match task.state {
            TaskState::Queued => (theme::DIM, t("downloads.state_queued")),
            TaskState::Running => (theme::OK, t("downloads.state_running")),
            TaskState::Paused => (theme::WARN, t("downloads.state_paused")),
            TaskState::Done => (theme::ACCENT, t("downloads.state_done")),
            TaskState::Failed => (theme::DANGER, t("downloads.state_failed")),
            TaskState::Cancelled => (theme::DIM, t("downloads.state_cancelled")),
        };

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} · {}", task.repo, task.asset))
                    .size(11.5)
                    .color(theme::TEXT),
            );
            ui.label(RichText::new(label).size(10.5).color(color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if task.state == TaskState::Running {
                    ui.label(
                        RichText::new(human_speed(task.speed))
                            .size(10.5)
                            .color(theme::OK),
                    );
                }
            });
        });

        let frac = if task.total > 0 {
            (task.downloaded as f64 / task.total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ui.add(
            egui::ProgressBar::new(frac as f32)
                .desired_height(5.0)
                .fill(color),
        );

        let mut tail = format!(
            "{} / {} · {}%",
            human_size(task.downloaded),
            human_size(task.total),
            (frac * 100.0).round() as u32
        );
        if task.state == TaskState::Running && task.speed > 1.0 {
            let left = task.total.saturating_sub(task.downloaded);
            tail.push_str(&format!(" · {}", human_eta(left, task.speed)));
        }
        if let Some(err) = &task.error {
            tail.push_str(&format!(" · {err}"));
        }
        ui.label(RichText::new(tail).size(10.5).color(theme::DIM));
        ui.add_space(6.0);
    }
}
