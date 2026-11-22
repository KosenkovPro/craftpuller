//! Источники релизов.
//!
//! Раньше CraftPull умел только GitHub. Теперь поддерживаются:
//!
//! * **GitHub Releases** — как и раньше;
//! * **GitLab Releases** — любой хост GitLab (`code.videolan.org`,
//!   `gitlab.gnome.org`);
//! * **Forgejo / Gitea Releases** — любой такой хостинг
//!   (`projects.blender.org`);
//! * **статический каталог** с автоиндексом nginx. Так раздаются готовые
//!   сборки VLC, GIMP и Blender: их репозитории не публикуют сборки как
//!   релизы (у GitLab у GIMP есть только архивы исходников, а у Blender
//!   на projects.blender.org релизы вообще выключены), поэтому файлы
//!   берутся прямо со страницы загрузок.

use crate::model::{AssetInfo, ReleaseInfo};
use reqwest::Client;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Сколько папок версий смотреть на каталоге.
const MAX_GROUPS: usize = 3;

/// Откуда брать релизы репозитория.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceSpec {
    /// GitHub Releases. Умолчание: репозиторий задаётся полем `slug`.
    Github,
    /// Проект на своём хостинге: хост и путь проекта.
    ///
    /// CraftPull сначала пробует API GitLab, а если проекта там нет —
    /// API Forgejo или Gitea: оба отдают список релизов.
    Project {
        /// Хост без схемы, например `gitlab.gnome.org`.
        host: String,
        /// Путь проекта, например `GNOME/gimp`.
        project: String,
    },
    /// Статический каталог со списком файлов.
    Index(IndexSpec),
}

impl SourceSpec {
    /// Подпись источника для интерфейса.
    pub fn label(&self) -> &'static str {
        match self {
            SourceSpec::Github => "GitHub",
            SourceSpec::Project { .. } => crate::lang::t("source.project"),
            SourceSpec::Index(_) => crate::lang::t("source.index"),
        }
    }

    /// Разобрать ссылку на проект: `https://host/путь/проекта`.
    ///
    /// Возвращает ключ репозитория и источник. Для `owner/repo` без схемы
    /// источник не меняется — это GitHub.
    pub fn detect(input: &str) -> Option<(String, Option<SourceSpec>)> {
        let text = input.trim().trim_end_matches('/');
        let rest = text.strip_prefix("https://")?;
        let (host, path) = rest.split_once('/')?;
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.len() < 2 {
            return None;
        }
        let project = parts.join("/");
        if host.eq_ignore_ascii_case("github.com") {
            return Some((parts[..2].join("/"), Some(SourceSpec::Github)));
        }
        let slug = format!("{host}/{}", parts[..2].join("/"));
        Some((
            slug,
            Some(SourceSpec::Project {
                host: host.to_string(),
                project,
            }),
        ))
    }

    /// Источник VLC: сборки лежат на странице загрузок VideoLAN.
    pub fn vlc() -> Self {
        SourceSpec::Index(IndexSpec {
            root: "https://get.videolan.org/vlc/".into(),
            group_pattern: r"^(\d+\.\d+\.\d+)$".into(),
            group_url: "https://get.videolan.org/vlc/{group}/".into(),
            subdirs: vec![
                "win64".into(),
                "win32".into(),
                "macosx".into(),
                "linux".into(),
            ],
            file_pattern: r"^vlc-(\d+\.\d+\.\d+)".into(),
            file_prefix: Some("vlc-".into()),
            hash_mode: HashMode::PerFile,
            hash_manifest: None,
            page_url: Some("https://code.videolan.org/videolan/vlc".into()),
        })
    }

    /// Источник GIMP: сборки лежат на странице загрузок GNOME.
    pub fn gimp() -> Self {
        SourceSpec::Index(IndexSpec {
            root: "https://download.gimp.org/gimp/".into(),
            group_pattern: r"^v(\d+\.\d+)$".into(),
            group_url: "https://download.gimp.org/gimp/v{group}/".into(),
            subdirs: vec!["windows".into(), "macos".into(), "linux".into()],
            file_pattern: r"^gimp-(\d+\.\d+\.\d+)".into(),
            file_prefix: Some("gimp-".into()),
            hash_mode: HashMode::Manifest,
            hash_manifest: Some("SHA256SUMS".into()),
            page_url: Some("https://gitlab.gnome.org/GNOME/gimp".into()),
        })
    }

    /// Источник Blender: сборки лежат на странице загрузок Blender.
    pub fn blender() -> Self {
        SourceSpec::Index(IndexSpec {
            root: "https://download.blender.org/release/".into(),
            group_pattern: r"^Blender(\d+\.\d+)$".into(),
            group_url: "https://download.blender.org/release/Blender{group}/".into(),
            subdirs: Vec::new(),
            file_pattern: r"^blender-(\d+\.\d+\.\d+)".into(),
            file_prefix: Some("blender-".into()),
            hash_mode: HashMode::Manifest,
            hash_manifest: Some("*.sha256".into()),
            page_url: Some("https://projects.blender.org/blender/blender".into()),
        })
    }
}

/// Как получать суммы SHA-256 для файлов каталога.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HashMode {
    /// Рядом с каждым файлом лежит свой файл с суммой — `имя.файла.sha256`.
    PerFile,
    /// В папке версии лежит общий файл со суммами всех файлов.
    Manifest,
    /// Сумм нет: проверка отключается.
    #[default]
    None,
}

/// Описание статического каталога со сборками.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexSpec {
    /// Корневой каталог, где перечислены папки версий.
    pub root: String,
    /// Регулярное выражение для имени папки версии; первая группа — ключ
    /// версии, по которому версии сравниваются между собой.
    pub group_pattern: String,
    /// Адрес папки версии. `{group}` — ключ из первой группы.
    pub group_url: String,
    /// Подкаталоги внутри папки версии, где лежат сами сборки.
    #[serde(default)]
    pub subdirs: Vec<String>,
    /// Регулярное выражение для имени файла; первая группа — версия.
    pub file_pattern: String,
    /// Оставлять только файлы, имя которых начинается с этой строки.
    #[serde(default)]
    pub file_prefix: Option<String>,
    /// Откуда берутся суммы SHA-256.
    #[serde(default)]
    pub hash_mode: HashMode,
    /// Имя файла со суммами для режима `Manifest`. Может содержать `*`.
    #[serde(default)]
    pub hash_manifest: Option<String>,
    /// Страница проекта: открывается из интерфейса.
    #[serde(default)]
    pub page_url: Option<String>,
}

/// Одна запись в каталоге.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexEntry {
    /// Имя файла или папки.
    pub name: String,
    /// Это папка, а не файл.
    pub is_dir: bool,
    /// Размер в байтах; у папок ноль.
    pub size: u64,
    /// Дата изменения в формате `ГГГГ-ММ-ДДЧЧ:ММ:СС`.
    pub modified: Option<String>,
}

/// Получить релизы репозитория из указанного источника.
pub async fn list_releases(
    client: &Client,
    slug: &str,
    source: Option<&SourceSpec>,
) -> Result<Vec<ReleaseInfo>, String> {
    match source {
        None | Some(SourceSpec::Github) => crate::github::list_releases(client, slug).await,
        Some(SourceSpec::Project { host, project }) => {
            // Сначала GitLab, потом Forgejo: проекты живут и там, и там.
            match list_gitlab_releases(client, host, project).await {
                Ok(releases) if !releases.is_empty() => Ok(releases),
                Ok(_) => list_forgejo_releases(client, host, project).await,
                Err(gitlab_err) => match list_forgejo_releases(client, host, project).await {
                    Ok(releases) if !releases.is_empty() => Ok(releases),
                    Ok(_) => Err(gitlab_err),
                    Err(forgejo_err) => Err(format!(
                        "{gitlab_err} / {forgejo_err}"
                    )),
                },
            }
        }
        Some(SourceSpec::Index(spec)) => list_index_releases(client, spec).await,
    }
}

/// Ответ GitLab Releases API.
#[derive(Debug, Deserialize)]
struct GitlabRelease {
    tag_name: String,
    name: Option<String>,
    #[serde(default)]
    upcoming_release: bool,
    released_at: Option<String>,
    #[serde(default)]
    assets: Option<GitlabAssets>,
}

#[derive(Debug, Default, Deserialize)]
struct GitlabAssets {
    #[serde(default)]
    sources: Vec<GitlabSource>,
    #[serde(default)]
    links: Vec<GitlabLink>,
}

#[derive(Debug, Deserialize)]
struct GitlabSource {
    url: String,
}

#[derive(Debug, Deserialize)]
struct GitlabLink {
    name: Option<String>,
    url: String,
    #[serde(default)]
    direct_asset_url: Option<String>,
}

/// Получить релизы проекта GitLab.
async fn list_gitlab_releases(
    client: &Client,
    host: &str,
    project: &str,
) -> Result<Vec<ReleaseInfo>, String> {
    let url = format!(
        "https://{host}/api/v4/projects/{}/releases?per_page=30",
        encode_path(project)
    );
    let raw: Vec<GitlabRelease> = get_json(client, url).await?;

    Ok(raw
        .into_iter()
        .filter(|r| !r.upcoming_release)
        .map(|r| {
            let mut assets: Vec<AssetInfo> = Vec::new();
            if let Some(a) = &r.assets {
                for l in &a.links {
                    let url = l
                        .direct_asset_url
                        .clone()
                        .unwrap_or_else(|| l.url.clone());
                    assets.push(AssetInfo {
                        name: l.name.clone().unwrap_or_else(|| file_of(&url)),
                        url,
                        size: 0,
                        digest: None,
                        synthetic: false,
                    });
                }
                for s in &a.sources {
                    assets.push(AssetInfo {
                        name: file_of(&s.url),
                        url: s.url.clone(),
                        size: 0,
                        digest: None,
                        synthetic: true,
                    });
                }
            }
            ReleaseInfo {
                tag: r.tag_name.clone(),
                name: r.name.unwrap_or_else(|| r.tag_name.clone()),
                prerelease: false,
                draft: false,
                published_at: r.released_at,
                html_url: format!("https://{host}/{project}/-/releases"),
                assets,
                source_zip: String::new(),
            }
        })
        .collect())
}

/// Ответ Forgejo (Gitea) Releases API.
#[derive(Debug, Deserialize)]
struct ForgejoRelease {
    tag_name: String,
    name: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    published_at: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<ForgejoAsset>,
}

#[derive(Debug, Deserialize)]
struct ForgejoAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// Получить релизы репозитория Forgejo или Gitea.
async fn list_forgejo_releases(
    client: &Client,
    host: &str,
    repo: &str,
) -> Result<Vec<ReleaseInfo>, String> {
    let url = format!("https://{host}/api/v1/repos/{repo}/releases?limit=30");
    let raw: Vec<ForgejoRelease> = get_json(client, url).await?;

    Ok(raw
        .into_iter()
        .filter(|r| !r.draft)
        .map(|r| ReleaseInfo {
            tag: r.tag_name.clone(),
            name: r.name.unwrap_or_else(|| r.tag_name.clone()),
            prerelease: r.prerelease,
            draft: false,
            published_at: r.published_at,
            html_url: r.html_url,
            assets: r
                .assets
                .into_iter()
                .map(|a| AssetInfo {
                    name: a.name,
                    url: a.browser_download_url,
                    size: a.size,
                    digest: a.digest,
                    synthetic: false,
                })
                .collect(),
            source_zip: String::new(),
        })
        .collect())
}

/// Получить релизы из статического каталога со сборками.
///
/// Каталог устроен как автоиндекс nginx: в корне лежат папки версий, внутри
/// — папки платформ и файлы сборок. Версия берётся из имени файла, поэтому
/// один и тот же способ работает и для VLC (`vlc-3.0.24-win64.exe`), и для
/// GIMP (`gimp-3.2.8-setup.exe`), и для Blender (`blender-4.5.0-…`).
async fn list_index_releases(
    client: &Client,
    spec: &IndexSpec,
) -> Result<Vec<ReleaseInfo>, String> {
    let group_re = Regex::new(&spec.group_pattern)
        .map_err(|e| crate::lang::tf("err.parse", &[("{e}", &e.to_string())]))?;
    let file_re = Regex::new(&spec.file_pattern)
        .map_err(|e| crate::lang::tf("err.parse", &[("{e}", &e.to_string())]))?;

    let root = fetch_index(client, &spec.root).await?;

    // Папки версий: сначала ключ версии, потом имя папки.
    let mut groups: Vec<(String, String)> = Vec::new();
    for e in &root {
        if !e.is_dir {
            continue;
        }
        let Some(cap) = group_re.captures(&e.name) else {
            continue;
        };
        let Some(m) = cap.get(1) else { continue };
        groups.push((m.as_str().to_string(), e.name.clone()));
    }
    if groups.is_empty() {
        return Err(crate::lang::t("err.index_no_versions").to_string());
    }
    groups.sort_by(|a, b| compare_versions(&b.0, &a.0));
    groups.truncate(MAX_GROUPS);

    let mut releases: Vec<ReleaseInfo> = Vec::new();
    for (key, dir) in &groups {
        let base = spec
            .group_url
            .replace("{group}", key)
            .replace("{dir}", dir);
        let mut dirs = vec![base.clone()];
        for sub in &spec.subdirs {
            dirs.push(format!("{base}{sub}/"));
        }

        // Файлы сборок и, если есть, файл с суммами.
        let mut files: Vec<(String, String, IndexEntry)> = Vec::new();
        let mut manifests: Vec<(String, String)> = Vec::new();
        for d in &dirs {
            let Ok(entries) = fetch_index(client, d).await else {
                continue;
            };
            for e in entries {
                if e.is_dir {
                    continue;
                }
                if let Some(pattern) = &spec.hash_manifest {
                    if matches_manifest(e.name.as_str(), pattern) {
                        manifests.push((format!("{d}{}", e.name), d.clone()));
                        continue;
                    }
                }
                if is_sidecar(&e.name) {
                    continue;
                }
                if let Some(prefix) = &spec.file_prefix {
                    if !e.name.starts_with(prefix.as_str()) {
                        continue;
                    }
                }
                let Some(cap) = file_re.captures(&e.name) else {
                    continue;
                };
                let Some(m) = cap.get(1) else { continue };
                files.push((m.as_str().to_string(), format!("{d}{}", e.name), e));
            }
        }

        if files.is_empty() {
            continue;
        }

        // Суммы: общий файл или отдельный файл рядом с каждым.
        let mut digests: Vec<(String, String)> = Vec::new();
        match spec.hash_mode {
            HashMode::None => {}
            HashMode::Manifest => {
                for (url, dir) in &manifests {
                    if let Ok(text) = fetch_text(client, url.clone()).await {
                        for (name, hex) in parse_sums(&text) {
                            digests.push((format!("{dir}{name}"), hex));
                        }
                    }
                }
            }
            HashMode::PerFile => {
                // Суммы тянем порциями: серверы не любят десятки запросов сразу.
                for chunk in files.chunks(6) {
                    let mut jobs = Vec::new();
                    for (_, url, _) in chunk {
                        let sum_url = format!("{url}.sha256");
                        jobs.push(fetch_text(client, sum_url));
                    }
                    let results = futures_util::future::join_all(jobs).await;
                    for (res, (_, url, _)) in results.into_iter().zip(chunk.iter()) {
                        if let Ok(text) = res {
                            if let Some(hex) = parse_hash(&text) {
                                digests.push((url.clone(), hex));
                            }
                        }
                    }
                }
            }
        }

        // Файлы группируем по версии — версия одна на сборку.
        let mut by_version: Vec<(String, Vec<AssetInfo>, Option<String>)> = Vec::new();
        for (version, url, entry) in files {
            let digest = digests
                .iter()
                .find(|(u, _)| u == &url)
                .map(|(_, hex)| format!("sha256:{hex}"));
            let asset = AssetInfo {
                name: entry.name.clone(),
                url,
                size: entry.size,
                digest,
                synthetic: false,
            };
            match by_version.iter_mut().find(|(v, _, _)| v == &version) {
                Some((_, list, when)) => {
                    list.push(asset);
                    if let Some(m) = &entry.modified {
                        if when.as_deref().is_none_or(|w| m.as_str() > w) {
                            *when = Some(m.clone());
                        }
                    }
                }
                None => by_version.push((version, vec![asset], entry.modified.clone())),
            }
        }
        by_version.sort_by(|a, b| compare_versions(&b.0, &a.0));

        for (version, assets, when) in by_version {
            releases.push(ReleaseInfo {
                tag: version.clone(),
                name: version.clone(),
                prerelease: is_prerelease(&version),
                draft: false,
                published_at: when,
                html_url: spec.page_url.clone().unwrap_or_default(),
                assets,
                source_zip: String::new(),
            });
        }
    }

    if releases.is_empty() {
        return Err(crate::lang::t("err.index_no_versions").to_string());
    }
    Ok(releases)
}

/// Скачать и разобрать страницу каталога.
async fn fetch_index(client: &Client, url: &str) -> Result<Vec<IndexEntry>, String> {
    let text = fetch_text(client, url.to_string()).await?;
    Ok(parse_index(&text))
}

/// Скачать текстовый файл.
///
/// Сайты загрузок иногда отбрасывают запросы, поэтому пробуем несколько раз.
async fn fetch_text(client: &Client, url: String) -> Result<String, String> {
    let mut last = String::new();
    for attempt in 1..=3 {
        match fetch_text_once(client, &url).await {
            Ok(text) => return Ok(text),
            Err(e) => {
                last = e;
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(300 * attempt)).await;
                }
            }
        }
    }
    Err(last)
}

/// Одна попытка скачивания текста.
async fn fetch_text_once(client: &Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| crate::lang::tf("err.network", &[("{e}", &e.to_string())]))?;
    if !resp.status().is_success() {
        return Err(crate::lang::tf(
            "err.http_status",
            &[("{status}", &resp.status().to_string()), ("{slug}", url)],
        ));
    }
    resp.text()
        .await
        .map_err(|e| crate::lang::tf("err.parse", &[("{e}", &e.to_string())]))
}

/// Скачать и разобрать JSON. Тоже с повторами.
async fn get_json<T: for<'de> Deserialize<'de>>(
    client: &Client,
    url: String,
) -> Result<T, String> {
    let mut last = String::new();
    for attempt in 1..=3 {
        match get_json_once(client, &url).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = e;
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(300 * attempt)).await;
                }
            }
        }
    }
    Err(last)
}

/// Одна попытка скачивания JSON.
async fn get_json_once<T: for<'de> Deserialize<'de>>(
    client: &Client,
    url: &str,
) -> Result<T, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| crate::lang::tf("err.network", &[("{e}", &e.to_string())]))?;
    if !resp.status().is_success() {
        return Err(crate::lang::tf(
            "err.http_status",
            &[("{status}", &resp.status().to_string()), ("{slug}", url)],
        ));
    }
    resp.json()
        .await
        .map_err(|e| crate::lang::tf("err.parse", &[("{e}", &e.to_string())]))
}

/// Разобрать страницу каталога nginx (autoindex).
///
/// Строки выглядят так:
/// ```text
/// <a href="vlc-3.0.24-win64.exe">vlc-3.0.24-win64.exe</a>  21-Sep-2026 17:04  39384682
/// ```
pub fn parse_index(html: &str) -> Vec<IndexEntry> {
    let mut out = Vec::new();
    for line in html.lines() {
        let Some(start) = line.find("<a href=\"") else {
            continue;
        };
        let rest = &line[start + 9..];
        let Some(end) = rest.find('"') else { continue };
        let name = rest[..end].to_string();
        if name.is_empty() || name == "../" || name == "./" {
            continue;
        }
        let Some(close) = rest.find("</a>") else { continue };
        let tail = &rest[close + 4..];
        let mut parts = tail.split_whitespace();
        let date = parts.next().unwrap_or("");
        let time = parts.next().unwrap_or("");
        let size = parts.next().unwrap_or("-");

        let is_dir = name.ends_with('/');
        let name = name.trim_end_matches('/').to_string();
        let size = parse_index_size(size);
        let modified = parse_index_date(date, time);

        out.push(IndexEntry {
            name,
            is_dir,
            size,
            modified,
        });
    }
    out
}

/// Размер из автоиндекса: обычное число или человекочитаемое «291M».
fn parse_index_size(raw: &str) -> u64 {
    let s = raw.trim();
    if let Ok(v) = s.parse::<u64>() {
        return v;
    }
    if s.is_empty() {
        return 0;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let Ok(value) = num.parse::<f64>() else {
        return 0;
    };
    let mult = match unit {
        "K" | "k" => 1024.0,
        "M" | "m" => 1024.0 * 1024.0,
        "G" | "g" => 1024.0 * 1024.0 * 1024.0,
        "T" | "t" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return 0,
    };
    (value * mult) as u64
}

/// Разобрать дату автоиндекса: `21-Sep-2026` и `17:04`.
fn parse_index_date(date: &str, time: &str) -> Option<String> {
    let mut it = date.split('-');
    let day: u32 = it.next()?.parse().ok()?;
    let month = match it.next()? {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    };
    let year: i32 = it.next()?.parse().ok()?;
    let time = if time.len() == 5 {
        format!("{time}:00")
    } else {
        time.to_string()
    };
    if time.is_empty() {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}T{time}"))
}

/// Сравнить версии по частям: 3.0.24 новее, чем 3.0.9.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let pa: Vec<u64> = a
        .split('.')
        .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect();
    let pb: Vec<u64> = b
        .split('.')
        .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect();
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    a.cmp(b)
}

/// Предрелизная версия: 3.2.0-RC1, 4.0-beta.
fn is_prerelease(version: &str) -> bool {
    let v = version.to_ascii_lowercase();
    ["rc", "beta", "alpha", "pre", "dev", "nightly"]
        .iter()
        .any(|m| v.contains(m))
}

/// Файл с суммой или подписью — сборой он не считается.
fn is_sidecar(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".sha256")
        || n.ends_with(".sha256sum")
        || n.ends_with(".sha1")
        || n.ends_with(".sha512")
        || n.ends_with(".md5")
        || n.ends_with(".asc")
        || n.ends_with(".sig")
        || n.ends_with(".torrent")
        || n.ends_with(".zsync")
        || n.ends_with(".blockmap")
        || n.ends_with(".minisig")
        || n.ends_with(".json")
        || n.ends_with(".txt")
        || n.ends_with(".md")
}

/// Подходит ли имя файла под шаблон файла с суммами.
///
/// В шаблоне можно использовать `*`: `*.sha256` — любой файл, кончающийся
/// на `.sha256`.
fn matches_manifest(name: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        None => name == pattern,
        Some((prefix, suffix)) => name.starts_with(prefix) && name.ends_with(suffix),
    }
}

/// Разобрать файл с суммами: строки вида `хеш  имя`.
fn parse_sums(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(hex) = it.next() else { continue };
        let Some(name) = it.next() else { continue };
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        // Иногда формат `*имя` — звезда означает двоичный режим.
        let name = name.trim_start_matches('*');
        out.push((name.to_string(), hex.to_ascii_lowercase()));
    }
    out
}

/// Взять хеш из файла с суммой одного файла.
fn parse_hash(text: &str) -> Option<String> {
    parse_sums(text)
        .into_iter()
        .next()
        .map(|(_, hex)| hex)
        .or_else(|| {
            let t = text.trim();
            if t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(t.to_ascii_lowercase())
            } else {
                None
            }
        })
}

/// Имя файла по адресу.
fn file_of(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_string()
}

/// Закодировать путь проекта для GitLab: `GNOME/gimp` → `GNOME%2Fgimp`.
fn encode_path(path: &str) -> String {
    path.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    const VLC_INDEX: &str = r#"<html>
<head><title>Index of /videolan/vlc/3.0.24/</title></head>
<body>
<h1>Index of /videolan/vlc/3.0.24/</h1><hr><pre><a href="../">../</a>
<a href="macosx/">macosx/</a>                                           18-Aug-2010 20:56                   -
<a href="vlc-3.0.24.tar.xz">vlc-3.0.24.tar.xz</a>                           21-Sep-2026 17:04           12345678
<a href="vlc-3.0.24.tar.xz.sha256">vlc-3.0.24.tar.xz.sha256</a>              21-Sep-2026 17:04                86
</pre></body></html>
"#;

    #[test]
    fn index_parsing() {
        let entries = parse_index(VLC_INDEX);
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "macosx");
        assert_eq!(entries[1].name, "vlc-3.0.24.tar.xz");
        assert_eq!(entries[1].size, 12345678);
        assert_eq!(
            entries[1].modified.as_deref(),
            Some("2026-09-21T17:04:00")
        );
        assert!(entries[2].name.ends_with(".sha256"));
    }

    #[test]
    fn source_detection() {
        let (slug, src) = SourceSpec::detect("https://github.com/mpv-player/mpv").unwrap();
        assert_eq!(slug, "mpv-player/mpv");
        assert_eq!(src, Some(SourceSpec::Github));

        let (slug, src) = SourceSpec::detect("https://gitlab.gnome.org/GNOME/gimp").unwrap();
        assert_eq!(slug, "gitlab.gnome.org/GNOME/gimp");
        assert!(matches!(src, Some(SourceSpec::Project { .. })));

        // Просто `owner/repo` — это GitHub, источник не меняется.
        assert!(SourceSpec::detect("mpv-player/mpv").is_none());
    }

    #[test]
    fn index_size_parsing() {
        assert_eq!(parse_index_size("39384682"), 39384682);
        assert_eq!(parse_index_size("291M"), 291 * 1024 * 1024);
        assert_eq!(parse_index_size("27K"), 27 * 1024);
        assert_eq!(parse_index_size("1G"), 1024 * 1024 * 1024);
        assert_eq!(parse_index_size("0"), 0);
        assert_eq!(parse_index_size("-"), 0);
    }

    #[test]
    fn version_comparison() {
        assert_eq!(compare_versions("3.0.24", "3.0.9.2"), Ordering::Greater);
        assert_eq!(compare_versions("4.5.0", "4.2.1"), Ordering::Greater);
        assert_eq!(compare_versions("3.2.8", "3.2.6"), Ordering::Greater);
        assert_eq!(compare_versions("1.0", "1.0"), Ordering::Equal);
        assert_eq!(compare_versions("2.04", "2.4"), Ordering::Less);
    }

    #[test]
    fn sums_parsing() {
        let text = "a18cd3f2863cfef80f26ebae4a468d4771737c69e7c1b2cb917f76aee41495c7  blender-4.5.0-windows-x64.msi\n\
                     0aabfc96d114632cf5787062b03c79e0e34e0af96ce20a2c2446051734f3e1a8 *blender-4.5.0-windows-arm64.zip\n";
        let sums = parse_sums(text);
        assert_eq!(sums.len(), 2);
        assert_eq!(sums[0].0, "blender-4.5.0-windows-x64.msi");
        assert_eq!(sums[0].1, "a18cd3f2863cfef80f26ebae4a468d4771737c69e7c1b2cb917f76aee41495c7");
        assert_eq!(sums[1].0, "blender-4.5.0-windows-arm64.zip");
    }

    #[test]
    fn manifest_pattern() {
        assert!(matches_manifest("blender-4.5.0.sha256", "*.sha256"));
        assert!(!matches_manifest("SHA256SUMS", "*.sha256"));
        assert!(matches_manifest("SHA256SUMS", "SHA256SUMS"));
    }

    #[test]
    fn sidecars_are_not_assets() {
        assert!(is_sidecar("vlc-3.0.24-win64.exe.sha256"));
        assert!(is_sidecar("vlc-3.0.24-win64.exe.asc"));
        assert!(!is_sidecar("vlc-3.0.24-win64.exe"));
    }

    #[test]
    fn prerelease_detection() {
        assert!(is_prerelease("3.2.0-RC1"));
        assert!(!is_prerelease("3.2.8"));
    }
}

