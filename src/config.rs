//! Конфигурация приложения, список репозиториев, формат JSON для обмена списком.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Сколько файлов можно качать одновременно — максимум.
pub const MAX_PARALLEL: usize = 16;

/// Список репозиториев, которые добавляются при первом запуске (ни один не выбран).
pub const DEFAULT_REPOS: &[&str] = &[
    "storytold/pdfcraft",
    "storytold/wordcraft",
    "storytold/gridcraft",
    "storytold/deckcraft",
    "storytold/photocraft",
    "storytold/vectorcraft",
    "storytold/figcraft",
    "storytold/designcraft",
    "storytold/cadcraft",
    "storytold/texturecraft",
    "storytold/materialcraft",
    "storytold/filmcraft",
    "storytold/lightcraft",
    "storytold/effectcraft",
    "storytold/soundcraft",
    "storytold/artcraft",
    "storytold/craft-anything",
    "storytold/craft-libs",
    "storytold/craft-launcher",
];

/// Какой файл релиза скачивать.
///
/// Значение выбирается из реальных файлов релиза: приложение определяет вид
/// файла по имени и подставляет нужный под текущую ОС.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AssetKind {
    /// Автоматически: файл под текущую ОС (по умолчанию).
    #[default]
    #[serde(alias = "Portable", alias = "Auto")]
    Auto,
    /// Windows: portable-архив (zip, без установщика).
    WindowsPortable,
    /// Windows: MSI-установщик.
    WindowsMsi,
    /// Windows: установщик setup.exe.
    WindowsSetup,
    /// Ubuntu: AppImage.
    LinuxAppImage,
    /// Ubuntu: tar.gz.
    LinuxTarGz,
    /// Ubuntu: deb.
    LinuxDeb,
    /// Ubuntu: rpm.
    LinuxRpm,
    /// Только архив с исходниками.
    Source,
    /// Все файлы релиза.
    All,
}

impl AssetKind {
    /// Подпись с учётом языка интерфейса.
    pub fn localized_label(self) -> &'static str {
        match self {
            AssetKind::Auto => crate::lang::t("filter.auto"),
            AssetKind::WindowsPortable => crate::lang::t("filter.windows_portable"),
            AssetKind::WindowsMsi => crate::lang::t("filter.windows_msi"),
            AssetKind::WindowsSetup => crate::lang::t("filter.windows_setup"),
            AssetKind::LinuxAppImage => crate::lang::t("filter.linux_appimage"),
            AssetKind::LinuxTarGz => crate::lang::t("filter.linux_targz"),
            AssetKind::LinuxDeb => crate::lang::t("filter.linux_deb"),
            AssetKind::LinuxRpm => crate::lang::t("filter.linux_rpm"),
            AssetKind::Source => crate::lang::t("filter.source"),
            AssetKind::All => crate::lang::t("filter.all"),
        }
    }

    /// Короткая метка ОС для списка вариантов.
    pub fn badge(self) -> &'static str {
        match self {
            AssetKind::WindowsPortable | AssetKind::WindowsMsi | AssetKind::WindowsSetup => "WIN",
            AssetKind::LinuxAppImage | AssetKind::LinuxTarGz | AssetKind::LinuxDeb
            | AssetKind::LinuxRpm => "LNX",
            AssetKind::Source => "SRC",
            _ => "···",
        }
    }

    /// Виды файлов, подходящих под текущую ОС, в порядке предпочтения.
    pub fn for_current_os() -> &'static [AssetKind] {
        if cfg!(target_os = "windows") {
            &[
                AssetKind::WindowsPortable,
                AssetKind::WindowsMsi,
                AssetKind::WindowsSetup,
            ]
        } else if cfg!(target_os = "macos") {
            &[]
        } else {
            &[
                AssetKind::LinuxAppImage,
                AssetKind::LinuxTarGz,
                AssetKind::LinuxDeb,
                AssetKind::LinuxRpm,
            ]
        }
    }
}

/// Режим отображения списка репозиториев.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    /// Сетка иконок — режим по умолчанию.
    Grid,
    /// Иконка слева, описание справа.
    List,
    /// Без картинок.
    Light,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::List => crate::lang::t("repo.view_list"),
            ViewMode::Grid => crate::lang::t("repo.view_grid"),
            ViewMode::Light => crate::lang::t("repo.view_light"),
        }
    }
}

/// Как скачиваются релизы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadMode {
    /// Файлы и репозитории качаются одновременно, в пределах лимита.
    Parallel,
    /// Строго по одному файлу за раз, репозитории по очереди.
    Sequential,
}

impl DownloadMode {
    pub fn label(self) -> &'static str {
        match self {
            DownloadMode::Parallel => crate::lang::t("download.mode_parallel"),
            DownloadMode::Sequential => crate::lang::t("download.mode_sequential"),
        }
    }
}

impl Default for DownloadMode {
    fn default() -> Self {
        DownloadMode::Parallel
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub slug: String,
    /// Выбран ли репозиторий для загрузки. По умолчанию — нет.
    pub enabled: bool,
    /// Релизы, которые нельзя удалять автоматически.
    pub pinned: Vec<String>,
    pub filter: AssetKind,
    /// Дополнительно клонировать/обновлять исходники репозитория через git.
    pub clone_source: bool,
    /// Краткое описание (подтягивается с GitHub, можно править вручную).
    pub description: String,
    /// Локальный путь к сохранённой иконке репозитория.
    pub icon: Option<String>,
}

impl RepoConfig {
    pub fn new(slug: &str) -> Self {
        Self {
            slug: slug.to_string(),
            enabled: false,
            pinned: Vec::new(),
            filter: AssetKind::Auto,
            clone_source: false,
            description: String::new(),
            icon: None,
        }
    }

    /// Имя репозитория без владельца.
    pub fn short_name(&self) -> String {
        self.slug.rsplit('/').next().unwrap_or(&self.slug).to_string()
    }

    /// Владелец репозитория (автор).
    pub fn author(&self) -> String {
        self.slug
            .split('/')
            .next()
            .unwrap_or("other")
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Куда складываются скачанные версии.
    pub root_dir: PathBuf,
    /// 0 — без ограничения скорости, иначе килобит/с.
    pub speed_limit_kbps: u64,
    pub max_parallel: usize,
    /// Параллельные или последовательные загрузки.
    #[serde(default)]
    pub download_mode: DownloadMode,
    pub stable_channel: bool,
    pub edge_channel: bool,
    pub verify_sha256: bool,
    /// Удалять прошлые версии после успешной загрузки новых.
    pub delete_old_versions: bool,
    pub autostart: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub github_token: String,
    /// Репозиторий с исходниками самого CraftPull: "owner/repo" или локальный путь.
    pub self_update_repo: String,
    pub self_update_branch: String,
    /// Язык интерфейса: 0 — английский, 1 — русский.
    pub lang: u8,
    pub view_mode: ViewMode,
    pub repos: Vec<RepoConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root_dir: default_root_dir(),
            speed_limit_kbps: 0,
            max_parallel: 8,
            download_mode: DownloadMode::Parallel,
            stable_channel: true,
            edge_channel: false,
            verify_sha256: true,
            delete_old_versions: true,
            autostart: false,
            start_minimized: false,
            close_to_tray: true,
            github_token: String::new(),
            self_update_repo: String::new(),
            self_update_branch: "main".to_string(),
            lang: crate::lang::EN,
            view_mode: ViewMode::Grid,
            repos: DEFAULT_REPOS
                .iter()
                .map(|s| RepoConfig::new(s))
                .collect(),
        }
    }
}

/// Папка загрузок по умолчанию.
fn default_root_dir() -> PathBuf {
    PathBuf::from(r"E:\Open\Src")
}

/// Путь к файлу конфигурации.
pub fn config_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CraftPull");
    dir.join("config.json")
}

/// Папка с данными приложения (логи, иконки, исходники для самообновления).
pub fn data_dir() -> PathBuf {
    let dir = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("CraftPull");
    dir
}

/// Папка с локальным кэшем иконок.
pub fn icons_dir() -> PathBuf {
    data_dir().join("icons")
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(mut cfg) => {
                    cfg.ensure_repos();
                    cfg
                }
                Err(e) => {
                    eprintln!("CraftPull: the config file is broken ({e}), creating a new one");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// Добавляет репозитории из списка по умолчанию, которых ещё нет.
    fn ensure_repos(&mut self) {
        for slug in DEFAULT_REPOS {
            if !self.repos.iter().any(|r| r.slug == *slug) {
                self.repos.push(RepoConfig::new(slug));
            }
        }
        if self.max_parallel == 0 {
            self.max_parallel = 1;
        }
        // Лимит параллельных загрузок — целое число в разумных пределах.
        self.max_parallel = self.max_parallel.clamp(1, MAX_PARALLEL);
    }

    pub fn repo(&self, slug: &str) -> Option<&RepoConfig> {
        self.repos.iter().find(|r| r.slug == slug)
    }

    pub fn repo_mut(&mut self, slug: &str) -> Option<&mut RepoConfig> {
        self.repos.iter_mut().find(|r| r.slug == slug)
    }

    /// Экспорт списка репозиториев в JSON.
    pub fn export_list(&self) -> RepoListFile {
        RepoListFile {
            version: 1,
            exported_at: chrono::Local::now().to_rfc3339(),
            repos: self
                .repos
                .iter()
                .map(|r| RepoEntry {
                    slug: r.slug.clone(),
                    enabled: r.enabled,
                    description: r.description.clone(),
                    pinned: r.pinned.clone(),
                    filter: r.filter,
                    clone_source: r.clone_source,
                })
                .collect(),
        }
    }

    /// Импорт списка репозиториев из JSON (слияние с текущим списком).
    pub fn import_list(&mut self, file: &RepoListFile) -> usize {
        let mut added = 0usize;
        for entry in &file.repos {
            let slug = entry.slug.trim().to_string();
            if slug.is_empty() || !slug.contains('/') {
                continue;
            }
            match self.repo_mut(&slug) {
                Some(existing) => {
                    existing.enabled = entry.enabled;
                    if !entry.description.is_empty() {
                        existing.description = entry.description.clone();
                    }
                    for p in &entry.pinned {
                        if !existing.pinned.contains(p) {
                            existing.pinned.push(p.clone());
                        }
                    }
                    existing.filter = entry.filter;
                    existing.clone_source = entry.clone_source;
                }
                None => {
                    self.repos.push(RepoConfig {
                        slug: slug.clone(),
                        enabled: entry.enabled,
                        pinned: entry.pinned.clone(),
                        filter: entry.filter,
                        clone_source: entry.clone_source,
                        description: entry.description.clone(),
                        icon: None,
                    });
                    added += 1;
                }
            }
        }
        added
    }
}

/// Формат файла со списком репозиториев.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoListFile {
    pub version: u32,
    pub exported_at: String,
    pub repos: Vec<RepoEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub slug: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub filter: AssetKind,
    #[serde(default)]
    pub clone_source: bool,
}
