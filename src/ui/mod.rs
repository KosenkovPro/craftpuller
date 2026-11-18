//! Окно приложения: верхняя панель, навигация, трей, фоновая работа.

pub mod anim;
pub mod hero;
pub mod logs;
pub mod repos;
pub mod settings;
pub mod theme;

use crate::icon::IconKind;
use crate::lang::{t, tf};
use crate::manager::{AppState, DownloadRequest};
use crate::model::{human_date, human_size, AssetInfo, LogLevel, ReleaseInfo, TaskProgress};
use crate::tray;
use egui::{self, Context, TextureHandle};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Tab {
    Repos,
    Settings,
    Logs,
}

/// Данные всплывающего окна со списком релизов.
#[derive(Clone)]
pub struct ReleasePopup {
    pub slug: String,
    pub releases: Vec<ReleaseInfo>,
    pub pinned: Vec<String>,
}

/// Параметры запуска окна.
#[derive(Debug, Default, Clone)]
pub struct Launch {
    /// Открыть программу свёрнутой в трей.
    pub hidden: bool,
    /// Вкладка, которую нужно открыть при старте.
    pub tab: Option<Tab>,
    /// Репозиторий, окно релизов которого нужно открыть при старте.
    pub popup: Option<String>,
    /// Проверять ли репозитории при старте. false — работать только с кэшем.
    pub auto_refresh: bool,
    /// Обновлять ли репозитории, даже если кэш свежий.
    pub force_refresh: bool,
    /// Карточка, которую нужно раскрыть при старте (в режиме сетки).
    pub hero: Option<String>,
}

pub struct CraftApp {
    pub state: Arc<AppState>,
    pub tab: Tab,
    pub tray: Option<tray::Tray>,
    pub popup: Option<ReleasePopup>,
    /// Развёрнутый релиз в окне релизов.
    pub popup_expanded: Option<String>,
    /// Отмеченные файлы в окне релизов.
    pub popup_assets: HashSet<String>,
    /// Кэш загруженных иконок.
    pub icons: HashMap<String, TextureHandle>,
    /// Иконка самого приложения — заглушка для репозиториев без иконки.
    pub app_icon: Option<TextureHandle>,
    /// Поле ввода нового репозитория.
    pub new_repo: String,
    pub toast: Option<(String, LogLevel, Instant)>,
    pub exit_requested: bool,
    pub hide_on_start: bool,
    /// Вкладка, которую нужно открыть при старте.
    pub start_tab: Option<Tab>,
    /// Репозиторий, окно релизов которого нужно открыть при старте.
    pub startup_popup: Option<String>,
    /// Состояние карточки-героя в режиме сетки.
    pub hero: hero::HeroState,
}

impl CraftApp {
    pub fn new(state: Arc<AppState>, launch: Launch) -> Self {
        let tray = match tray::Tray::build() {
            Ok(t) => Some(t),
            Err(e) => {
                state.shared.log(
                    LogLevel::Error,
                    crate::lang::tf("err.tray", &[("{e}", &e.to_string())]),
                );
                None
            }
        };
        // При старте сразу проверяем выбранные репозитории и подтягиваем иконки.
        if launch.auto_refresh && state.cfg().repos.iter().any(|r| r.enabled) {
            let st = state.clone();
            let handle = st.handle.clone();
            let force = launch.force_refresh;
            handle.spawn(async move {
                st.refresh_all(force).await;
                st.refresh_icons().await;
            });
        }
        // Раскрытая карточка, заданная в командной строке.
        let hero_state = {
            let mut hero = hero::HeroState::new();
            if let Some(slug) = launch.hero {
                hero.open = Some(slug.clone());
                hero.progress.set_target(1.0);
                state.shared.log(
                    LogLevel::Info,
                    format!("CraftPull: hero card requested — {slug}"),
                );
            }
            hero
        };
        Self {
            state,
            tab: Tab::Repos,
            tray,
            popup: None,
            popup_expanded: None,
            popup_assets: HashSet::new(),
            icons: HashMap::new(),
            app_icon: None,
            new_repo: String::new(),
            toast: None,
            exit_requested: false,
            hide_on_start: launch.hidden,
            start_tab: launch.tab,
            startup_popup: launch.popup,
            hero: hero_state,
        }
    }

    /// Показать всплывающее сообщение.
    pub fn notify(&mut self, text: impl Into<String>, level: LogLevel) {
        self.toast = Some((text.into(), level, Instant::now()));
    }

    fn show_window(&self, ctx: &Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn handle_tray(&mut self, ctx: &Context) {
        for ev in tray::poll_menu_events() {
            let id = ev.id.0.clone();
            match id.as_str() {
                tray::ID_SHOW => self.show_window(ctx),
                tray::ID_PAUSE => {
                    let paused = self.state.toggle_pause();
                    if let Some(t) = &self.tray {
                        t.set_paused(paused);
                    }
                }
                tray::ID_STOP => {
                    self.state.stop_all();
                    self.notify(t("notify.stopped"), LogLevel::Warn);
                }
                tray::ID_EXIT => {
                    self.exit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            }
        }
        for ev in tray::poll_tray_events() {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = ev
            {
                self.show_window(ctx);
            }
        }
    }

    /// Состояние иконки трея.
    fn tray_kind(&self) -> IconKind {
        if self.state.shared.control.is_paused() {
            IconKind::Paused
        } else if self.state.shared.is_busy() {
            IconKind::Working
        } else if self.state.shared.has_errors() {
            IconKind::Error
        } else {
            IconKind::Idle
        }
    }

    /// Текст иконки трея с учётом языка.
    fn tray_tooltip(kind: IconKind) -> &'static str {
        match kind {
            IconKind::Idle => t("tray.tooltip_idle"),
            IconKind::Working => t("tray.tooltip_working"),
            IconKind::Paused => t("tray.tooltip_paused"),
            IconKind::Error => t("tray.tooltip_error"),
        }
    }

    /// Загрузить (и закешировать) иконку репозитория.
    pub fn icon_texture(&mut self, ctx: &Context, slug: &str, path: &str) -> Option<TextureHandle> {
        if let Some(tex) = self.icons.get(slug) {
            return Some(tex.clone());
        }
        let (rgba, size) = crate::icons::load_rgba(std::path::Path::new(path))?;
        let tex = ctx.load_texture(
            format!("repo-icon-{slug}"),
            egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
            egui::TextureOptions::LINEAR,
        );
        self.icons.insert(slug.to_string(), tex.clone());
        Some(tex)
    }

    /// Иконка самого приложения. Ставится тем карточкам репозиториев,
    /// у которых нет собственной иконки.
    pub fn app_icon_texture(&mut self, ctx: &Context) -> Option<TextureHandle> {
        if let Some(tex) = &self.app_icon {
            return Some(tex.clone());
        }
        let size = 128usize;
        let rgba = crate::icon::make_rgba(size as u32, IconKind::Idle);
        let tex = ctx.load_texture(
            "craftpull-app-icon",
            egui::ColorImage::from_rgba_unmultiplied([size, size], &rgba),
            egui::TextureOptions::LINEAR,
        );
        self.app_icon = Some(tex.clone());
        Some(tex)
    }

    /// Переключить язык.
    pub fn set_lang(&mut self, lang: u8) {
        crate::lang::set(lang);
        let mut cfg = self.state.cfg();
        cfg.lang = lang;
        self.state.save_cfg(cfg);
        if let Some(tr) = &mut self.tray {
            tr.set_texts();
        }
    }
}

impl eframe::App for CraftApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        theme::apply(ctx);
        self.handle_tray(ctx);

        // Старт свёрнутым в трей.
        if self.hide_on_start {
            self.hide_on_start = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Escape сворачивает раскрытую карточку-героя.
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.hero.open.is_some() {
            self.hero.close();
        }

        // Пока идёт загрузка, окно должно перерисовываться само: иначе
        // прогресс и скорость в списке загрузок замирают.
        if self
            .tasks()
            .iter()
            .any(|t| !t.state.is_finished())
        {
            ctx.request_repaint();
        }

        // Нужная вкладка при запуске.
        if let Some(tab) = self.start_tab.take() {
            self.tab = tab;
        }

        // Окно релизов по запросу (данные могут подгрузиться не сразу).
        if let Some(slug) = self.startup_popup.clone() {
            let runtime = self.state.shared.repo_runtime(&slug);
            let releases = runtime
                .as_ref()
                .map(|rt| rt.releases.clone())
                .filter(|r| !r.is_empty())
                .or_else(|| self.state.cached_releases(&slug))
                .unwrap_or_default();
            if !releases.is_empty() {
                let pinned = self
                    .state
                    .cfg()
                    .repo(&slug)
                    .map(|r| r.pinned.clone())
                    .unwrap_or_default();
                self.popup_expanded = runtime
                    .as_ref()
                    .and_then(|rt| rt.latest_stable.as_ref())
                    .map(|r| r.tag.clone())
                    .or_else(|| releases.first().map(|r| r.tag.clone()));
                self.popup = Some(ReleasePopup {
                    slug: slug.clone(),
                    releases,
                    pinned,
                });
                self.startup_popup = None;
            }
        }

        let cfg = self.state.cfg();

        // Сворачивание в трей по кнопке закрытия.
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && cfg.close_to_tray {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // Верхняя панель.
        egui::TopBottomPanel::top("top").frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(16, 10))
                .stroke(egui::Stroke::NONE),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Иконка приложения — ровно та же, что в заголовке окна.
                // Клик по ней открывает главное меню со списком репозиториев.
                let icon = self.app_icon_texture(ctx);
                if let Some(tex) = icon {
                    let resp = ui
                        .add(
                            egui::Image::new(&tex)
                                .fit_to_exact_size(egui::vec2(24.0, 24.0))
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text(t("app.home"));
                    if resp.clicked() {
                        self.tab = Tab::Repos;
                    }
                }
                ui.label(egui::RichText::new(t("app.name")).size(20.0).strong());
                ui.label(
                    egui::RichText::new(t("app.subtitle"))
                        .size(12.0)
                        .color(theme::DIM),
                );

                ui.add_space(12.0);
                self.status_chip(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Переключатель языка: одна кнопка — русский ↔ английский.
                    let ru = crate::lang::current() == crate::lang::RU;
                    if ui
                        .button(if ru { "EN" } else { "RU" })
                        .on_hover_text(t("settings.language"))
                        .clicked()
                    {
                        self.set_lang(if ru { crate::lang::EN } else { crate::lang::RU });
                    }
                    if ui.button(t("common.stop")).clicked() {
                        self.state.stop_all();
                        self.notify(t("notify.stopped"), LogLevel::Warn);
                    }
                    let paused = self.state.shared.control.is_paused();
                    if ui
                        .button(if paused {
                            t("common.resume")
                        } else {
                            t("common.pause")
                        })
                        .clicked()
                    {
                        let p = self.state.toggle_pause();
                        if let Some(tr) = &self.tray {
                            tr.set_paused(p);
                        }
                    }
                    if ui.button(t("common.download_new")).clicked() {
                        let st = self.state.clone();
                        let handle = st.handle.clone();
                        handle.spawn(async move { st.download_all().await });
                        self.notify(t("notify.update_all"), LogLevel::Info);
                    }
                    if ui.button(t("common.refresh")).clicked() {
                        let st = self.state.clone();
                        let handle = st.handle.clone();
                        handle.spawn(async move {
                            st.refresh_all(true).await;
                            st.refresh_icons().await;
                        });
                    }
                });
            });
        });

        // Боковая навигация.
        egui::SidePanel::left("nav")
            .exact_width(196.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(12))
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                ui.add_space(4.0);
                if ui
                    .add_sized([168.0, 36.0], egui::Button::new(t("nav.repos")))
                    .clicked()
                {
                    self.tab = Tab::Repos;
                }
                if ui
                    .add_sized([168.0, 36.0], egui::Button::new(t("nav.settings")))
                    .clicked()
                {
                    self.tab = Tab::Settings;
                }
                if ui
                    .add_sized([168.0, 36.0], egui::Button::new(t("nav.logs")))
                    .clicked()
                {
                    self.tab = Tab::Logs;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.label(
                        egui::RichText::new(t("app.author"))
                            .size(11.0)
                            .color(theme::DIM),
                    );
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .size(11.0)
                            .color(theme::DIM),
                    );
                    ui.label(
                        egui::RichText::new(t("nav.footer"))
                            .size(11.0)
                            .color(theme::DIM),
                    );
                    ui.add_space(6.0);
                });
            });

        // Основная панель.
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Repos => repos::show(ui, self, ctx),
            Tab::Settings => settings::show(ui, self),
            Tab::Logs => logs::show(ui, self),
        });

        // Окно со списком релизов.
        self.show_release_popup(ctx);

        // Всплывающее уведомление.
        if let Some((text, level, at)) = self.toast.clone() {
            if at.elapsed() < std::time::Duration::from_secs(4) {
                egui::TopBottomPanel::bottom("toast")
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let color = match level {
                            LogLevel::Ok => theme::OK,
                            LogLevel::Warn => theme::WARN,
                            LogLevel::Error => theme::DANGER,
                            LogLevel::Info => theme::ACCENT,
                        };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(text).color(color));
                        });
                    });
            } else {
                self.toast = None;
            }
        }

        // Значок трея отражает состояние.
        let kind = self.tray_kind();
        if let Some(tr) = &mut self.tray {
            tr.set_kind(kind);
            tr.set_tooltip(CraftApp::tray_tooltip(kind));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.state.shared.log(LogLevel::Info, crate::lang::t("log.exit").to_string());
    }
}

impl CraftApp {
    fn status_chip(&mut self, ui: &mut egui::Ui) {
        let paused = self.state.shared.control.is_paused();
        let busy = self.state.shared.is_busy();
        let (text, color) = if paused {
            (t("status.paused"), theme::WARN)
        } else if busy {
            (t("status.downloading"), theme::OK)
        } else {
            (t("status.ready"), theme::DIM)
        };
        let (speed, total, done) = self.progress_summary();
        let active = self
            .tasks()
            .iter()
            .filter(|t| !t.state.is_finished())
            .count();
        let label = if busy {
            let files = tf("status.files", &[("{n}", &active.to_string())]);
            format!(
                "{text} · {files} · {} · {}/{}",
                crate::model::human_speed(speed),
                human_size(done),
                human_size(total)
            )
        } else {
            text.to_string()
        };
        egui::Frame::new()
            .fill(color.gamma_multiply(0.16))
            .corner_radius(egui::CornerRadius::same(14))
            .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.5)))
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(label).size(12.0).color(color));
            });
    }

    /// Суммарный прогресс по всем активным загрузкам.
    pub fn progress_summary(&self) -> (f64, u64, u64) {
        let mut speed = 0.0;
        let mut total = 0u64;
        let mut done = 0u64;
        if let Ok(guard) = self.state.shared.tasks.try_lock() {
            for t in guard.values() {
                if t.state == crate::model::TaskState::Running {
                    speed += t.speed;
                }
                total += t.total;
                done += t.downloaded;
            }
        }
        (speed, total, done)
    }

    /// Активные задачи по репозиторию.
    pub fn tasks_for(&self, slug: &str) -> Vec<TaskProgress> {
        match self.state.shared.tasks.try_lock() {
            Ok(guard) => guard
                .values()
                .filter(|t| t.repo == slug)
                .cloned()
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Все задачи загрузки: активные и завершённые.
    pub fn tasks(&self) -> Vec<TaskProgress> {
        match self.state.shared.tasks.try_lock() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    fn show_release_popup(&mut self, ctx: &Context) {
        let Some(popup) = self.popup.clone() else {
            return;
        };
        let mut open = true;
        let mut actions: Vec<PopupAction> = Vec::new();

        egui::Window::new(tf("popup.title", &[("{s}", &popup.slug)]))
            .open(&mut open)
            .default_width(680.0)
            .default_height(520.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for rel in &popup.releases {
                        let pinned = popup.pinned.iter().any(|p| p == &rel.tag);
                        let expanded = self.popup_expanded.as_deref() == Some(rel.tag.as_str());
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
                            .inner_margin(egui::Margin::same(10))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(if expanded { "v" } else { ">" })
                                        .on_hover_text(t("popup.assets"))
                                        .clicked()
                                    {
                                        actions.push(PopupAction::Expand(rel.tag.clone()));
                                    }
                                    ui.label(egui::RichText::new(&rel.tag).strong());
                                    ui.label(
                                        egui::RichText::new(if rel.is_edge() {
                                            t("popup.edge")
                                        } else {
                                            t("popup.stable")
                                        })
                                        .size(10.0)
                                        .color(if rel.is_edge() {
                                            theme::EDGE
                                        } else {
                                            theme::OK
                                        }),
                                    );
                                    if let Some(date) = &rel.published_at {
                                        ui.label(
                                            egui::RichText::new(human_date(date))
                                                .size(11.0)
                                                .color(theme::DIM),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let pin_text = if pinned {
                                                t("common.unpin")
                                            } else {
                                                t("common.pin")
                                            };
                                            if ui.button(pin_text).clicked() {
                                                actions.push(PopupAction::TogglePin(
                                                    popup.slug.clone(),
                                                    rel.tag.clone(),
                                                ));
                                            }
                                            if ui.button(t("common.download")).clicked() {
                                                actions.push(PopupAction::DownloadTag(
                                                    popup.slug.clone(),
                                                    rel.tag.clone(),
                                                ));
                                            }
                                        },
                                    );
                                });

                                if expanded {
                                    ui.separator();
                                    ui.label(
                                        egui::RichText::new(t("popup.assets"))
                                            .size(11.5)
                                            .color(theme::DIM),
                                    );
                                    for asset in &rel.assets {
                                        let key = format!("{}::{}", rel.tag, asset.name);
                                        let mut checked = self.popup_assets.contains(&key);
                                        ui.horizontal(|ui| {
                                            if ui.checkbox(&mut checked, "").changed() {
                                                actions.push(PopupAction::ToggleAsset(key.clone()));
                                            }
                                            ui.label(
                                                egui::RichText::new(&asset.name).size(11.5),
                                            );
                                            ui.label(
                                                egui::RichText::new(human_size(asset.size))
                                                    .size(10.5)
                                                    .color(theme::DIM),
                                            );
                                        });
                                    }
                                    ui.horizontal(|ui| {
                                        let selected = self
                                            .popup_assets
                                            .iter()
                                            .filter(|k| k.starts_with(&format!("{}::", rel.tag)))
                                            .count();
                                        ui.label(
                                            egui::RichText::new(tf(
                                                "popup.selected",
                                                &[("{n}", &selected.to_string())],
                                            ))
                                            .size(11.0)
                                            .color(theme::DIM),
                                        );
                                        if ui.button(t("popup.download_selected")).clicked() {
                                            actions.push(PopupAction::DownloadAssets(
                                                popup.slug.clone(),
                                                rel.tag.clone(),
                                            ));
                                        }
                                        if ui.button(t("popup.download_all")).clicked() {
                                            actions.push(PopupAction::DownloadEveryFile(
                                                popup.slug.clone(),
                                                rel.tag.clone(),
                                            ));
                                        }
                                    });
                                }
                            });
                        ui.add_space(6.0);
                    }
                });
            });

        if !open {
            self.popup = None;
            self.popup_assets.clear();
        }
        for action in actions {
            match action {
                PopupAction::Expand(tag) => {
                    if self.popup_expanded.as_deref() == Some(tag.as_str()) {
                        self.popup_expanded = None;
                    } else {
                        self.popup_expanded = Some(tag);
                    }
                }
                PopupAction::ToggleAsset(key) => {
                    if !self.popup_assets.remove(&key) {
                        self.popup_assets.insert(key);
                    }
                }
                PopupAction::TogglePin(slug, tag) => {
                    let mut cfg = self.state.cfg();
                    if let Some(rc) = cfg.repo_mut(&slug) {
                        if let Some(pos) = rc.pinned.iter().position(|p| p == &tag) {
                            rc.pinned.remove(pos);
                        } else {
                            rc.pinned.push(tag.clone());
                        }
                    }
                    self.state.save_cfg(cfg.clone());
                    if let Some(p) = &mut self.popup {
                        p.pinned = cfg
                            .repo(&slug)
                            .map(|r| r.pinned.clone())
                            .unwrap_or_default();
                    }
                    self.notify(t("notify.pin_changed"), LogLevel::Ok);
                }
                PopupAction::DownloadTag(slug, tag) => {
                    let st = self.state.clone();
                    let handle = st.handle.clone();
                    let slug_c = slug.clone();
                    handle.spawn(async move {
                        st.download_fresh(&slug_c, DownloadRequest::Tag(tag.clone()))
                            .await
                    });
                    self.notify(tf("repo.download_started", &[("{s}", &slug)]), LogLevel::Info);
                }
                PopupAction::DownloadAssets(slug, tag) => {
                    let assets: Vec<AssetInfo> = self
                        .popup_assets
                        .iter()
                        .filter_map(|k| k.strip_prefix(&format!("{tag}::")))
                        .filter_map(|name| {
                            let rel = self
                                .popup
                                .as_ref()?
                                .releases
                                .iter()
                                .find(|r| r.tag == tag)?;
                            rel.assets.iter().find(|a| &a.name == name).cloned()
                        })
                        .collect();
                    if assets.is_empty() {
                        self.notify(t("notify.err").replace("{e}", "no files selected"), LogLevel::Warn);
                        continue;
                    }
                    let st = self.state.clone();
                    let handle = st.handle.clone();
                    let slug_c = slug.clone();
                    handle.spawn(async move {
                        st.download_fresh(&slug_c, DownloadRequest::Assets(tag.clone(), assets.clone()))
                            .await
                    });
                    self.notify(tf("repo.download_started", &[("{s}", &slug)]), LogLevel::Info);
                }
                PopupAction::DownloadEveryFile(slug, tag) => {
                    let assets: Vec<AssetInfo> = self
                        .popup
                        .as_ref()
                        .and_then(|p| p.releases.iter().find(|r| r.tag == tag))
                        .map(|r| r.assets.clone())
                        .unwrap_or_default();
                    if assets.is_empty() {
                        continue;
                    }
                    let st = self.state.clone();
                    let handle = st.handle.clone();
                    let slug_c = slug.clone();
                    handle.spawn(async move {
                        st.download_fresh(&slug_c, DownloadRequest::Assets(tag.clone(), assets.clone()))
                            .await
                    });
                    self.notify(tf("repo.download_started", &[("{s}", &slug)]), LogLevel::Info);
                }
            }
        }
    }
}

enum PopupAction {
    Expand(String),
    ToggleAsset(String),
    TogglePin(String, String),
    DownloadTag(String, String),
    DownloadAssets(String, String),
    DownloadEveryFile(String, String),
}
