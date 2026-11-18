//! Оркестратор: обновление репозиториев, загрузки, очистка старых версий.

use crate::cache::Cache;
use crate::config::{AssetFilter, Config, DownloadMode};
use crate::download::{task_id, Control, Downloader, RateLimiter, TaskRegistry};
use crate::github;
use crate::git;
use crate::icons;
use crate::model::*;
use crate::storage;
use reqwest::Client;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

/// Сколько минут кэш релизов считается свежим.
pub const CACHE_TTL_SECONDS: i64 = 15 * 60;

/// Что именно скачиваем.
#[derive(Debug, Clone)]
pub enum DownloadRequest {
    /// Последние релизы по включённым каналам (стабильный и/или нестабильный).
    Targets,
    /// Конкретный тег релиза.
    Tag(String),
    /// Явно выбранные пользователем файлы релиза.
    Assets(String, Vec<AssetInfo>),
}

/// Состояние репозитория во время работы программы.
#[derive(Debug, Default, Clone)]
pub struct RepoRuntime {
    pub slug: String,
    pub releases: Vec<ReleaseInfo>,
    pub latest_stable: Option<ReleaseInfo>,
    pub latest_edge: Option<ReleaseInfo>,
    pub installed: Vec<InstalledVersion>,
    pub last_check: Option<String>,
    pub last_error: Option<String>,
    pub checking: bool,
}

/// Общее состояние приложения.
pub struct Shared {
    pub cfg: Mutex<Config>,
    pub repos: Mutex<HashMap<String, RepoRuntime>>,
    pub control: Arc<Control>,
    pub limiter: Arc<RateLimiter>,
    pub client: Client,
    pub tasks: TaskRegistry,
    pub logs: Mutex<VecDeque<LogEntry>>,
    pub log_file: Mutex<Option<std::fs::File>>,
    pub busy: AtomicUsize,
    /// Кэш релизов на диске: переживает перезапуск и бережёт лимит GitHub API.
    pub cache: Mutex<Cache>,
    /// Ворота файлов: сколько файлов качается одновременно.
    pub file_gate: Mutex<Arc<Semaphore>>,
    /// Ворота репозиториев: сколько репозиториев качается одновременно.
    pub repo_gate: Mutex<Arc<Semaphore>>,
}

impl Shared {
    /// Записать строку в журнал и в файл лога.
    pub fn log(&self, level: LogLevel, text: impl Into<String>) {
        let text = text.into();
        let entry = LogEntry {
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            level,
            text: text.clone(),
        };
        let line = format!("[{}] [{}] {}", entry.time, level_tag(level), text);
        eprintln!("{line}");
        if let Ok(mut file) = self.log_file.lock() {
            use std::io::Write;
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
        if let Ok(mut logs) = self.logs.lock() {
            logs.push_back(entry);
            while logs.len() > 3000 {
                logs.pop_front();
            }
        }
    }

    pub fn repo_runtime(&self, slug: &str) -> Option<RepoRuntime> {
        self.repos.lock().ok().and_then(|m| m.get(slug).cloned())
    }

    pub fn set_repo_runtime(&self, rt: RepoRuntime) {
        if let Ok(mut m) = self.repos.lock() {
            m.insert(rt.slug.clone(), rt);
        }
    }

    pub fn is_busy(&self) -> bool {
        if self.busy.load(Ordering::SeqCst) > 0 {
            return true;
        }
        // Идущая загрузка файлов тоже считается занятостью.
        if let Ok(tasks) = self.tasks.try_lock() {
            return tasks.values().any(|t| !t.state.is_finished());
        }
        false
    }

    /// Есть ли ошибки у последней проверки репозиториев.
    pub fn has_errors(&self) -> bool {
        self.repos
            .lock()
            .map(|m| m.values().any(|r| r.last_error.is_some()))
            .unwrap_or(false)
    }

    /// Снимок настроек.
    pub fn cfg_snapshot(&self) -> Config {
        self.cfg.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Сколько файлов можно качать одновременно с учётом режима загрузок.
    pub fn parallel_limit(&self) -> usize {
        let cfg = self.cfg_snapshot();
        match cfg.download_mode {
            DownloadMode::Sequential => 1,
            DownloadMode::Parallel => cfg.max_parallel.max(1),
        }
    }

    /// Ворота файлов.
    pub fn file_gate(&self) -> Arc<Semaphore> {
        self.file_gate
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| Arc::new(Semaphore::new(1)))
    }

    /// Ворота репозиториев.
    pub fn repo_gate(&self) -> Arc<Semaphore> {
        self.repo_gate
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| Arc::new(Semaphore::new(1)))
    }

    /// Пересобрать ворота под текущие настройки.
    pub fn sync_gates(&self) {
        let limit = self.parallel_limit();
        if let Ok(mut g) = self.file_gate.lock() {
            *g = Arc::new(Semaphore::new(limit));
        }
        if let Ok(mut g) = self.repo_gate.lock() {
            *g = Arc::new(Semaphore::new(limit));
        }
    }
}

fn level_tag(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Info => "INFO",
        LogLevel::Ok => "OK",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERR",
    }
}

/// Handle приложения: общий доступ к состоянию и запуск фоновых задач.
pub struct AppState {
    pub shared: Arc<Shared>,
    pub handle: tokio::runtime::Handle,
}

impl AppState {
    pub fn new(handle: tokio::runtime::Handle) -> Result<Arc<Self>, String> {
        let mut cfg = Config::load();
        crate::lang::set(cfg.lang);
        // Синхронизируем состояние автозапуска с реестром Windows.
        if crate::selfupdate::autostart_enabled(std::path::Path::new("")) {
            cfg.autostart = true;
        }
        let client = github::build_client(&cfg.github_token)?;

        if let Err(e) = std::fs::create_dir_all(&cfg.root_dir) {
            return Err(crate::lang::tf(
                "err.folder",
                &[("{dir}", &cfg.root_dir.display().to_string()), ("{e}", &e.to_string())],
            ));
        }

        let data = crate::config::data_dir();
        let _ = std::fs::create_dir_all(&data);
        let _ = std::fs::create_dir_all(crate::config::icons_dir());
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(data.join("craftpull.log"))
            .ok();

        let limiter = Arc::new(RateLimiter::default());
        limiter.set_limit(cfg.speed_limit_kbps * 1024);

        let mut repos = HashMap::new();
        // Кэш с диска: показываем сохранённые релизы сразу, не дожидаясь сети.
        let cache = Cache::load();
        for rc in &cfg.repos {
            let installed = storage::scan_installed(&cfg.root_dir, &rc.slug);
            let mut rt = RepoRuntime {
                slug: rc.slug.clone(),
                installed,
                ..Default::default()
            };
            if let Some(cached) = cache.get(&rc.slug) {
                let (stable, edge) = split_channels(&cached.releases);
                rt.releases = cached.releases.clone();
                rt.latest_stable = stable;
                rt.latest_edge = edge;
                rt.last_check = cached.last_check.clone();
            }
            repos.insert(rc.slug.clone(), rt);
        }

        let gate_limit = if cfg.download_mode == DownloadMode::Sequential {
            1
        } else {
            cfg.max_parallel.max(1)
        };

        let shared = Arc::new(Shared {
            cfg: Mutex::new(cfg),
            repos: Mutex::new(repos),
            control: Arc::new(Control::default()),
            limiter,
            client,
            tasks: Arc::new(std::sync::Mutex::new(HashMap::new())),
            logs: Mutex::new(VecDeque::new()),
            log_file: Mutex::new(log_file),
            busy: AtomicUsize::new(0),
            cache: Mutex::new(cache),
            file_gate: Mutex::new(Arc::new(Semaphore::new(gate_limit))),
            repo_gate: Mutex::new(Arc::new(Semaphore::new(gate_limit))),
        });

        let state = Arc::new(Self { shared, handle });
        state.shared.log(
            LogLevel::Info,
            crate::lang::tf(
                "log.started",
                &[("{dir}", &state.cfg().root_dir.display().to_string())],
            ),
        );
        Ok(state)
    }

    pub fn cfg(&self) -> Config {
        self.shared
            .cfg
            .lock()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    pub fn save_cfg(&self, cfg: Config) {
        if let Ok(mut c) = self.shared.cfg.lock() {
            *c = cfg;
        }
        let snapshot = self.cfg();
        if let Err(e) = snapshot.save() {
            self.shared
                .log(LogLevel::Error, crate::lang::tf("err.cfg_save", &[("{e}", &e.to_string())]));
        }
        self.shared
            .limiter
            .set_limit(snapshot.speed_limit_kbps * 1024);
        self.shared.sync_gates();
        crate::lang::set(snapshot.lang);
    }

    /// Запомнить релизы репозитория в кэше на диске.
    fn remember_releases(&self, slug: &str, releases: &[ReleaseInfo], last_check: Option<String>) {
        if let Ok(mut cache) = self.shared.cache.lock() {
            let now = chrono::Local::now().timestamp();
            cache.put(slug, releases.to_vec(), last_check, Some(now));
            if let Err(e) = cache.save() {
                self.shared.log(
                    LogLevel::Warn,
                    crate::lang::tf("err.cache_save", &[("{e}", &e.to_string())]),
                );
            }
        }
    }

    /// Релизы репозитория из кэша — используется, если сеть недоступна.
    pub fn cached_releases(&self, slug: &str) -> Option<Vec<ReleaseInfo>> {
        self.shared
            .cache
            .lock()
            .ok()
            .and_then(|c| c.get(slug).map(|r| r.releases.clone()))
    }

    /// Давно ли проверяли репозиторий. Свежие данные повторно не запрашиваем:
    /// анонимный доступ к GitHub API ограничен 60 запросами в час.
    pub fn is_stale(&self, slug: &str) -> bool {
        let checked = self
            .shared
            .cache
            .lock()
            .ok()
            .and_then(|c| c.checked_at(slug));
        match checked {
            Some(ts) => chrono::Local::now().timestamp() - ts > CACHE_TTL_SECONDS,
            None => true,
        }
    }

    /// Обновить список релизов и описание репозитория.
    pub async fn refresh_repo(self: &Arc<Self>, slug: &str) {
        self.shared.busy.fetch_add(1, Ordering::SeqCst);
        let mut rt = self
            .shared
            .repo_runtime(slug)
            .unwrap_or_else(|| RepoRuntime {
                slug: slug.to_string(),
                ..Default::default()
            });
        rt.checking = true;
        self.shared.set_repo_runtime(rt.clone());

        match github::list_releases(&self.shared.client, slug).await {
            Ok(mut releases) => {
                releases.sort_by(|a, b| {
                    b.published_at
                        .cmp(&a.published_at)
                        .then_with(|| b.tag.cmp(&a.tag))
                });
                let (stable, edge) = split_channels(&releases);
                rt.releases = releases;
                rt.latest_stable = stable;
                rt.latest_edge = edge;
                rt.last_check = Some(chrono::Local::now().format("%d.%m.%Y %H:%M").to_string());
                rt.last_error = None;
                self.shared.set_repo_runtime(rt.clone());
                self.remember_releases(slug, &rt.releases, rt.last_check.clone());
                self.shared.log(
                    LogLevel::Info,
                    crate::lang::tf(
                        "log.checked",
                        &[("{slug}", slug), ("{n}", &rt.releases.len().to_string())],
                    ),
                );
            }
            Err(e) => {
                rt.last_error = Some(e.clone());
                rt.checking = false;
                self.shared.set_repo_runtime(rt);
                self.shared.log(LogLevel::Error, format!("{slug}: {e}"));
            }
        }
        self.shared.busy.fetch_sub(1, Ordering::SeqCst);

        // Описание подтягиваем только один раз, чтобы не тратить лимит API.
        let need_description = self
            .cfg()
            .repo(slug)
            .map(|r| r.description.trim().is_empty())
            .unwrap_or(false);
        if need_description {
            if let Ok(info) = github::repo_info(&self.shared.client, slug).await {
                if let Some(desc) = info.description {
                    let mut cfg = self.cfg();
                    if let Some(rc) = cfg.repo_mut(slug) {
                        rc.description = desc;
                    }
                    self.save_cfg(cfg);
                }
            }
        }
    }

    /// Проверить все выбранные репозитории.
    ///
    /// `force` игнорирует «свежесть» кэша: свежие данные повторно не запрашиваются,
    /// чтобы не выбирать лимит GitHub API (60 запросов в час для анонимных клиентов).
    pub async fn refresh_all(self: &Arc<Self>, force: bool) {
        let cfg = self.cfg();
        let enabled: Vec<String> = cfg
            .repos
            .iter()
            .filter(|r| r.enabled)
            .map(|r| r.slug.clone())
            .collect();
        if enabled.is_empty() {
            self.shared.log(
                LogLevel::Warn,
                crate::lang::t("log.no_repo").to_string(),
            );
            return;
        }
        let mut joins = Vec::new();
        for slug in enabled {
            if !force && !self.is_stale(&slug) {
                continue;
            }
            let me = self.clone();
            joins.push(tokio::spawn(async move { me.refresh_repo(&slug).await }));
        }
        for j in joins {
            let _ = j.await;
        }
    }

    /// Скачать/обновить иконки репозиториев в локальный кэш.
    pub async fn refresh_icons(self: &Arc<Self>) {
        let dir = crate::config::icons_dir();
        let slugs: Vec<String> = self.cfg().repos.iter().map(|r| r.slug.clone()).collect();
        for slug in slugs {
            let cached = self
                .cfg()
                .repo(&slug)
                .and_then(|r| r.icon.clone())
                .filter(|p| std::path::Path::new(p).exists());
            if cached.is_some() {
                continue;
            }
            if let Some(path) = icons::ensure_icon(&self.shared.client, &slug, &dir).await {
                let mut cfg = self.cfg();
                if let Some(rc) = cfg.repo_mut(&slug) {
                    rc.icon = Some(path.to_string_lossy().to_string());
                }
                self.save_cfg(cfg);
            }
        }
    }

    /// Скачать иконку одного репозитория, если её ещё нет в кэше.
    pub async fn ensure_icon(self: &Arc<Self>, slug: &str) -> Option<PathBuf> {
        let dir = crate::config::icons_dir();
        let cached = self
            .cfg()
            .repo(slug)
            .and_then(|r| r.icon.clone())
            .filter(|p| std::path::Path::new(p).exists());
        if let Some(p) = cached {
            return Some(PathBuf::from(p));
        }
        let path = icons::ensure_icon(&self.shared.client, slug, &dir).await?;
        let mut cfg = self.cfg();
        if let Some(rc) = cfg.repo_mut(slug) {
            rc.icon = Some(path.to_string_lossy().to_string());
        }
        self.save_cfg(cfg);
        Some(path)
    }

    /// Какие релизы считаем целями для репозитория.
    pub fn plan_targets(&self, slug: &str) -> Vec<ReleaseInfo> {
        let cfg = self.cfg();
        let Some(rt) = self.shared.repo_runtime(slug) else {
            return Vec::new();
        };
        let mut out: Vec<ReleaseInfo> = Vec::new();
        if cfg.stable_channel {
            if let Some(r) = rt.latest_stable.clone() {
                out.push(r);
            }
        }
        if cfg.edge_channel {
            if let Some(r) = rt.latest_edge.clone() {
                out.push(r);
            }
        }
        let mut seen = HashSet::new();
        out.retain(|r| seen.insert(r.tag.clone()));
        out
    }

    /// Скачать целевые релизы репозитория и убрать старые версии.
    ///
    /// Флаг отмены здесь **не** сбрасывается: он живёт на весь заход, иначе
    /// «Стоп» прерывал бы только один репозиторий, а следующий тут же
    /// стартовал бы заново.
    pub async fn download_repo(self: &Arc<Self>, slug: &str, request: DownloadRequest) {
        let cfg = self.cfg();
        let filter = cfg
            .repo(slug)
            .map(|r| r.filter)
            .unwrap_or(AssetFilter::Portable);

        let mut rt = self
            .shared
            .repo_runtime(slug)
            .unwrap_or_else(|| RepoRuntime {
                slug: slug.to_string(),
                ..Default::default()
            });
        if rt.releases.is_empty() {
            self.refresh_repo(slug).await;
            rt = self
                .shared
                .repo_runtime(slug)
                .unwrap_or_else(|| RepoRuntime {
                    slug: slug.to_string(),
                    ..Default::default()
                });
        }

        // План: что и какими файлами качаем.
        let mut plan: Vec<(ReleaseInfo, Vec<AssetInfo>)> = Vec::new();
        match request {
            DownloadRequest::Targets => {
                for target in self.plan_targets(slug) {
                    let assets = select_assets(filter, &target);
                    plan.push((target, assets));
                }
            }
            DownloadRequest::Tag(tag) => {
                if let Some(rel) = rt.releases.iter().find(|r| r.tag == tag) {
                    let assets = select_assets(filter, rel);
                    plan.push((rel.clone(), assets));
                }
            }
            DownloadRequest::Assets(tag, assets) => {
                if let Some(rel) = rt.releases.iter().find(|r| r.tag == tag) {
                    plan.push((rel.clone(), assets));
                }
            }
        }

        if plan.is_empty() {
            self.shared.log(
                LogLevel::Warn,
                crate::lang::tf("log.no_releases", &[("{slug}", slug)]),
            );
            return;
        }

        let root = cfg.root_dir.clone();
        let mut downloaded_tags: Vec<String> = Vec::new();
        let mut total_failures = 0usize;

        for (target, assets) in plan {
            let tag = target.tag.clone();
            let channel = target.channel();
            let dir = storage::version_dir(&root, slug, &tag);

            if assets.is_empty() {
                self.shared.log(
                    LogLevel::Warn,
                    crate::lang::tf("log.no_assets", &[("{slug}", slug), ("{tag}", &tag)]),
                );
                continue;
            }

            // «Стоп» — дальше не идём.
            if self.shared.control.is_cancelled() {
                break;
            }

            // Уже скачано?
            let existing = rt
                .installed
                .iter()
                .find(|v| v.tag == tag && !v.files.is_empty());
            if existing.is_some() {
                self.shared.log(
                    LogLevel::Info,
                    crate::lang::tf("log.already", &[("{slug}", slug), ("{tag}", &tag)]),
                );
                downloaded_tags.push(tag);
                continue;
            }

            let meta = storage::VersionMeta {
                tag: tag.clone(),
                channel: channel.label().to_string(),
                installed_at: chrono::Local::now().to_rfc3339(),
            };
            let _ = std::fs::create_dir_all(&dir);
            let _ = storage::write_meta(&dir, &meta);

            self.shared.log(
                LogLevel::Info,
                crate::lang::tf(
                    "log.downloading",
                    &[
                        ("{slug}", slug),
                        ("{tag}", &tag),
                        ("{size}", channel.label()),
                        ("{n}", &assets.len().to_string()),
                    ],
                ),
            );

            let parallel = cfg.download_mode == DownloadMode::Parallel;
            let mut failures = 0usize;

            if parallel {
                // Параллельно: файлы релиза качаются одновременно, но не больше
                // лимита одновременных загрузок.
                let gate = self.shared.file_gate();
                let mut handles = Vec::new();
                for asset in assets {
                    // Ожидание места в очереди тоже должно прерываться «Стопом».
                    let permit = tokio::select! {
                        _ = self.shared.control.wait_cancelled() => break,
                        p = gate.clone().acquire_owned() => match p {
                            Ok(p) => p,
                            Err(_) => break,
                        },
                    };
                    let dl = Downloader {
                        client: self.shared.client.clone(),
                        limiter: self.shared.limiter.clone(),
                        control: self.shared.control.clone(),
                        tasks: self.shared.tasks.clone(),
                        verify_sha256: cfg.verify_sha256,
                    };
                    let id = task_id(slug, &tag, &asset.name);
                    let asset_clone = asset.clone();
                    let slug_c = slug.to_string();
                    let tag_c = tag.clone();
                    let dir_c = dir.clone();
                    handles.push(tokio::spawn(async move {
                        let _permit = permit;
                        let res = dl.fetch(&id, &slug_c, &tag_c, &asset_clone, &dir_c).await;
                        (id, asset_clone.name.clone(), res)
                    }));
                }

                for h in handles {
                    if let Ok((_id, name, res)) = h.await {
                        match res {
                            Ok(()) => {
                                self.shared.log(
                                    LogLevel::Ok,
                                    crate::lang::tf(
                                        "log.ready",
                                        &[("{slug}", slug), ("{tag}", &tag), ("{name}", &name)],
                                    ),
                                );
                            }
                            Err(e) => {
                                failures += 1;
                                if self.shared.control.is_cancelled() {
                                    self.shared
                                        .log(LogLevel::Warn, crate::lang::t("log.stopped").to_string());
                                } else {
                                    self.shared
                                        .log(LogLevel::Error, format!("{slug} {tag}: {name} — {e}"));
                                }
                            }
                        }
                    }
                }
            } else {
                // Последовательно: строго один файл за раз.
                for asset in assets {
                    if self.shared.control.is_cancelled() {
                        break;
                    }
                    let dl = Downloader {
                        client: self.shared.client.clone(),
                        limiter: self.shared.limiter.clone(),
                        control: self.shared.control.clone(),
                        tasks: self.shared.tasks.clone(),
                        verify_sha256: cfg.verify_sha256,
                    };
                    let id = task_id(slug, &tag, &asset.name);
                    match dl.fetch(&id, slug, &tag, &asset, &dir).await {
                        Ok(()) => {
                            self.shared.log(
                                LogLevel::Ok,
                                crate::lang::tf(
                                    "log.ready",
                                    &[("{slug}", slug), ("{tag}", &tag), ("{name}", &asset.name)],
                                ),
                            );
                        }
                        Err(e) => {
                            failures += 1;
                            if self.shared.control.is_cancelled() {
                                self.shared
                                    .log(LogLevel::Warn, crate::lang::t("log.stopped").to_string());
                            } else {
                                self.shared.log(
                                    LogLevel::Error,
                                    format!("{slug} {tag}: {} — {e}", asset.name),
                                );
                            }
                        }
                    }
                }
            }
            total_failures += failures;

            if failures == 0 {
                downloaded_tags.push(tag.clone());
            }
        }

        // Уборка старых версий — только если новые действительно скачались.
        // Если загрузка сорвалась, старые версии остаются на месте: терять их
        // из-за неудачной сети нельзя.
        if total_failures == 0 && !downloaded_tags.is_empty() {
            self.cleanup_repo(slug, &downloaded_tags).await;
        } else if total_failures > 0 {
            self.shared.log(
                LogLevel::Warn,
                crate::lang::tf(
                    "log.cleanup_skipped",
                    &[("{slug}", slug), ("{n}", &total_failures.to_string())],
                ),
            );
        }

        // Опционально: исходники репозитория.
        let repo_cfg = self.cfg().repos.into_iter().find(|r| r.slug == slug);
        if let Some(rc) = repo_cfg {
            if rc.clone_source {
                self.clone_sources(slug).await;
            }
        }

        // Обновить список установленного.
        let mut rt = self
            .shared
            .repo_runtime(slug)
            .unwrap_or_else(|| RepoRuntime {
                slug: slug.to_string(),
                ..Default::default()
            });
        rt.installed = storage::scan_installed(&root, slug);
        self.shared.set_repo_runtime(rt);
    }

    /// Удалить старые версии, оставив новые и закреплённые.
    pub async fn cleanup_repo(self: &Arc<Self>, slug: &str, keep: &[String]) {
        let cfg = self.cfg();
        if !cfg.delete_old_versions {
            return;
        }
        let pinned = cfg
            .repo(slug)
            .map(|r| r.pinned.clone())
            .unwrap_or_default();
        let keep_set: HashSet<String> = keep.iter().cloned().collect();
        let removed = storage::cleanup_versions(&cfg.root_dir, slug, &keep_set, &pinned);
        for path in removed {
            self.shared.log(
                LogLevel::Info,
                crate::lang::tf(
                    "log.deleted",
                    &[("{slug}", slug), ("{path}", &path.display().to_string())],
                ),
            );
        }
    }

    /// Клонировать/обновить исходники репозитория.
    pub async fn clone_sources(self: &Arc<Self>, slug: &str) {
        let cfg = self.cfg();
        let dir = storage::source_dir(&cfg.root_dir, slug);
        let url = git::repo_url(slug);
        self.shared.log(
            LogLevel::Info,
            crate::lang::tf("log.updating_sources", &[("{slug}", slug)]),
        );
        let slug_c = slug.to_string();
        let result = tokio::task::spawn_blocking(move || git::sync_repo(&url, &dir, ""))
            .await
            .unwrap_or_else(|e| Err(format!("{e}")));
        match result {
            Ok(msg) => self.shared.log(LogLevel::Ok, format!("{slug_c}: {msg}")),
            Err(e) => self.shared.log(LogLevel::Error, format!("{slug_c}: {e}")),
        }
    }

    /// Скачать новые версии для всех выбранных репозиториев.
    pub async fn download_all(self: &Arc<Self>) {
        // Новая серия загрузок: сбрасываем флаг отмены, оставшийся от прошлой.
        self.shared.control.reset();
        self.refresh_all(true).await;
        if self.shared.control.is_cancelled() {
            self.shared
                .log(LogLevel::Warn, crate::lang::t("log.stopped").to_string());
            return;
        }
        let cfg = self.cfg();
        let enabled: Vec<String> = cfg
            .repos
            .iter()
            .filter(|r| r.enabled)
            .map(|r| r.slug.clone())
            .collect();
        if enabled.is_empty() {
            return;
        }

        match cfg.download_mode {
            DownloadMode::Sequential => {
                // Строго по очереди: один репозиторий за раз.
                for slug in enabled {
                    if self.shared.control.is_cancelled() {
                        break;
                    }
                    self.download_repo(&slug, DownloadRequest::Targets).await;
                }
            }
            DownloadMode::Parallel => {
                // Одновременно, но не больше лимита: иначе упрёмся в лимит API.
                let gate = self.shared.repo_gate();
                let mut handles = Vec::new();
                for slug in enabled {
                    if self.shared.control.is_cancelled() {
                        break;
                    }
                    let permit = tokio::select! {
                        _ = self.shared.control.wait_cancelled() => break,
                        p = gate.clone().acquire_owned() => match p {
                            Ok(p) => p,
                            Err(_) => break,
                        },
                    };
                    let st = Arc::clone(self);
                    let slug_c = slug.clone();
                    handles.push(tokio::spawn(async move {
                        let _permit = permit;
                        st.download_repo(&slug_c, DownloadRequest::Targets).await;
                    }));
                }
                for h in handles {
                    let _ = h.await;
                }
            }
        }
    }

    /// Одиночная загрузка, запущенная пользователем: сбрасывает флаг отмены,
    /// чтобы прошлый «Стоп» не мешал новому запуску.
    pub async fn download_fresh(self: &Arc<Self>, slug: &str, request: DownloadRequest) {
        self.shared.control.reset();
        self.download_repo(slug, request).await;
    }

    /// Остановить все текущие загрузки.
    pub fn stop_all(&self) {
        self.shared.control.cancel();
        self.shared.log(
            LogLevel::Warn,
            crate::lang::t("log.stopped").to_string(),
        );
    }

    pub fn toggle_pause(&self) -> bool {
        let paused = self.shared.control.toggle();
        self.shared.log(
            LogLevel::Info,
            if paused {
                crate::lang::t("notify.paused").to_string()
            } else {
                crate::lang::t("notify.resumed").to_string()
            },
        );
        paused
    }
}

/// Разделить релизы по каналам: (стабильный, нестабильный).
pub fn split_channels(releases: &[ReleaseInfo]) -> (Option<ReleaseInfo>, Option<ReleaseInfo>) {
    let mut stable = None;
    let mut edge = None;
    for r in releases {
        if r.is_edge() {
            if edge.is_none() {
                edge = Some(r.clone());
            }
        } else if stable.is_none() {
            stable = Some(r.clone());
        }
    }
    (stable, edge)
}

/// Выбрать файлы релиза по правилу фильтра.
pub fn select_assets(filter: AssetFilter, rel: &ReleaseInfo) -> Vec<AssetInfo> {
    match filter {
        AssetFilter::All => rel.assets.clone(),
        AssetFilter::Source => vec![AssetInfo::source_zip(&rel.tag, rel.source_zip.clone())],
        AssetFilter::Portable => portable_assets(rel),
    }
}

/// Portable-сборки: Windows x64 и Ubuntu x86_64.
pub fn portable_assets(rel: &ReleaseInfo) -> Vec<AssetInfo> {
    let mut out = Vec::new();
    if let Some(a) = pick_windows(rel) {
        out.push(a);
    }
    if let Some(a) = pick_ubuntu(rel) {
        out.push(a);
    }
    if out.is_empty() {
        out.push(AssetInfo::source_zip(&rel.tag, rel.source_zip.clone()));
    }
    out
}

/// Windows x64: portable zip → msi → setup.exe.
fn pick_windows(rel: &ReleaseInfo) -> Option<AssetInfo> {
    let mut best: Option<(i32, AssetInfo)> = None;
    for a in &rel.assets {
        let n = a.name.to_ascii_lowercase();
        if n.contains("arm64") || n.contains("aarch64") {
            continue;
        }
        let windows = n.contains("windows")
            || n.ends_with(".msi")
            || n.contains("-setup.exe")
            || n.contains("_x64-setup.exe");
        if !windows {
            continue;
        }
        let x64 = n.contains("x64") || n.contains("x86_64") || n.contains("amd64");
        if !x64 {
            continue;
        }
        let score = if n.contains("portable") && n.ends_with(".zip") {
            30
        } else if n.ends_with(".msi") {
            20
        } else if n.contains("setup.exe") {
            10
        } else {
            5
        };
        match &best {
            Some((s, _)) if *s >= score => {}
            _ => best = Some((score, a.clone())),
        }
    }
    best.map(|(_, a)| a)
}

/// Ubuntu x86_64: AppImage → tar.gz → deb.
fn pick_ubuntu(rel: &ReleaseInfo) -> Option<AssetInfo> {
    let mut best: Option<(i32, AssetInfo)> = None;
    for a in &rel.assets {
        let n = a.name.to_ascii_lowercase();
        if !n.contains("linux") {
            continue;
        }
        let x64 = n.contains("x86_64") || n.contains("x64") || n.contains("amd64");
        if !x64 {
            continue;
        }
        let score = if n.ends_with(".appimage") {
            30
        } else if n.ends_with(".tar.gz") {
            20
        } else if n.ends_with(".deb") {
            10
        } else {
            5
        };
        match &best {
            Some((s, _)) if *s >= score => {}
            _ => best = Some((score, a.clone())),
        }
    }
    best.map(|(_, a)| a)
}

/// Куда положить скачанное для репозитория.
#[allow(dead_code)]
pub fn target_dir(root: &std::path::Path, slug: &str, tag: &str) -> PathBuf {
    storage::version_dir(root, slug, tag)
}
