//! Вкладка «Репозитории»: группировка по категориям программ, режимы отображения, карточки.

use super::{hero, progress, releases, theme, CraftApp};
use crate::config::{category_key, category_order, RepoConfig, ViewMode};
use crate::lang::{category as category_title, t, tf};
use crate::model::*;
use crate::storage;
use egui::{self, Align, Color32, Context, Layout, RichText, TextureHandle};

/// Иконка карточки: своя иконка репозитория, а если её нет — иконка приложения.
pub(super) fn card_icon(app: &mut CraftApp, rc: &RepoConfig, ctx: &Context) -> Option<TextureHandle> {
    let own = rc
        .icon
        .as_ref()
        .and_then(|p| app.icon_texture(ctx, &rc.slug, p));
    match own {
        Some(tex) => Some(tex),
        None => app.app_icon_texture(ctx),
    }
}

/// Основное содержимое вкладки.
pub fn show(ui: &mut egui::Ui, app: &mut CraftApp, ctx: &Context) {
    let cfg = app.state.cfg();
    let selected = cfg.repos.iter().filter(|r| r.enabled).count();

    // Шапка.
    ui.horizontal(|ui| {
        ui.label(RichText::new(t("repo.title")).size(22.0).strong());
        ui.label(
            RichText::new(tf(
                "repo.selected_count",
                &[("{n}", &selected.to_string()), ("{m}", &cfg.repos.len().to_string())],
            ))
            .size(12.0)
            .color(theme::DIM),
        );

        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(t("repo.export")).clicked() {
                export_list(app);
            }
            if ui.button(t("repo.import")).clicked() {
                import_list(app);
            }
        });
    });

    ui.add_space(6.0);

    // Добавление своего репозитория.
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.new_repo)
                .hint_text(t("repo.slug_hint"))
                .desired_width(280.0),
        );
        if ui.button(t("repo.add")).clicked() {
            add_repo(app);
        }
        if ui.small_button(t("repo.select_all")).clicked() {
            let mut c = app.state.cfg();
            for r in c.repos.iter_mut() {
                r.enabled = true;
            }
            app.state.save_cfg(c);
        }
        if ui.small_button(t("repo.select_none")).clicked() {
            let mut c = app.state.cfg();
            for r in c.repos.iter_mut() {
                r.enabled = false;
            }
            app.state.save_cfg(c);
        }
    });

    ui.add_space(6.0);

    // Режим отображения.
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("settings.view_mode"))
                .size(12.0)
                .color(theme::DIM),
        );
        for mode in [ViewMode::Grid, ViewMode::List, ViewMode::Light] {
            let selected_mode = cfg.view_mode == mode;
            let btn = egui::Button::new(mode.label())
                .selected(selected_mode)
                .min_size(egui::vec2(0.0, 26.0));
            if ui.add(btn).clicked() {
                let mut c = app.state.cfg();
                c.view_mode = mode;
                app.state.save_cfg(c);
            }
        }
        ui.add_space(8.0);
        if ui
            .small_button(t("common.refresh"))
            .on_hover_text(t("repo.icon_hint"))
            .clicked()
        {
            let st = app.state.clone();
            let handle = st.handle.clone();
            handle.spawn(async move {
                st.refresh_all(true).await;
                st.refresh_icons().await;
            });
        }
    });

    // Общая полоса прогресса.
    let (speed, total, done) = app.progress_summary();
    if total > 0 && done < total {
        ui.add_space(6.0);
        egui::Frame::new()
            .fill(theme::CARD)
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}: {} / {}",
                            t("status.total"),
                            human_size(done),
                            human_size(total)
                        ))
                        .size(12.0),
                    );
                    ui.label(RichText::new(human_speed(speed)).size(12.0).color(theme::ACCENT));
                });
                // Ширину берём у панели: бесконечная ширина ломает
                // раскладку сетки (available_width становится inf).
                progress::bar(
                    ui,
                    (done as f64 / total as f64).clamp(0.0, 1.0) as f32,
                    theme::ACCENT,
                    18.0,
                );
            });
    }

    ui.add_space(10.0);

    // Группировка по категориям программы.
    let groups = group_by_category(&cfg.repos);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            match cfg.view_mode {
                ViewMode::List => {
                    for (category, repos) in &groups {
                        group_header(ui, category, repos.len());
                        for rc in repos {
                            repo_card(ui, app, rc, ctx);
                            ui.add_space(8.0);
                        }
                        ui.add_space(10.0);
                    }
                }
                ViewMode::Grid => {
                    hero::show_groups(ui, app, &groups, ctx);
                }
                ViewMode::Light => {
                    for (category, repos) in &groups {
                        group_header(ui, category, repos.len());
                        for rc in repos {
                            light_row(ui, app, rc);
                            ui.add_space(4.0);
                        }
                        ui.add_space(10.0);
                    }
                }
            }
            ui.add_space(20.0);
        });
}

/// Разложить репозитории по категориям программы.
///
/// Ключ группы — категория из настроек; репозитории без категории попадают
/// в «Другое». Порядок групп — как в каталоге: от своих программ к прочим.
fn group_by_category(repos: &[RepoConfig]) -> Vec<(String, Vec<RepoConfig>)> {
    let mut groups: Vec<(String, Vec<RepoConfig>)> = Vec::new();
    for rc in repos {
        let key = category_key(&rc.category);
        match groups.iter_mut().find(|(k, _)| k == key) {
            Some((_, list)) => list.push(rc.clone()),
            None => groups.push((key.to_string(), vec![rc.clone()])),
        }
    }
    groups.sort_by_key(|(k, _)| category_order(k));
    groups
}

/// Заголовок группы репозиториев: название категории и её размер.
fn group_header(ui: &mut egui::Ui, category: &str, count: usize) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(category_title(category))
                .size(13.0)
                .strong()
                .color(theme::ACCENT),
        );
        ui.label(
            RichText::new(format!("{}", count))
                .size(11.0)
                .color(theme::DIM),
        );
    });
    ui.add_space(4.0);
}

/// Карточка репозитория: иконка слева, описание справа.
fn repo_card(ui: &mut egui::Ui, app: &mut CraftApp, rc: &RepoConfig, ctx: &Context) {
    let rt = app.state.shared.repo_runtime(&rc.slug);
    let accent = theme::accent_for(&rc.slug);
    let name = rc.short_name();

    let mut do_download = false;
    let mut open_popup = false;
    let mut toggle_pin = false;
    let mut open_folder = false;
    let mut delete_tag: Option<String> = None;

    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());

        ui.horizontal(|ui| {
            // Иконка репозитория из локального кэша, иначе — иконка приложения.
            let tex = card_icon(app, rc, ctx);
            match tex {
                Some(tex) => {
                    ui.image(egui::load::SizedTexture::new(
                        tex.id(),
                        egui::vec2(46.0, 46.0),
                    ));
                }
                None => {
                    egui::Frame::new()
                        .fill(accent.gamma_multiply(0.22))
                        .corner_radius(egui::CornerRadius::same(10))
                        .inner_margin(egui::Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(initials(&name))
                                    .size(16.0)
                                    .strong()
                                    .color(accent),
                            );
                        });
                }
            }

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&name).size(16.0).strong());
                    if !rc.enabled {
                        ui.label(
                            RichText::new(t("common.disabled"))
                                .size(11.0)
                                .color(theme::DIM),
                        );
                    }
                    if !rc.pinned.is_empty() {
                        ui.label(
                            RichText::new(tf("repo.pinned_n", &[("{n}", &rc.pinned.len().to_string())]))
                                .size(11.0)
                                .color(theme::WARN),
                        );
                    }
                });
                ui.label(
                    RichText::new(&rc.slug).size(11.0).color(theme::DIM),
                );
                let desc = if rc.description.trim().is_empty() {
                    t("repo.desc_missing").to_string()
                } else {
                    rc.description.clone()
                };
                ui.label(
                    RichText::new(desc)
                        .size(12.0)
                        .color(if rc.description.trim().is_empty() {
                            theme::DIM
                        } else {
                            theme::TEXT
                        })
                        .weak(),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Open").on_hover_text(t("common.open")).clicked() {
                    open_folder = true;
                }
                let pinned_current = rt
                    .as_ref()
                    .and_then(|r| {
                        r.latest_stable
                            .as_ref()
                            .or(r.latest_edge.as_ref())
                            .map(|rel| rel.tag.clone())
                    })
                    .map(|tag| rc.pinned.contains(&tag))
                    .unwrap_or(false);
                let pin_text = if pinned_current {
                    t("common.unpin")
                } else {
                    t("common.pin")
                };
                if ui.button(pin_text).on_hover_text(tf("repo.pinned_n", &[("{n}", "1")])).clicked() {
                    toggle_pin = true;
                }
                if ui.button(t("common.releases")).clicked() {
                    open_popup = true;
                }
                if ui.button(t("common.download")).clicked() {
                    do_download = true;
                }
            });
        });

        // Каналы: стабильный и нестабильный.
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if let Some(rel) = rt.as_ref().and_then(|r| r.latest_stable.as_ref()) {
                channel_chip(ui, t("popup.stable"), &rel.tag, theme::OK);
            }
            if let Some(rel) = rt.as_ref().and_then(|r| r.latest_edge.as_ref()) {
                channel_chip(ui, t("popup.edge"), &rel.tag, theme::EDGE);
            }
            if rt
                .as_ref()
                .map(|r| r.latest_stable.is_none() && r.latest_edge.is_none())
                .unwrap_or(true)
            {
                ui.label(
                    RichText::new(t("repo.no_releases"))
                        .size(11.0)
                        .color(theme::DIM),
                );
            }
            if let Some(rt) = &rt {
                if let Some(check) = &rt.last_check {
                    ui.label(
                        RichText::new(format!("{}: {check}", t("repo.checked")))
                            .size(11.0)
                            .color(theme::DIM),
                    );
                }
            }
        });

        // Что уже скачано.
        let installed = rt.as_ref().map(|r| r.installed.clone()).unwrap_or_default();
        if !installed.is_empty() {
            ui.add_space(6.0);
            egui::Frame::new()
                .fill(theme::BG)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(t("repo.on_disk")).size(11.5).color(theme::DIM),
                    );
                    for v in &installed {
                        ui.horizontal(|ui| {
                            let is_pinned = rc.pinned.contains(&v.tag);
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}  ·  {} {}",
                                    v.tag,
                                    human_size(v.size),
                                    v.files.len(),
                                    t("common.files")
                                ))
                                .size(11.5)
                                .color(if is_pinned { theme::WARN } else { theme::TEXT }),
                            );
                            if is_pinned {
                                ui.label(
                                    RichText::new(t("common.pin")).size(10.0).color(theme::WARN),
                                );
                            }
                            if ui
                                .add_sized([26.0, 18.0], egui::Button::new(t("common.delete")).small())
                                .on_hover_text(t("common.delete"))
                                .clicked()
                            {
                                delete_tag = Some(v.tag.clone());
                            }
                        });
                    }
                });
        }

        // Активные загрузки — подробно: номер в очереди, имя, скорость,
        // прошедшее время и оценка остатка.
        let tasks = app.tasks_for(&rc.slug);
        let active: Vec<TaskProgress> = tasks
            .into_iter()
            .filter(|t| !t.state.is_finished())
            .collect();
        if !active.is_empty() {
            ui.add_space(8.0);
            for task in &active {
                progress::detailed(ui, task);
                ui.add_space(4.0);
            }
        }

        // Выбор файла: варианты из файлов релиза, по умолчанию — под текущую ОС.
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(t("repo.filter"))
                    .size(11.5)
                    .color(theme::DIM),
            );
            if let Some(kind) = releases::asset_combo(ui, app, &rc.slug, rc.filter) {
                let mut cfg = app.state.cfg();
                if let Some(r) = cfg.repo_mut(&rc.slug) {
                    r.filter = kind;
                }
                app.state.save_cfg(cfg);
            }
        });

        // Ошибка.
        if let Some(err) = rt.as_ref().and_then(|r| r.last_error.clone()) {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("⚠ {err}")).size(11.5).color(theme::DANGER));
        }
    });

    // Подсветка карточки при наведении.
    let card_rect = ui.min_rect();
    if ui
        .interact(card_rect, ui.id().with("repo_card"), egui::Sense::hover())
        .hovered()
    {
        ui.painter().rect_stroke(
            card_rect,
            egui::CornerRadius::same(14),
            egui::Stroke::new(1.5_f32, theme::ACCENT),
            egui::StrokeKind::Middle,
        );
    }

    // Действия после отрисовки карточки.
    if do_download {
        let st = app.state.clone();
        let handle = st.handle.clone();
        let slug = rc.slug.clone();
        handle.spawn(async move {
            st.download_fresh(&slug, crate::manager::DownloadRequest::Targets)
                .await
        });
        app.notify(
            tf("repo.download_started", &[("{s}", &rc.slug)]),
            LogLevel::Info,
        );
    }
    if open_popup {
        app.open_releases(&rc.slug);
    }
    if toggle_pin {
        let mut cfg = app.state.cfg();
        if let Some(r) = cfg.repo_mut(&rc.slug) {
            if let Some(tag) = rt
                .as_ref()
                .and_then(|r| r.latest_stable.as_ref().or(r.latest_edge.as_ref()))
                .map(|rel| rel.tag.clone())
            {
                if let Some(pos) = r.pinned.iter().position(|p| p == &tag) {
                    r.pinned.remove(pos);
                } else {
                    r.pinned.push(tag);
                }
            }
        }
        app.state.save_cfg(cfg);
        app.notify(t("notify.pin_changed"), LogLevel::Ok);
    }
    if open_folder {
        let cfg = app.state.cfg();
        let dir = storage::repo_dir(&cfg.root_dir, &rc.slug);
        let _ = std::fs::create_dir_all(&dir);
        let _ = open::that(&dir);
    }
    if let Some(tag) = delete_tag {
        let cfg = app.state.cfg();
        let removed = storage::remove_version(&cfg.root_dir, &rc.slug, &tag, &rc.pinned);
        if removed {
            let mut rt = app.state.shared.repo_runtime(&rc.slug).unwrap_or_default();
            rt.slug = rc.slug.clone();
            rt.installed = storage::scan_installed(&cfg.root_dir, &rc.slug);
            app.state.shared.set_repo_runtime(rt);
            app.notify(
                tf("repo.version_deleted", &[("{t}", &tag)]),
                LogLevel::Ok,
            );
        } else {
            app.notify(
                tf("repo.version_kept", &[("{t}", &tag)]),
                LogLevel::Warn,
            );
        }
    }
}

/// Компактная строка без картинок — с подробным блоком прогресса.
fn light_row(ui: &mut egui::Ui, app: &mut CraftApp, rc: &RepoConfig) {
    let rt = app.state.shared.repo_runtime(&rc.slug);
    let mut do_download = false;
    let mut open_popup = false;

    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Название и адрес репозитория.
                ui.vertical(|ui| {
                    ui.set_width(160.0);
                    ui.horizontal(|ui| {
                        let mark = if rc.enabled { "[x]" } else { "[ ]" };
                        if ui
                            .add_sized([18.0, 18.0], egui::Button::new(mark).small())
                            .on_hover_text(t("common.enabled"))
                            .clicked()
                        {
                            let mut cfg = app.state.cfg();
                            if let Some(r) = cfg.repo_mut(&rc.slug) {
                                r.enabled = !r.enabled;
                            }
                            app.state.save_cfg(cfg);
                        }
                        ui.label(RichText::new(rc.short_name()).strong().size(13.0));
                    });
                    ui.label(RichText::new(&rc.slug).size(10.5).color(theme::DIM));
                });

                // Версии, проверка и описание — фиксированной ширины,
                // иначе длинное описание распирает всю строку.
                ui.vertical(|ui| {
                    ui.set_width(330.0);
                    let mut meta: Vec<String> = Vec::new();
                    if let Some(rel) = rt.as_ref().and_then(|r| r.latest_stable.as_ref()) {
                        meta.push(format!("{} · {}", rel.tag, t("popup.stable")));
                    }
                    if let Some(rel) = rt.as_ref().and_then(|r| r.latest_edge.as_ref()) {
                        meta.push(format!("{} · {}", rel.tag, t("popup.edge")));
                    }
                    if let Some(check) = rt.as_ref().and_then(|r| r.last_check.clone()) {
                        meta.push(format!("{}: {check}", t("repo.checked")));
                    }
                    if !meta.is_empty() {
                        ui.add(
                            egui::Label::new(RichText::new(meta.join(" · ")).size(11.0).color(theme::DIM))
                                .wrap(),
                        );
                    }
                    if !rc.description.trim().is_empty() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&rc.description)
                                    .size(11.0)
                                    .color(theme::TEXT)
                                    .weak(),
                            )
                            .wrap(),
                        );
                    }
                });

                // Прогресс — отдельной колонкой, максимально подробно.
                let tasks = app.tasks_for(&rc.slug);
                let active: Vec<TaskProgress> = tasks
                    .into_iter()
                    .filter(|t| !t.state.is_finished())
                    .collect();
                ui.vertical(|ui| {
                    ui.set_min_width(300.0);
                    for task in &active {
                        progress::detailed(ui, task);
                        ui.add_space(4.0);
                    }
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(t("common.download")).clicked() {
                        do_download = true;
                    }
                    if ui.button(t("common.releases")).clicked() {
                        open_popup = true;
                    }
                });
            });
        });

    if do_download {
        let st = app.state.clone();
        let handle = st.handle.clone();
        let slug = rc.slug.clone();
        handle.spawn(async move {
            st.download_fresh(&slug, crate::manager::DownloadRequest::Targets)
                .await
        });
        app.notify(
            tf("repo.download_started", &[("{s}", &rc.slug)]),
            LogLevel::Info,
        );
    }
    if open_popup {
        app.open_releases(&rc.slug);
    }
}

/// Плашка канала.
fn channel_chip(ui: &mut egui::Ui, title: &str, tag: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.15))
        .corner_radius(egui::CornerRadius::same(12))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.45)))
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(10.0).color(color));
                ui.label(RichText::new(tag).size(11.5).strong());
            });
        });
}

fn initials(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

/// Добавить свой репозиторий.
fn add_repo(app: &mut CraftApp) {
    let slug = app.new_repo.trim().to_string();
    if slug.is_empty() || !slug.contains('/') {
        app.notify(t("repo.add_err"), LogLevel::Warn);
        return;
    }
    let mut cfg = app.state.cfg();
    if cfg.repo(&slug).is_some() {
        app.notify(
            tf("repo.add_err", &[("{e}", &slug)]),
            LogLevel::Warn,
        );
        return;
    }
    let mut rc = crate::config::RepoConfig::new(&slug);
    rc.enabled = true;
    cfg.repos.push(rc);
    app.state.save_cfg(cfg);
    app.new_repo.clear();
    app.notify(
        tf("repo.added", &[("{s}", &slug)]),
        LogLevel::Ok,
    );
}

/// Экспорт списка репозиториев в JSON.
fn export_list(app: &mut CraftApp) {
    let cfg = app.state.cfg();
    let file = cfg.export_list();
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("craftpull-repos.json")
        .save_file()
    else {
        return;
    };
    match serde_json::to_string_pretty(&file) {
        Ok(text) => match std::fs::write(&path, text) {
            Ok(()) => app.notify(
                tf("repo.export_ok", &[("{p}", &path.to_string_lossy())]),
                LogLevel::Ok,
            ),
            Err(e) => app.notify(
                tf("notify.err", &[("{e}", &e.to_string())]),
                LogLevel::Error,
            ),
        },
        Err(e) => app.notify(
            tf("notify.err", &[("{e}", &e.to_string())]),
            LogLevel::Error,
        ),
    }
}

/// Импорт списка репозиториев из JSON.
fn import_list(app: &mut CraftApp) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name("craftpull-repos.json")
        .pick_file()
    else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        app.notify(
            tf("repo.import_err", &[("{e}", &path.to_string_lossy())]),
            LogLevel::Error,
        );
        return;
    };
    match serde_json::from_str::<crate::config::RepoListFile>(&text) {
        Ok(file) => {
            let mut cfg = app.state.cfg();
            let added = cfg.import_list(&file);
            app.state.save_cfg(cfg);
            app.notify(
                tf("repo.import_ok", &[("{n}", &added.to_string())]),
                LogLevel::Ok,
            );
        }
        Err(e) => app.notify(
            tf("repo.import_err", &[("{e}", &e.to_string())]),
            LogLevel::Error,
        ),
    }
}
