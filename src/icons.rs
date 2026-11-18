//! Иконки репозиториев: скачивание с GitHub и локальный кэш.

use reqwest::Client;
use std::path::{Path, PathBuf};

/// Возможные пути к иконке внутри репозитория.
fn candidates(slug: &str) -> Vec<String> {
    let name = slug.rsplit('/').next().unwrap_or(slug);
    vec![
        format!("assets/app-icon/hicolor/256x256/apps/ai.storyteller.{name}.png"),
        format!("assets/app-icon/hicolor/128x128/apps/ai.storyteller.{name}.png"),
        format!("assets/app-icon/hicolor/64x64/apps/ai.storyteller.{name}.png"),
        format!("assets/app-icon/hicolor/256x256/apps/{name}.png"),
        format!("assets/icons/{name}.png"),
        format!("assets/icon.png"),
        format!("assets/app-icon.png"),
        format!("icon.png"),
    ]
}

/// Имя файла иконки в кэше.
pub fn cache_name(slug: &str) -> String {
    format!("{}.png", slug.replace('/', "__"))
}

/// Скачать иконку репозитория и положить в локальный кэш.
pub async fn ensure_icon(client: &Client, slug: &str, dir: &Path) -> Option<PathBuf> {
    let _ = std::fs::create_dir_all(dir);
    let file = dir.join(cache_name(slug));
    if file.exists() && std::fs::metadata(&file).map(|m| m.len() > 0).unwrap_or(false) {
        return Some(file);
    }
    for branch in ["main", "master"] {
        for path in candidates(slug) {
            let url = format!("https://raw.githubusercontent.com/{slug}/{branch}/{path}");
            let Ok(resp) = client.get(&url).send().await else {
                continue;
            };
            if !resp.status().is_success() {
                continue;
            }
            let Ok(bytes) = resp.bytes().await else {
                continue;
            };
            // Проверяем сигнатуру PNG, чтобы не сохранить 404-страницу.
            if !bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
                continue;
            }
            if std::fs::write(&file, &bytes).is_ok() {
                return Some(file);
            }
        }
    }
    None
}

/// Прочитать иконку и подготовить RGBA для egui.
pub fn load_rgba(path: &Path) -> Option<(Vec<u8>, [usize; 2])> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    Some((img.into_raw(), [w, h]))
}
