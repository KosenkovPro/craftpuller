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
    // Скриншоты и запись экрана.
    "flameshot-org/flameshot",
    "obsproject/obs-studio",
    "NickeManarin/ScreenToGif",
    // Видео, аудио и конвертация.
    "mifi/lossless-cut",
    "HandBrake/HandBrake",
    "audacity/audacity",
    "mpv-player/mpv",
    "jeffvli/feishin",
    "SV-stark/FileConverter-rs",
    // Графика и 3D.
    "d2phap/ImageGlass",
    "darktable-org/darktable",
    "FreeCAD/FreeCAD",
    // Система, файлы и сеть.
    "microsoft/PowerToys",
    "dundee/gdu",
    "iterate-ch/cyberduck",
    "qbittorrent/qBittorrent",
    "pbatard/rufus",
    "rustdesk/rustdesk",
    // Безопасность, карты и оффлайн-базы.
    "keepassxreboot/keepassxc",
    "organicmaps/organicmaps",
    "kiwix/kiwix-desktop",
    "kiwix/kiwix-android",
    // Self-hosted серверы.
    "passbolt/passbolt_api",
    "immich-app/immich",
    "owncloud/ocis",
    "nextcloud/server",
    "Freika/dawarich",
    "jellyfin/jellyfin",
    "kieraneglin/pinchflat",
    "navidrome/navidrome",
    "advplyr/audiobookshelf",
    "Kareadita/Kavita",
    "owncast/owncast",
    "kiwix/kiwix-tools",
    "VERT-sh/VERT",
    "VERT-sh/vertd",
];

/// Короткое назначение программы для репозиториев по умолчанию.
///
/// Это пользовательские данные, а не переводы интерфейса, поэтому они живут
/// здесь, а не в `lang.rs`.
pub const DEFAULT_PURPOSES: &[(&str, &str)] = &[
    ("storytold/pdfcraft", "PDF-документы"),
    ("storytold/wordcraft", "Текстовый редактор"),
    ("storytold/gridcraft", "Таблицы"),
    ("storytold/deckcraft", "Презентации"),
    ("storytold/photocraft", "Работа с фото"),
    ("storytold/vectorcraft", "Векторная графика"),
    ("storytold/figcraft", "Прототипы"),
    ("storytold/designcraft", "Дизайн и вёрстка"),
    ("storytold/cadcraft", "Чертежи и CAD"),
    ("storytold/texturecraft", "Текстуры"),
    ("storytold/materialcraft", "Материалы"),
    ("storytold/filmcraft", "Монтаж видео"),
    ("storytold/lightcraft", "Освещение"),
    ("storytold/effectcraft", "Эффекты"),
    ("storytold/soundcraft", "Звук и музыка"),
    ("storytold/artcraft", "Живопись"),
    ("storytold/craft-anything", "Всё сразу"),
    ("storytold/craft-libs", "Библиотеки"),
    ("storytold/craft-launcher", "Запуск программ"),
    // Скриншоты и запись экрана.
    ("flameshot-org/flameshot", "Скриншоты и аннотации"),
    ("obsproject/obs-studio", "Запись и стриминг"),
    ("NickeManarin/ScreenToGif", "Запись в GIF/MP4"),
    // Видео, аудио и конвертация.
    ("mifi/lossless-cut", "Нарезка видео"),
    ("HandBrake/HandBrake", "Сжатие видео"),
    ("audacity/audacity", "Запись и монтаж звука"),
    ("mpv-player/mpv", "Всеядный медиаплеер"),
    ("jeffvli/feishin", "Плеер для сервера"),
    ("SV-stark/FileConverter-rs", "Конвертер файлов"),
    // Графика и 3D.
    ("d2phap/ImageGlass", "Просмотрщик фото"),
    ("darktable-org/darktable", "Растровый фоторедактор"),
    ("FreeCAD/FreeCAD", "3D-моделирование и CAD"),
    // Система, файлы и сеть.
    ("microsoft/PowerToys", "Утилиты Windows"),
    ("dundee/gdu", "Анализ занятого места"),
    ("iterate-ch/cyberduck", "FTP / SFTP клиент"),
    ("qbittorrent/qBittorrent", "Торрент-клиент"),
    ("pbatard/rufus", "Запись флешек USB"),
    ("rustdesk/rustdesk", "Удалённый рабочий стол"),
    // Безопасность, карты и оффлайн-базы.
    ("keepassxreboot/keepassxc", "Менеджер паролей"),
    ("organicmaps/organicmaps", "Карты и навигатор"),
    ("kiwix/kiwix-desktop", "Оффлайн Википедия"),
    ("kiwix/kiwix-android", "Википедия (Android)"),
    // Self-hosted серверы.
    ("passbolt/passbolt_api", "Сервер паролей и 2FA"),
    ("immich-app/immich", "Аналог Google Фото"),
    ("owncloud/ocis", "Файловое облако"),
    ("nextcloud/server", "Календари и контакты"),
    ("Freika/dawarich", "История поездок"),
    ("jellyfin/jellyfin", "Домашний кинотеатр"),
    ("kieraneglin/pinchflat", "Архив YouTube-каналов"),
    ("navidrome/navidrome", "Музыкальный сервер"),
    ("advplyr/audiobookshelf", "Сервер аудиокниг"),
    ("Kareadita/Kavita", "Сервер книг и PDF"),
    ("owncast/owncast", "Сервер прямых эфиров"),
    ("kiwix/kiwix-tools", "Веб-сервер Википедии"),
    ("VERT-sh/VERT", "Веб-конвертер файлов"),
    ("VERT-sh/vertd", "Демон веб-конвертера"),
];

/// Назначение по умолчанию для репозитория. Пустая строка — назначения нет.
pub fn default_purpose(slug: &str) -> &'static str {
    DEFAULT_PURPOSES
        .iter()
        .find(|(s, _)| *s == slug)
        .map(|(_, p)| *p)
        .unwrap_or("")
}

/// Категория программы по умолчанию: репозиторий → ключ группы.
///
/// Это пользовательские данные, а не переводы интерфейса, поэтому они живут
/// здесь, а не в `lang.rs`. Ключи категорий: own, capture, media, graphics,
/// system, security, server, other.
pub const DEFAULT_CATEGORIES: &[(&str, &str)] = &[
    // Свои программы.
    ("storytold/pdfcraft", "own"),
    ("storytold/wordcraft", "own"),
    ("storytold/gridcraft", "own"),
    ("storytold/deckcraft", "own"),
    ("storytold/photocraft", "own"),
    ("storytold/vectorcraft", "own"),
    ("storytold/figcraft", "own"),
    ("storytold/designcraft", "own"),
    ("storytold/cadcraft", "own"),
    ("storytold/texturecraft", "own"),
    ("storytold/materialcraft", "own"),
    ("storytold/filmcraft", "own"),
    ("storytold/lightcraft", "own"),
    ("storytold/effectcraft", "own"),
    ("storytold/soundcraft", "own"),
    ("storytold/artcraft", "own"),
    ("storytold/craft-anything", "own"),
    ("storytold/craft-libs", "own"),
    ("storytold/craft-launcher", "own"),
    // Скриншоты и запись экрана.
    ("flameshot-org/flameshot", "capture"),
    ("obsproject/obs-studio", "capture"),
    ("NickeManarin/ScreenToGif", "capture"),
    // Видео, аудио и конвертация.
    ("mifi/lossless-cut", "media"),
    ("HandBrake/HandBrake", "media"),
    ("audacity/audacity", "media"),
    ("mpv-player/mpv", "media"),
    ("jeffvli/feishin", "media"),
    ("SV-stark/FileConverter-rs", "media"),
    // Графика и 3D.
    ("d2phap/ImageGlass", "graphics"),
    ("darktable-org/darktable", "graphics"),
    ("FreeCAD/FreeCAD", "graphics"),
    // Система, файлы и сеть.
    ("microsoft/PowerToys", "system"),
    ("dundee/gdu", "system"),
    ("iterate-ch/cyberduck", "system"),
    ("qbittorrent/qBittorrent", "system"),
    ("pbatard/rufus", "system"),
    ("rustdesk/rustdesk", "system"),
    // Безопасность, карты и оффлайн-базы.
    ("keepassxreboot/keepassxc", "security"),
    ("organicmaps/organicmaps", "security"),
    ("kiwix/kiwix-desktop", "security"),
    ("kiwix/kiwix-android", "security"),
    // Self-hosted серверы.
    ("passbolt/passbolt_api", "server"),
    ("immich-app/immich", "server"),
    ("owncloud/ocis", "server"),
    ("nextcloud/server", "server"),
    ("Freika/dawarich", "server"),
    ("jellyfin/jellyfin", "server"),
    ("kieraneglin/pinchflat", "server"),
    ("navidrome/navidrome", "server"),
    ("advplyr/audiobookshelf", "server"),
    ("Kareadita/Kavita", "server"),
    ("owncast/owncast", "server"),
    ("kiwix/kiwix-tools", "server"),
    ("VERT-sh/VERT", "server"),
    ("VERT-sh/vertd", "server"),
];

/// Категория по умолчанию для репозитория. Пустая строка — категории нет.
pub fn default_category(slug: &str) -> &'static str {
    DEFAULT_CATEGORIES
        .iter()
        .find(|(s, _)| *s == slug)
        .map(|(_, c)| *c)
        .unwrap_or("")
}

/// Порядок категорий в каталоге: от своих программ к «другим».
pub const CATEGORY_ORDER: &[&str] = &[
    "own", "capture", "media", "graphics", "system", "security", "server", "other",
];

/// Ключ категории репозитория: пустая категория — это «other».
pub fn category_key(category: &str) -> &str {
    let key = category.trim();
    if key.is_empty() {
        "other"
    } else {
        key
    }
}

/// Позиция категории в порядке каталога. Неизвестные категории — в конец.
pub fn category_order(key: &str) -> usize {
    CATEGORY_ORDER
        .iter()
        .position(|c| *c == key)
        .unwrap_or(CATEGORY_ORDER.len())
}

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
    #[serde(default)]
    pub enabled: bool,
    /// Релизы, которые нельзя удалять автоматически.
    #[serde(default)]
    pub pinned: Vec<String>,
    /// Какой файл релиза скачивать.
    #[serde(default)]
    pub filter: AssetKind,
    /// Дополнительно клонировать/обновлять исходники репозитория через git.
    #[serde(default)]
    pub clone_source: bool,
    /// Краткое описание (подтягивается с GitHub, можно править вручную).
    #[serde(default)]
    pub description: String,
    /// Короткое назначение программы («PDF-документы», «Векторная графика»).
    #[serde(default)]
    pub purpose: String,
    /// Категория программы — ключ группы в каталоге («own», «media»…).
    #[serde(default)]
    pub category: String,
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
            purpose: default_purpose(slug).to_string(),
            category: default_category(slug).to_string(),
            icon: None,
        }
    }

    /// Имя репозитория без владельца.
    pub fn short_name(&self) -> String {
        self.slug.rsplit('/').next().unwrap_or(&self.slug).to_string()
    }

    /// Владелец репозитория (автор).
    ///
    /// Каталог теперь сгруппирован по категориям, поэтому владелец нужен
    /// только как справочные данные.
    #[allow(dead_code)]
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
        // Назначение появляется и у уже сохранённых репозиториев.
        for repo in self.repos.iter_mut() {
            if repo.purpose.trim().is_empty() {
                repo.purpose = default_purpose(&repo.slug).to_string();
            }
            // Категория — тоже: старый конфиг получает её из таблицы.
            if repo.category.trim().is_empty() {
                repo.category = default_category(&repo.slug).to_string();
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
                    purpose: r.purpose.clone(),
                    category: r.category.clone(),
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
                    if !entry.purpose.trim().is_empty() {
                        existing.purpose = entry.purpose.clone();
                    }
                    if !entry.category.trim().is_empty() {
                        existing.category = entry.category.clone();
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
                        purpose: entry.purpose.clone(),
                        category: entry.category.clone(),
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
    pub purpose: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub filter: AssetKind,
    #[serde(default)]
    pub clone_source: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Каталог программ: у каждого репозитория из списка по умолчанию есть
    /// назначение, а повторов в списке нет.
    #[test]
    fn default_catalog_has_purposes() {
        let mut seen: Vec<&str> = Vec::new();
        for slug in DEFAULT_REPOS {
            assert!(!seen.contains(slug), "репозиторий {slug} повторяется");
            seen.push(slug);
            assert!(
                !default_purpose(slug).is_empty(),
                "у репозитория {slug} нет назначения"
            );
        }
    }

    /// Назначение программы переживает сохранение и загрузку настроек.
    #[test]
    fn purpose_survives_serde() {
        let rc = RepoConfig::new("flameshot-org/flameshot");
        let data = serde_json::to_string(&rc).expect("сериализация");
        assert!(data.contains("\"purpose\""), "в JSON нет поля purpose: {data}");
        let back: RepoConfig = serde_json::from_str(&data).expect("десериализация");
        assert_eq!(back.purpose, "Скриншоты и аннотации");
    }

    /// Старый конфиг без назначения читается и получает назначение по умолчанию.
    #[test]
    fn old_config_gets_purpose() {
        let old = r#"{"slug":"obsproject/obs-studio","enabled":true}"#;
        let mut rc: RepoConfig = serde_json::from_str(old).expect("десериализация");
        assert!(rc.purpose.is_empty());
        if rc.purpose.trim().is_empty() {
            rc.purpose = default_purpose(&rc.slug).to_string();
        }
        assert_eq!(rc.purpose, "Запись и стриминг");
    }

    /// Каталог программ: у каждого репозитория из списка по умолчанию есть
    /// и назначение, и категория, а повторов в списке нет.
    #[test]
    fn default_catalog_has_purposes_and_categories() {
        let mut seen: Vec<&str> = Vec::new();
        for slug in DEFAULT_REPOS {
            assert!(!seen.contains(slug), "репозиторий {slug} повторяется");
            seen.push(slug);
            assert!(
                !default_purpose(slug).is_empty(),
                "у репозитория {slug} нет назначения"
            );
            assert!(
                !default_category(slug).is_empty(),
                "у репозитория {slug} нет категории"
            );
            assert!(
                CATEGORY_ORDER.contains(&default_category(slug)),
                "у репозитория {slug} неизвестная категория {}",
                default_category(slug)
            );
        }
        // Категория есть у всех репозиториев каталога, лишних записей нет.
        for (slug, _) in DEFAULT_CATEGORIES {
            assert!(
                DEFAULT_REPOS.contains(slug),
                "категория задана для неизвестного репозитория {slug}"
            );
        }
        assert_eq!(DEFAULT_CATEGORIES.len(), DEFAULT_REPOS.len());
    }

    /// Категория переживает экспорт и импорт списка.
    #[test]
    fn category_survives_export_import() {
        let mut cfg = Config::default();
        cfg.repos
            .retain(|r| r.slug == "storytold/pdfcraft" || r.slug == "jellyfin/jellyfin");
        let file = cfg.export_list();
        assert_eq!(file.repos.len(), 2);

        let mut other = Config::default();
        other.repos.clear();
        let added = other.import_list(&file);
        assert_eq!(added, 2);
        let pdfcraft = other.repo("storytold/pdfcraft").expect("pdfcraft");
        assert_eq!(pdfcraft.category, "own");
        let jellyfin = other.repo("jellyfin/jellyfin").expect("jellyfin");
        assert_eq!(jellyfin.category, "server");

        // Повторный импорт перезаписывает категорию непустым значением.
        let mut patched = file.clone();
        patched.repos[0].category = "other".to_string();
        other.import_list(&patched);
        assert_eq!(
            other.repo("storytold/pdfcraft").expect("pdfcraft").category,
            "other"
        );
    }

    /// Старый конфиг без категории читается и получает категорию по умолчанию.
    #[test]
    fn old_config_gets_category() {
        let old = r#"{"slug":"obsproject/obs-studio","enabled":true}"#;
        let mut rc: RepoConfig = serde_json::from_str(old).expect("десериализация");
        assert!(rc.category.is_empty());
        if rc.category.trim().is_empty() {
            rc.category = default_category(&rc.slug).to_string();
        }
        assert_eq!(rc.category, "capture");
    }
}
