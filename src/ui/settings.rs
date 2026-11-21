//! Вкладка «Настройки».

use super::{theme, CraftApp};
use crate::config::ViewMode;
use crate::lang::{lang_name, t, tf};
use crate::model::LogLevel;
use egui::{self, RichText};

/// Рамка-раздел настроек.
fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(egui::CornerRadius::same(12))
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.label(RichText::new(title).size(15.0).strong());
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(8.0);
            add(ui);
        });
    ui.add_space(10.0);
}

pub fn show(ui: &mut egui::Ui, app: &mut CraftApp) {
    let mut cfg = app.state.cfg();
    let mut dirty = false;
    let mut lang_choice = cfg.lang;
    let mut root_text = cfg.root_dir.to_string_lossy().to_string();
    let mut new_slug = String::new();
    let mut remove_slug: Option<String> = None;
    let mut import_done = false;
    let mut export_done = false;

    ui.label(RichText::new(t("settings.title")).size(22.0).strong());
    ui.add_space(10.0);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            // --- Язык и вид ---
            section(ui, t("settings.language"), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("settings.language")).size(12.5));
                    egui::ComboBox::from_id_salt("lang")
                        .selected_text(lang_name(lang_choice))
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for code in [crate::lang::EN, crate::lang::RU] {
                                ui.selectable_value(&mut lang_choice, code, lang_name(code));
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("settings.view_mode")).size(12.5));
                    for mode in [ViewMode::Grid, ViewMode::List, ViewMode::Light] {
                        let btn = egui::Button::new(mode.label())
                            .selected(cfg.view_mode == mode)
                            .min_size(egui::vec2(0.0, 26.0));
                        if ui.add(btn).clicked() {
                            cfg.view_mode = mode;
                            dirty = true;
                        }
                    }
                });
            });

            // --- Папка загрузок ---
            section(ui, t("settings.folder"), |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut root_text)
                            .desired_width(f32::INFINITY)
                            .hint_text(t("settings.folder_hint")),
                    );
                    if ui.button(t("common.pick")).clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            root_text = dir.to_string_lossy().to_string();
                        }
                    }
                    if ui.button(t("common.open")).clicked() {
                        let _ = open::that(&cfg.root_dir);
                    }
                });
            });

            // --- Сеть ---
            section(ui, t("settings.network"), |ui| {
                let mut unlimited = cfg.speed_limit_kbps == 0;
                if ui
                    .checkbox(&mut unlimited, t("settings.no_limit"))
                    .changed()
                {
                    cfg.speed_limit_kbps = if unlimited { 0 } else { 5 * 1024 };
                    dirty = true;
                }
                if !unlimited {
                    let mut mbps = cfg.speed_limit_kbps as f64 / 1024.0;
                    ui.horizontal(|ui| {
                        ui.label(t("settings.limit"));
                        if ui
                            .add_sized(
                                [280.0, 24.0],
                                egui::Slider::new(&mut mbps, 0.1..=200.0).suffix(" MB/s"),
                            )
                            .changed()
                        {
                            cfg.speed_limit_kbps = (mbps * 1024.0).round() as u64;
                            dirty = true;
                        }
                        ui.label(
                            RichText::new(format!("{} KB/s", cfg.speed_limit_kbps))
                                .size(11.5)
                                .color(theme::DIM),
                        );
                    });
                }

                // Лимит параллельных загрузок — только целые числа.
                let mut parallel = cfg.max_parallel.clamp(1, crate::config::MAX_PARALLEL);
                ui.horizontal(|ui| {
                    ui.label(t("settings.parallel"));
                    if ui
                        .add_sized(
                            [240.0, 24.0],
                            egui::Slider::new(&mut parallel, 1..=crate::config::MAX_PARALLEL),
                        )
                        .changed()
                    {
                        cfg.max_parallel = parallel;
                        dirty = true;
                    }
                    ui.label(RichText::new(parallel.to_string()).size(12.5).strong());
                    ui.label(
                        RichText::new(crate::lang::tf(
                            "settings.parallel_max",
                            &[("{n}", &crate::config::MAX_PARALLEL.to_string())],
                        ))
                        .size(11.0)
                        .color(theme::DIM),
                    );
                });

                // Параллельные или последовательные загрузки.
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("download.mode")).size(12.5));
                    for mode in [
                        crate::config::DownloadMode::Parallel,
                        crate::config::DownloadMode::Sequential,
                    ] {
                        let btn = egui::Button::new(mode.label())
                            .selected(cfg.download_mode == mode)
                            .min_size(egui::vec2(0.0, 26.0));
                        if ui.add(btn).clicked() {
                            cfg.download_mode = mode;
                            dirty = true;
                        }
                    }
                });
                ui.label(
                    RichText::new(t("download.mode_hint"))
                        .size(11.5)
                        .color(theme::DIM),
                );

                if ui
                    .checkbox(&mut cfg.verify_sha256, t("settings.sha"))
                    .changed()
                {
                    dirty = true;
                }

                ui.horizontal(|ui| {
                    ui.label(t("settings.token"));
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut cfg.github_token)
                                .password(true)
                                .desired_width(300.0)
                                .hint_text(t("settings.token_hint")),
                        )
                        .changed()
                    {
                        dirty = true;
                    }
                });
            });

            // --- Каналы ---
            section(ui, t("settings.channels"), |ui| {
                if ui.checkbox(&mut cfg.stable_channel, t("settings.stable")).changed() {
                    dirty = true;
                }
                if ui.checkbox(&mut cfg.edge_channel, t("settings.edge")).changed() {
                    dirty = true;
                }
                ui.label(
                    RichText::new(t("settings.channels_hint"))
                        .size(11.5)
                        .color(theme::DIM),
                );
            });

            // --- Поведение ---
            section(ui, t("settings.behaviour"), |ui| {
                if ui
                    .checkbox(&mut cfg.delete_old_versions, t("settings.delete_old"))
                    .changed()
                {
                    dirty = true;
                }
                if ui
                    .checkbox(&mut cfg.close_to_tray, t("settings.close_tray"))
                    .changed()
                {
                    dirty = true;
                }
                if ui
                    .checkbox(&mut cfg.start_minimized, t("settings.start_min"))
                    .changed()
                {
                    dirty = true;
                }
                if ui
                    .checkbox(&mut cfg.autostart, t("settings.autostart"))
                    .changed()
                {
                    let exe = std::env::current_exe().unwrap_or_default();
                    match crate::selfupdate::set_autostart(cfg.autostart, &exe) {
                        Ok(()) => dirty = true,
                        Err(e) => {
                            cfg.autostart = false;
                            app.notify(
                                tf("settings.autostart_err", &[("{e}", &e)]),
                                LogLevel::Error,
                            );
                        }
                    }
                }
            });

            // --- Обновление программы ---
            section(ui, t("settings.update_title"), |ui| {
                ui.label(
                    RichText::new(tf(
                        "settings.update_current",
                        &[("{v}", crate::updater::current_version())],
                    ))
                    .size(12.5),
                );
                let upd = crate::updater::snapshot();
                ui.horizontal(|ui| {
                    let check = ui.add(
                        egui::Button::new(t("settings.update_check"))
                            .min_size(egui::vec2(0.0, 28.0)),
                    );
                    if check.clicked() && !upd.checking && !upd.updating {
                        crate::updater::spawn_check(app.state.clone());
                    }
                    if upd.checking {
                        ui.spinner();
                    }
                });

                // Новая версия — кнопка обновления заметно выделяется.
                if upd.available {
                    ui.label(
                        RichText::new(tf("settings.update_available", &[("{tag}", &upd.tag)]))
                            .size(12.5)
                            .color(theme::OK),
                    );
                    ui.horizontal(|ui| {
                        let update = ui.add(
                            egui::Button::new(tf("settings.update_now", &[("{tag}", &upd.tag)]))
                                .fill(theme::ACCENT)
                                .stroke(egui::Stroke::new(
                                    1.0_f32,
                                    theme::ACCENT.gamma_multiply(1.4),
                                ))
                                .min_size(egui::vec2(0.0, 30.0)),
                        );
                        if update.clicked() && !upd.updating {
                            crate::updater::spawn_update(app.state.clone());
                        }
                        if upd.updating {
                            ui.spinner();
                        }
                    });
                }

                if !upd.status.is_empty() {
                    ui.label(RichText::new(&upd.status).size(11.5).color(theme::DIM));
                }
                ui.label(
                    RichText::new(t("settings.update_hint"))
                        .size(11.5)
                        .color(theme::DIM),
                );
            });

            // --- Самообновление ---
            section(ui, t("settings.selfupdate"), |ui| {
                ui.horizontal(|ui| {
                    ui.label(t("settings.self_repo"));
                    ui.add(
                        egui::TextEdit::singleline(&mut cfg.self_update_repo)
                            .desired_width(300.0)
                            .hint_text(t("settings.self_repo_hint")),
                    );
                    if ui.button(t("common.save")).clicked() {
                        dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t("settings.self_branch"));
                    ui.add(
                        egui::TextEdit::singleline(&mut cfg.self_update_branch)
                            .desired_width(160.0),
                    );
                    if ui.button(t("settings.self_now")).clicked() {
                        let st = app.state.clone();
                        let handle = st.handle.clone();
                        handle.spawn(async move {
                            match crate::selfupdate::run_self_update(st.clone()).await {
                                Ok(msg) => st.shared.log(LogLevel::Ok, msg),
                                Err(e) => st.shared.log(LogLevel::Error, format!("{e}")),
                            }
                        });
                        app.notify(t("notify.selfupdate_started"), LogLevel::Info);
                    }
                });
                ui.label(
                    RichText::new(t("settings.self_hint"))
                        .size(11.5)
                        .color(theme::DIM),
                );
            });

            // --- Репозитории ---
            section(ui, t("settings.repos"), |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut new_slug)
                            .hint_text(t("repo.slug_hint"))
                            .desired_width(260.0),
                    );
                    if ui.button(t("repo.add")).clicked() {
                        let slug = new_slug.trim().to_string();
                        if !slug.is_empty() && slug.contains('/') && cfg.repo(&slug).is_none() {
                            let mut rc = crate::config::RepoConfig::new(&slug);
                            rc.enabled = true;
                            cfg.repos.push(rc);
                            new_slug.clear();
                            dirty = true;
                        }
                    }
                    if ui.button(t("repo.import")).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("craftpull-repos.json")
                            .pick_file()
                        {
                            if let Ok(text) = std::fs::read_to_string(&path) {
                                match serde_json::from_str::<crate::config::RepoListFile>(&text) {
                                    Ok(file) => {
                                        let added = cfg.import_list(&file);
                                        dirty = true;
                                        import_done = true;
                                        let _ = added;
                                    }
                                    Err(e) => {
                                        app.notify(
                                            tf("repo.import_err", &[("{e}", &e.to_string())]),
                                            LogLevel::Error,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    if ui.button(t("repo.export")).clicked() {
                        let file = cfg.export_list();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("craftpull-repos.json")
                            .save_file()
                        {
                            match serde_json::to_string_pretty(&file) {
                                Ok(text) => match std::fs::write(&path, text) {
                                    Ok(()) => export_done = true,
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
                    }
                });

                ui.separator();

                for i in 0..cfg.repos.len() {
                    ui.push_id(i, |ui| {
                        ui.horizontal(|ui| {
                            let mut enabled = cfg.repos[i].enabled;
                            if ui.checkbox(&mut enabled, "").changed() {
                                cfg.repos[i].enabled = enabled;
                                dirty = true;
                            }
                            ui.label(
                                RichText::new(&cfg.repos[i].slug)
                                    .size(12.5)
                                    .strong(),
                            );
                            let mut filter = cfg.repos[i].filter;
                            let slug = cfg.repos[i].slug.clone();
                            // Варианты — реальные файлы релиза этого репозитория.
                            let options: Vec<(crate::config::AssetKind, String)> = app
                                .state
                                .shared
                                .repo_runtime(&slug)
                                .and_then(|rt| {
                                    rt.releases
                                        .iter()
                                        .find(|rel| !rel.is_edge())
                                        .or_else(|| rt.releases.first())
                                        .map(|rel| {
                                            crate::manager::asset_options(&slug, rel)
                                                .into_iter()
                                                .map(|(k, a)| {
                                                    (
                                                        k,
                                                        format!(
                                                            "{} · {}",
                                                            k.localized_label(),
                                                            crate::model::human_size(a.size)
                                                        ),
                                                    )
                                                })
                                                .collect()
                                        })
                                })
                                .unwrap_or_default();
                            let combo = egui::ComboBox::from_id_salt("filter")
                                .selected_text(filter.localized_label())
                                .width(250.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut filter,
                                        crate::config::AssetKind::Auto,
                                        format!(
                                            "{} · {}",
                                            crate::config::AssetKind::Auto.localized_label(),
                                            t("filter.auto_hint")
                                        ),
                                    );
                                    for (k, label) in &options {
                                        ui.selectable_value(&mut filter, *k, label.clone());
                                    }
                                    ui.separator();
                                    ui.selectable_value(
                                        &mut filter,
                                        crate::config::AssetKind::Source,
                                        crate::config::AssetKind::Source.localized_label(),
                                    );
                                    ui.selectable_value(
                                        &mut filter,
                                        crate::config::AssetKind::All,
                                        crate::config::AssetKind::All.localized_label(),
                                    );
                                });
                            if combo.response.changed() {
                                cfg.repos[i].filter = filter;
                                dirty = true;
                            }
                            let mut clone = cfg.repos[i].clone_source;
                            if ui.checkbox(&mut clone, t("settings.sources")).changed() {
                                cfg.repos[i].clone_source = clone;
                                dirty = true;
                            }
                            if !cfg.repos[i].pinned.is_empty() {
                                ui.label(
                                    RichText::new(tf(
                                        "repo.pinned_n",
                                        &[("{n}", &cfg.repos[i].pinned.len().to_string())],
                                    ))
                                    .size(11.0)
                                    .color(theme::WARN),
                                );
                            }
                            if ui.small_button(t("repo.remove")).clicked() {
                                remove_slug = Some(cfg.repos[i].slug.clone());
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(t("repo.desc_hint"))
                                    .size(11.0)
                                    .color(theme::DIM),
                            );
                            let mut desc = cfg.repos[i].description.clone();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut desc)
                                        .desired_width(f32::INFINITY)
                                        .hint_text(t("repo.desc_hint")),
                                )
                                .changed()
                            {
                                cfg.repos[i].description = desc;
                                dirty = true;
                            }
                        });
                        ui.separator();
                    });
                }
            });

            // --- Установка ---
            section(ui, t("settings.install"), |ui| {
                ui.horizontal(|ui| {
                    if ui.button(t("settings.shortcut")).clicked() {
                        match crate::icon::install_shortcut() {
                            Ok(path) => app.notify(
                                tf("notify.shortcut_ok", &[("{p}", &path.to_string_lossy())]),
                                LogLevel::Ok,
                            ),
                            Err(e) => app.notify(
                                tf("notify.err", &[("{e}", &e)]),
                                LogLevel::Error,
                            ),
                        }
                    }
                    if ui.button(t("settings.open_data")).clicked() {
                        let _ = open::that(crate::config::data_dir());
                    }
                    if ui.button(t("settings.open_log")).clicked() {
                        let _ = open::that(crate::config::data_dir().join("craftpull.log"));
                    }
                });
                ui.label(
                    RichText::new(format!("Data: {}", crate::config::data_dir().display()))
                        .size(11.5)
                        .color(theme::DIM),
                );
            });

            // --- Об авторе ---
            section(ui, t("about.title"), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("app.author_label")).size(12.5));
                    ui.label(RichText::new(t("app.author")).size(13.0).strong());
                });
                ui.label(
                    RichText::new(t("about.note"))
                        .size(11.5)
                        .color(theme::DIM),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("about.email")).size(12.5));
                    ui.hyperlink_to(
                        RichText::new("support@chekak.store").size(12.5).color(theme::ACCENT),
                        "mailto:support@chekak.store",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("about.site")).size(12.5));
                    ui.hyperlink_to(
                        RichText::new("chekak.store").size(12.5).color(theme::ACCENT),
                        "https://chekak.store",
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("about.telegram")).size(12.5));
                    ui.hyperlink_to(
                        RichText::new("t.me/chekakstore").size(12.5).color(theme::ACCENT),
                        "https://t.me/chekakstore",
                    );
                });
            });
        });

    // Папка загрузок.
    if root_text != cfg.root_dir.to_string_lossy().as_ref() {
        cfg.root_dir = std::path::PathBuf::from(&root_text);
        dirty = true;
    }

    if let Some(slug) = remove_slug {
        cfg.repos.retain(|r| r.slug != slug);
        dirty = true;
        app.notify(tf("repo.removed", &[("{s}", &slug)]), LogLevel::Ok);
    }

    if dirty {
        app.state.save_cfg(cfg.clone());
    }
    if lang_choice != cfg.lang {
        app.set_lang(lang_choice);
    }
    if import_done {
        app.notify(t("repo.import_ok").replace("{n}", "—"), LogLevel::Ok);
    }
    if export_done {
        app.notify(t("repo.export_ok").replace("{p}", "—"), LogLevel::Ok);
    }
}
