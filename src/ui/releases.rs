//! Релизы внутри страницы приложения.
//!
//! Отдельного окна больше нет: список релизов открывается панелью справа,
//! файлы отмечаются галочками, а кнопка «Скачать выбранное» всегда висит
//! внизу панели — её не нужно искать после прокрутки.

use super::{progress, theme};
use crate::lang::{t, tf};
use crate::manager::{asset_kind, os_default_kind};
use crate::model::{human_date, human_size, AssetInfo, LogLevel, ReleaseInfo};
use crate::ui::CraftApp;
use egui::{Align, Layout, RichText};

/// Действия, собранные при отрисовке панели.
enum Action {
    ToggleExpand(String),
    ToggleAsset(String),
    SelectRelease(String),
    ClearSelection,
    Download(Vec<(String, Vec<AssetInfo>)>),
    Close,
}

/// Ключ отметки файла: «тег::имя файла».
pub fn asset_key(tag: &str, name: &str) -> String {
    format!("{tag}::{name}")
}

/// Панель релизов выбранного репозитория.
pub fn panel(ui: &mut egui::Ui, app: &mut CraftApp, slug: &str, releases: &[ReleaseInfo]) {
    let mut actions: Vec<Action> = Vec::new();
    let expanded = app.release_expanded.clone();
    let selected: Vec<String> = app.selected_assets.iter().cloned().collect();

    // --- Шапка панели ---
    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(egui::CornerRadius::same(12))
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(slug.rsplit('/').next().unwrap_or(slug))
                        .size(15.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(tf("popup.releases_n", &[("{n}", &releases.len().to_string())]))
                        .size(11.0)
                        .color(theme::DIM),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("×").on_hover_text(t("common.close")).clicked() {
                        actions.push(Action::Close);
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(t("releases.hint"))
                    .size(11.0)
                    .color(theme::DIM),
            );
        });

    ui.add_space(8.0);

    // --- Список релизов ---
    // Место под липкую нижнюю панель: итог + ряд кнопок + рамка.
    let bottom_h = 76.0_f32;
    let scroll_h = (ui.available_height() - bottom_h).max(120.0);
    egui::ScrollArea::vertical()
        .max_height(scroll_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if releases.is_empty() {
                ui.label(
                    RichText::new(t("repo.no_releases"))
                        .size(12.0)
                        .color(theme::DIM),
                );
            }
            for rel in releases {
                let is_expanded = expanded.as_deref() == Some(rel.tag.as_str());
                let pinned = app
                    .state
                    .cfg()
                    .repo(slug)
                    .map(|r| r.pinned.contains(&rel.tag))
                    .unwrap_or(false);

                egui::Frame::new()
                    .fill(theme::CARD)
                    .corner_radius(egui::CornerRadius::same(10))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        if rel.is_edge() {
                            theme::EDGE
                        } else {
                            theme::BORDER
                        },
                    ))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .button(if is_expanded { "v" } else { ">" })
                                .on_hover_text(t("popup.assets"))
                                .clicked()
                            {
                                actions.push(Action::ToggleExpand(rel.tag.clone()));
                            }
                            ui.label(RichText::new(&rel.tag).size(13.0).strong());
                            ui.label(
                                RichText::new(if rel.is_edge() {
                                    t("popup.edge")
                                } else {
                                    t("popup.stable")
                                })
                                .size(9.5)
                                .color(if rel.is_edge() {
                                    theme::EDGE
                                } else {
                                    theme::OK
                                }),
                            );
                            if pinned {
                                ui.label(
                                    RichText::new(t("common.pin")).size(9.5).color(theme::WARN),
                                );
                            }
                            if let Some(date) = &rel.published_at {
                                ui.label(
                                    RichText::new(human_date(date))
                                        .size(10.5)
                                        .color(theme::DIM),
                                );
                            }
                        });

                        if is_expanded {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);

                            // Сначала варианты основного продукта под x64:
                            // именно их ищет пользователь («Windows Portable»,
                            // «Windows MSI» и прочие). Остальные — под раскрытием.
                            let main_prefix = crate::manager::main_product_prefix(slug, &rel.tag);
                            let mut variants: Vec<&AssetInfo> = Vec::new();
                            let mut extras: Vec<&AssetInfo> = Vec::new();
                            for a in &rel.assets {
                                if crate::manager::is_noise_asset(&a.name) {
                                    continue;
                                }
                                if a.name.starts_with(&main_prefix) && asset_kind(&a.name).is_some() {
                                    variants.push(a);
                                } else {
                                    extras.push(a);
                                }
                            }

                            for asset in &variants {
                                file_row(ui, &rel.tag, asset, &selected, &mut actions);
                            }
                            if !extras.is_empty() {
                                egui::CollapsingHeader::new(
                                    RichText::new(crate::lang::tf(
                                        "releases.more_files",
                                        &[("{n}", &extras.len().to_string())],
                                    ))
                                    .size(11.0),
                                )
                                .id_salt(("extras", rel.tag.as_str()))
                                .default_open(false)
                                .show(ui, |ui| {
                                    for asset in &extras {
                                        file_row(ui, &rel.tag, asset, &selected, &mut actions);
                                    }
                                });
                            }
                            // Исходники — отдельным пунктом.
                            let src_key = asset_key(&rel.tag, &format!("{}-source.zip", rel.tag));
                            let mut checked = selected.contains(&src_key);
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut checked, "").changed() {
                                    actions.push(Action::ToggleAsset(src_key.clone()));
                                }
                                ui.label(
                                    RichText::new("SRC").size(9.5).monospace().color(theme::DIM),
                                );
                                ui.label(RichText::new(t("filter.source")).size(11.5));
                                ui.label(
                                    RichText::new(format!("{}-source.zip", rel.tag))
                                        .size(10.5)
                                        .color(theme::DIM),
                                );
                            });

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.button(t("releases.select_all")).clicked() {
                                    actions.push(Action::SelectRelease(rel.tag.clone()));
                                }
                                let count = selected
                                    .iter()
                                    .filter(|k| k.starts_with(&format!("{}::", rel.tag)))
                                    .count();
                                ui.label(
                                    RichText::new(
                                        tf("popup.selected", &[("{n}", &count.to_string())]),
                                    )
                                    .size(10.5)
                                    .color(theme::DIM),
                                );
                            });
                        }
                    });
                ui.add_space(6.0);
            }
        });

    // --- Липкая нижняя панель: «Скачать выбранное» ---
    ui.add_space(4.0);
    let (count, size, tag) = selection_summary(app, releases);
    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(egui::CornerRadius::same(12))
        .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            // Итог — отдельной строкой, чтобы длинные числа не обрезались.
            ui.label(
                RichText::new(tf(
                    "releases.summary",
                    &[
                        ("{n}", &count.to_string()),
                        ("{s}", &size),
                        ("{t}", &tag),
                    ],
                ))
                .size(11.5),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ready = count > 0;
                    let btn = egui::Button::new(
                        RichText::new(t("releases.download_selected")).size(12.0),
                    )
                    .fill(if ready {
                        theme::ACCENT
                    } else {
                        theme::DIM.gamma_multiply(0.4)
                    })
                    .corner_radius(egui::CornerRadius::same(8));
                    if ui.add(btn).clicked() && ready {
                        actions.push(Action::Download(collect_selection(app, releases)));
                    }
                    if ui.button(t("releases.clear")).clicked() {
                        actions.push(Action::ClearSelection);
                    }
                });
            });
        });
    ui.add_space(2.0);

    for action in actions {
        match action {
            Action::ToggleExpand(tag) => {
                if app.release_expanded.as_deref() == Some(tag.as_str()) {
                    app.release_expanded = None;
                } else {
                    app.release_expanded = Some(tag);
                }
            }
            Action::ToggleAsset(key) => {
                if !app.selected_assets.remove(&key) {
                    app.selected_assets.insert(key);
                }
            }
            Action::SelectRelease(tag) => {
                let Some(rel) = releases.iter().find(|r| r.tag == tag) else {
                    continue;
                };
                for asset in &rel.assets {
                    app.selected_assets
                        .insert(asset_key(&tag, &asset.name));
                }
                app.selected_assets
                    .insert(asset_key(&tag, &format!("{tag}-source.zip")));
            }
            Action::ClearSelection => {
                app.selected_assets.clear();
            }
            Action::Download(groups) => {
                if groups.is_empty() {
                    app.notify(t("notify.err").replace("{e}", t("releases.none")), LogLevel::Warn);
                    continue;
                }
                let st = app.state.clone();
                let handle = st.handle.clone();
                let slug_c = slug.to_string();
                handle.spawn(async move {
                    for (tag, assets) in groups {
                        st.download_fresh(
                            &slug_c,
                            crate::manager::DownloadRequest::Assets(tag, assets),
                        )
                        .await;
                    }
                });
                app.notify(
                    tf("repo.download_started", &[("{s}", slug)]),
                    LogLevel::Info,
                );
            }
            Action::Close => {
                app.releases_open = None;
                app.release_expanded = None;
                app.selected_assets.clear();
            }
        }
    }
}

/// Отмеченные файлы, сгруппированные по релизу.
fn collect_selection(
    app: &CraftApp,
    releases: &[ReleaseInfo],
) -> Vec<(String, Vec<AssetInfo>)> {
    let mut out: Vec<(String, Vec<AssetInfo>)> = Vec::new();
    for rel in releases {
        let mut assets: Vec<AssetInfo> = Vec::new();
        for asset in &rel.assets {
            if app.selected_assets.contains(&asset_key(&rel.tag, &asset.name)) {
                assets.push(asset.clone());
            }
        }
        let src_name = format!("{}-source.zip", rel.tag);
        if app.selected_assets.contains(&asset_key(&rel.tag, &src_name)) {
            assets.push(AssetInfo::source_zip(&rel.tag, rel.source_zip.clone()));
        }
        if !assets.is_empty() {
            out.push((rel.tag.clone(), assets));
        }
    }
    out
}

/// Итог по отмеченным файлам: сколько, какого размера, релиз.
fn selection_summary(app: &CraftApp, releases: &[ReleaseInfo]) -> (usize, String, String) {
    let mut count = 0usize;
    let mut bytes = 0u64;
    let mut tag = String::new();
    for rel in releases {
        for asset in &rel.assets {
            if app.selected_assets.contains(&asset_key(&rel.tag, &asset.name)) {
                count += 1;
                bytes += asset.size;
                if tag.is_empty() {
                    tag = rel.tag.clone();
                }
            }
        }
        let src_name = format!("{}-source.zip", rel.tag);
        if app.selected_assets.contains(&asset_key(&rel.tag, &src_name)) {
            count += 1;
            if tag.is_empty() {
                tag = rel.tag.clone();
            }
        }
    }
    (count, human_size(bytes), tag)
}

/// Отметить по умолчанию файл под текущую ОС в свежем релизе.
pub fn preselect(app: &mut CraftApp, slug: &str, releases: &[ReleaseInfo]) {
    if !app.selected_assets.is_empty() {
        return;
    }
    let Some(rel) = releases
        .iter()
        .find(|r| !r.is_edge())
        .or_else(|| releases.first())
    else {
        return;
    };
    let wanted = os_default_kind(slug, rel);
    if wanted == crate::config::AssetKind::Source {
        return;
    }
    for asset in &rel.assets {
        if asset_kind(&asset.name) == Some(wanted) {
            app.selected_assets
                .insert(asset_key(&rel.tag, &asset.name));
            app.release_expanded = Some(rel.tag.clone());
            return;
        }
    }
}

/// Строка файла релиза: галочка, метка системы, имя и размер.
fn file_row(
    ui: &mut egui::Ui,
    tag: &str,
    asset: &AssetInfo,
    selected: &[String],
    actions: &mut Vec<Action>,
) {
    let key = asset_key(tag, &asset.name);
    let mut checked = selected.contains(&key);
    ui.horizontal(|ui| {
        if ui.checkbox(&mut checked, "").changed() {
            actions.push(Action::ToggleAsset(key.clone()));
        }
        let kind = asset_kind(&asset.name);
        let label = kind
            .map(|k| k.localized_label())
            .unwrap_or(t("releases.other"));
        let badge = kind.map(|k| k.badge()).unwrap_or("···");
        let badge_font = egui::FontId::new(9.5, egui::FontFamily::Monospace);
        let label_font = egui::FontId::new(11.5, egui::FontFamily::Proportional);
        let name_font = egui::FontId::new(10.5, egui::FontFamily::Proportional);
        let size_font = egui::FontId::new(10.5, egui::FontFamily::Proportional);
        let size_text = crate::model::human_size_or_dash(asset.size);

        // Имя файла сокращаем: иначе оно наезжает на размер.
        let fixed = progress::text_w(ui, &badge_font, badge)
            + progress::text_w(ui, &label_font, label)
            + progress::text_w(ui, &size_font, &size_text)
            + 34.0;
        let name = progress::elide(
            ui,
            &asset.name,
            &name_font,
            (ui.available_width() - fixed).max(40.0),
        );

        ui.label(
            RichText::new(badge)
                .size(9.5)
                .monospace()
                .color(theme::DIM),
        );
        ui.label(RichText::new(label).size(11.5));
        ui.label(RichText::new(name).size(10.5).color(theme::DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(size_text).size(10.5).color(theme::DIM));
        });
    });
}

/// Выпадающий список файлов релиза.
///
/// Список собирается из реальных файлов релиза, а не из общих правил.
/// По умолчанию выбран вариант под текущую ОС.
pub fn asset_combo(
    ui: &mut egui::Ui,
    app: &mut CraftApp,
    slug: &str,
    current: crate::config::AssetKind,
) -> Option<crate::config::AssetKind> {
    use crate::config::AssetKind;

    let releases = app
        .state
        .shared
        .repo_runtime(slug)
        .map(|rt| rt.releases.clone())
        .unwrap_or_default();
    let rel = releases
        .iter()
        .find(|r| !r.is_edge())
        .or_else(|| releases.first());

    let mut kinds: Vec<(AssetKind, String)> = Vec::new();
    if let Some(rel) = rel {
        for (k, a) in crate::manager::asset_options(slug, rel) {
            kinds.push((
                k,
                format!("{} · {}", k.localized_label(), crate::model::human_size_or_dash(a.size)),
            ));
        }
    }

    let mut choice = current;
    egui::ComboBox::from_id_salt(("asset_kind", slug))
        .selected_text(current.localized_label())
        .width(260.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut choice,
                AssetKind::Auto,
                format!(
                    "{} · {}",
                    AssetKind::Auto.localized_label(),
                    t("filter.auto_hint")
                ),
            );
            for (k, label) in &kinds {
                ui.selectable_value(&mut choice, *k, label.clone());
            }
            ui.separator();
            ui.selectable_value(
                &mut choice,
                AssetKind::Source,
                AssetKind::Source.localized_label(),
            );
            ui.selectable_value(&mut choice, AssetKind::All, AssetKind::All.localized_label());
        });

    if choice != current {
        Some(choice)
    } else {
        None
    }
}


