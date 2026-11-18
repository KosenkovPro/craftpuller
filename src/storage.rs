//! Работа с версиями на диске: каталоги, сканирование, очистка старых версий.

use crate::model::InstalledVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Имя каталога репозитория (без владельца).
pub fn repo_name(slug: &str) -> String {
    slug.rsplit('/').next().unwrap_or(slug).to_string()
}

/// Каталог репозитория внутри корня загрузок.
pub fn repo_dir(root: &Path, slug: &str) -> PathBuf {
    root.join(repo_name(slug))
}

/// Каталог конкретной версии.
pub fn version_dir(root: &Path, slug: &str, tag: &str) -> PathBuf {
    repo_dir(root, slug).join(sanitize(tag))
}

/// Приводит имя тега к безопасному виду для имени каталога.
pub fn sanitize(tag: &str) -> String {
    tag.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Метаданные версии внутри каталога.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    pub tag: String,
    pub channel: String,
    pub installed_at: String,
}

/// Сохранить метаданные версии.
pub fn write_meta(dir: &Path, meta: &VersionMeta) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(meta).unwrap_or_default();
    std::fs::write(dir.join("meta.json"), text)
}

/// Прочитать метаданные версии.
pub fn read_meta(dir: &Path) -> Option<VersionMeta> {
    let text = std::fs::read_to_string(dir.join("meta.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Размер каталога в байтах.
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += dir_size(&path);
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

/// Найти уже скачанные версии репозитория.
pub fn scan_installed(root: &Path, slug: &str) -> Vec<InstalledVersion> {
    let mut out = Vec::new();
    let dir = repo_dir(root, slug);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta = read_meta(&path);
        let tag = meta
            .as_ref()
            .map(|m| m.tag.clone())
            .unwrap_or_else(|| path.file_name().unwrap_or_default().to_string_lossy().to_string());
        let mut files = Vec::new();
        if let Ok(inner) = std::fs::read_dir(&path) {
            for f in inner.flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                // meta.json — служебный файл, а не содержимое релиза.
                if name != "meta.json" {
                    files.push(name);
                }
            }
        }
        files.sort();
        // Каталог без файлов — это неудачная загрузка, а не установленная версия.
        if files.is_empty() {
            continue;
        }
        let channel = meta.as_ref().map(|m| m.channel.clone());
        let installed_at = meta.as_ref().map(|m| m.installed_at.clone());
        out.push(InstalledVersion {
            tag,
            dir: path.to_string_lossy().to_string(),
            size: dir_size(&path),
            files,
            channel,
            installed_at,
        });
    }
    out.sort_by(|a, b| b.tag.cmp(&a.tag));
    out
}

/// Удалить все версии репозитория, кроме `keep` и закреплённых `pinned`.
/// Возвращает список удалённых каталогов.
pub fn cleanup_versions(
    root: &Path,
    slug: &str,
    keep: &HashSet<String>,
    pinned: &[String],
) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    let dir = repo_dir(root, slug);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return removed;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let meta = read_meta(&path);
        let tag = meta
            .as_ref()
            .map(|m| m.tag.clone())
            .unwrap_or_else(|| name.clone());
        if keep.contains(&tag) || keep.contains(&name) {
            continue;
        }
        if pinned.iter().any(|p| p == &tag || p == &name) {
            continue;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
            removed.push(path);
        }
    }
    removed
}

/// Удалить конкретную версию (если она не закреплена).
pub fn remove_version(root: &Path, slug: &str, tag: &str, pinned: &[String]) -> bool {
    if pinned.iter().any(|p| p == tag) {
        return false;
    }
    let dir = version_dir(root, slug, tag);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).is_ok()
    } else {
        false
    }
}

/// Каталог для исходников репозитория (git-клон).
pub fn source_dir(root: &Path, slug: &str) -> PathBuf {
    root.join("_sources").join(repo_name(slug))
}
