//! Кэш релизов на диске.
//!
//! GitHub отдаёт анонимным клиентам всего 60 запросов в час, поэтому список
//! релизов сохраняется рядом с настройками. При следующем запуске программа
//! показывает сохранённые данные сразу, а обновляет их в фоне — и только
//! если данные действительно устарели.

use crate::model::ReleaseInfo;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Сохранённые данные одного репозитория.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRepo {
    /// Все релизы, от свежих к старым.
    pub releases: Vec<ReleaseInfo>,
    /// Когда последний раз проверяли репозиторий (для показа в интерфейсе).
    pub last_check: Option<String>,
    /// Когда последний раз проверяли репозиторий (Unix-время) — по нему решаем,
    /// пора ли снова обращаться к GitHub.
    #[serde(default)]
    pub checked_at: Option<i64>,
}

/// Кэш релизов для всех известных репозиториев.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cache {
    pub repos: HashMap<String, CachedRepo>,
}

impl Cache {
    /// Путь к файлу кэша в папке данных приложения.
    pub fn path() -> PathBuf {
        crate::config::data_dir().join("releases-cache.json")
    }

    /// Прочитать кэш. Повреждённый файл не считаем ошибкой — начинаем заново.
    pub fn load() -> Cache {
        let path = Cache::path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Cache::default();
        };
        match serde_json::from_str::<Cache>(&text) {
            Ok(cache) => cache,
            Err(e) => {
                eprintln!("CraftPull: the release cache is broken ({e}), starting fresh");
                Cache::default()
            }
        }
    }

    /// Записать кэш на диск.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Cache::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(&path, text)
    }

    /// Запомнить релизы репозитория.
    pub fn put(
        &mut self,
        slug: &str,
        releases: Vec<ReleaseInfo>,
        last_check: Option<String>,
        checked_at: Option<i64>,
    ) {
        self.repos.insert(
            slug.to_string(),
            CachedRepo {
                releases,
                last_check,
                checked_at,
            },
        );
    }

    /// Релизы репозитория из кэша.
    pub fn get(&self, slug: &str) -> Option<&CachedRepo> {
        self.repos.get(slug)
    }

    /// Когда репозиторий проверяли последний раз (Unix-время).
    pub fn checked_at(&self, slug: &str) -> Option<i64> {
        self.get(slug).and_then(|r| r.checked_at)
    }
}
