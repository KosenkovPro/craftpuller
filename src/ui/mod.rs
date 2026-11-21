//! Окно приложения: верхняя панель, навигация, трей, фоновая работа.

pub mod anim;
pub mod hero;
pub mod logs;
pub mod progress;
pub mod releases;
pub mod repos;
pub mod settings;
pub mod theme;

use crate::icon::IconKind;
use crate::lang::{t, tf};
use crate::manager::AppState;
use crate::model::{human_size, LogLevel, TaskProgress};
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
    /// Репозиторий, у которого открыт список релизов (панель справа).
    pub releases_open: Option<String>,
    /// Раскрытый релиз внутри списка.
    pub release_expanded: Option<String>,
    /// Отмеченные файлы: «тег::имя файла».
    pub selected_assets: HashSet<String>,
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
    /// Репозиторий, список релизов которого нужно открыть при старте.
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
        // Тихая фоновая проверка обновлений самой программы.
        if launch.auto_refresh {
            crate::updater::spawn_check(state.clone());
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
            releases_open: None,
            release_expanded: None,
            selected_assets: HashSet::new(),
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
        if crate::icons::disabled() {
            return None;
        }
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

        // Уведомление от фоновой проверки обновлений программы.
        if let Some((text, level)) = crate::updater::take_toast() {
            self.notify(text, level);
        }

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

        // То же самое для проверки репозиториев: список проверок должен
        // обновляться на глазах.
        let checking = self
            .state
            .shared
            .checks
            .lock()
            .map(|c| c.running)
            .unwrap_or(false);
        if checking {
            ctx.request_repaint();
        }

        // Нужная вкладка при запуске.
        if let Some(tab) = self.start_tab.take() {
            self.tab = tab;
        }

        // Список релизов по запросу (данные могут подгрузиться не сразу).
        if let Some(slug) = self.startup_popup.clone() {
            let runtime = self.state.shared.repo_runtime(&slug);
            let releases = runtime
                .as_ref()
                .map(|rt| rt.releases.clone())
                .filter(|r| !r.is_empty())
                .or_else(|| self.state.cached_releases(&slug))
                .unwrap_or_default();
            if !releases.is_empty() {
                self.releases_open = Some(slug.clone());
                releases::preselect(self, &slug, &releases);
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
                    // Лицензия программы: при наведении — пояснение, что она разрешает.
                    ui.label(
                        egui::RichText::new(t("license.name"))
                            .size(11.0)
                            .color(theme::DIM),
                    )
                    .on_hover_text(t("license.hint"));
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

        // Панель релизов справа — внутри страницы приложения.
        // Показываем до основной панели: иначе она накроет её собой.
        if self.tab == Tab::Repos {
            self.show_releases_panel(ctx);
        }

        // Основная панель.
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Repos => repos::show(ui, self, ctx),
            Tab::Settings => settings::show(ui, self),
            Tab::Logs => logs::show(ui, self),
        });

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
        // Идёт проверка репозиториев — показываем, сколько уже проверено.
        let checks = self
            .state
            .shared
            .checks
            .lock()
            .ok()
            .map(|c| (c.running, c.done(), c.total()))
            .unwrap_or((false, 0, 0));
        let (text, color): (String, egui::Color32) = if paused {
            (t("status.paused").to_string(), theme::WARN)
        } else if checks.0 {
            (
                tf(
                    "status.checking",
                    &[
                        ("{n}", &checks.1.to_string()),
                        ("{m}", &checks.2.to_string()),
                    ],
                ),
                theme::ACCENT,
            )
        } else if busy {
            (t("status.downloading").to_string(), theme::OK)
        } else {
            (t("status.ready").to_string(), theme::DIM)
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
        let list: Vec<TaskProgress> = match self.state.shared.tasks.try_lock() {
            Ok(guard) => guard
                .values()
                .filter(|t| t.repo == slug)
                .cloned()
                .collect(),
            Err(_) => Vec::new(),
        };
        with_queue(list)
    }

    /// Все задачи загрузки: активные и завершённые.
    pub fn tasks(&self) -> Vec<TaskProgress> {
        let list: Vec<TaskProgress> = match self.state.shared.tasks.try_lock() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(_) => Vec::new(),
        };
        with_queue(list)
    }

    /// Открыть список релизов репозитория (панель справа).
    pub fn open_releases(&mut self, slug: &str) {
        let releases = self
            .state
            .shared
            .repo_runtime(slug)
            .map(|rt| rt.releases.clone())
            .unwrap_or_default();
        self.releases_open = Some(slug.to_string());
        self.release_expanded = None;
        self.selected_assets.clear();
        releases::preselect(self, slug, &releases);
        // Релизов ещё нет — подтягиваем их с GitHub.
        if releases.is_empty() {
            let st = self.state.clone();
            let handle = st.handle.clone();
            let slug_c = slug.to_string();
            handle.spawn(async move {
                let _ = st.refresh_repo(&slug_c).await;
            });
        }
    }

    /// Панель релизов справа — внутри страницы приложения.
    fn show_releases_panel(&mut self, ctx: &Context) {
        let Some(slug) = self.releases_open.clone() else {
            return;
        };
        let releases = self
            .state
            .shared
            .repo_runtime(&slug)
            .map(|rt| rt.releases.clone())
            .unwrap_or_default();
        // Данные подгрузились позже — отмечаем файл под текущую ОС.
        if !releases.is_empty() && self.selected_assets.is_empty() {
            releases::preselect(self, &slug, &releases);
        }
        egui::SidePanel::right("releases")
            .exact_width(430.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(12))
                    .stroke(egui::Stroke::NONE),
            )
            .show(ctx, |ui| {
                releases::panel(ui, self, &slug, &releases);
            });
    }
}

/// Проставить номера очереди: активные задачи по порядку постановки в очередь.
fn with_queue(mut list: Vec<TaskProgress>) -> Vec<TaskProgress> {
    let mut active: Vec<usize> = list
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.state.is_finished())
        .map(|(i, _)| i)
        .collect();
    active.sort_by_key(|i| (list[*i].started_at, list[*i].seq));
    let total = active.len();
    for (pos, idx) in active.into_iter().enumerate() {
        list[idx].queue = pos + 1;
        list[idx].queue_total = total;
    }
    list
}


