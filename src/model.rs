//! Модель данных: релизы, ассеты, состояние задач.

use serde::{Deserialize, Serialize};

/// Канал распространения сборки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    /// Стабильный релиз.
    Stable,
    /// Нестабильный: pre-release / nightly / rc.
    Edge,
}

impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Channel::Stable => crate::lang::t("popup.stable"),
            Channel::Edge => crate::lang::t("popup.edge"),
        }
    }
}

/// Один файл внутри релиза GitHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// sha256:<hex> — если GitHub его отдаёт.
    pub digest: Option<String>,
    /// Синтетический ассет: архив исходников репозитория.
    pub synthetic: bool,
}

impl AssetInfo {
    pub fn source_zip(tag: &str, url: String) -> Self {
        Self {
            name: format!("{tag}-source.zip"),
            url,
            size: 0,
            digest: None,
            synthetic: true,
        }
    }
}

/// Релиз репозитория.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag: String,
    pub name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub published_at: Option<String>,
    pub html_url: String,
    pub assets: Vec<AssetInfo>,
    pub source_zip: String,
}

impl ReleaseInfo {
    /// Сборка считается нестабильной, если GitHub помечает релиз как
    /// pre-release либо в теге есть ночные/предрелизные маркеры.
    pub fn is_edge(&self) -> bool {
        if self.prerelease {
            return true;
        }
        let tag = self.tag.to_ascii_lowercase();
        ["nightly", "dev", "alpha", "beta", "rc", "-pre", "preview"]
            .iter()
            .any(|m| tag.contains(m))
    }

    pub fn channel(&self) -> Channel {
        if self.is_edge() {
            Channel::Edge
        } else {
            Channel::Stable
        }
    }
}

/// Состояние одной загрузки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Queued,
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}

impl TaskState {
    pub fn is_finished(self) -> bool {
        matches!(self, TaskState::Done | TaskState::Failed | TaskState::Cancelled)
    }
}

/// Прогресс одной загрузки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub id: String,
    pub repo: String,
    pub tag: String,
    pub asset: String,
    pub dest: String,
    pub total: u64,
    pub downloaded: u64,
    /// Текущая скорость, байт/с.
    pub speed: f64,
    pub state: TaskState,
    pub error: Option<String>,
    /// Порядковый номер постановки в очередь: чем меньше, тем раньше.
    pub seq: u64,
    /// Момент постановки в очередь, мс с начала эпохи.
    pub started_at: u64,
    /// Место в очереди загрузки (1 — первый).
    pub queue: usize,
    /// Сколько всего задач в очереди.
    pub queue_total: usize,
}

impl TaskProgress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            if self.state == TaskState::Done {
                1.0
            } else {
                0.0
            }
        } else {
            (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0) as f32
        }
    }

    /// Сколько секунд идёт загрузка.
    pub fn elapsed_secs(&self) -> u64 {
        let now = now_ms();
        now.saturating_sub(self.started_at) / 1000
    }

    /// Осталось примерно столько секунд.
    pub fn remaining_secs(&self) -> Option<u64> {
        if self.speed <= 1.0 || self.total == 0 {
            return None;
        }
        let left = self.total.saturating_sub(self.downloaded);
        Some((left as f64 / self.speed) as u64)
    }
}

/// Текущее время, мс с начала эпохи.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Форматирование времени в формате «мм:сс» или «ч:мм:сс».
pub fn human_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Уже скачанная версия, найденная на диске.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledVersion {
    pub tag: String,
    pub dir: String,
    pub size: u64,
    pub files: Vec<String>,
    pub channel: Option<String>,
    pub installed_at: Option<String>,
}

/// Уровень записи в журнал.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub time: String,
    pub level: LogLevel,
    pub text: String,
}

/// Форматирование даты релиза: RFC3339 → «12 Nov 2026, 02:18».
pub fn human_date(raw: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(raw) {
        Ok(dt) => {
            let local = dt.with_timezone(&chrono::Local);
            local.format("%d %b %Y, %H:%M").to_string()
        }
        Err(_) => raw.to_string(),
    }
}

/// Форматирование размера для интерфейса.
pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        crate::lang::tf("unit.gb", &[("{v}", &format!("{:.2}", b / GB))])
    } else if b >= MB {
        crate::lang::tf("unit.mb", &[("{v}", &format!("{:.1}", b / MB))])
    } else if b >= KB {
        crate::lang::tf("unit.kb", &[("{v}", &format!("{:.0}", b / KB))])
    } else {
        crate::lang::tf("unit.b", &[("{v}", &b.to_string())])
    }
}

/// Форматирование скорости.
pub fn human_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec <= 0.0 {
        "—".to_string()
    } else {
        let size = human_size(bytes_per_sec as u64);
        crate::lang::tf("unit.per_sec", &[("{v}", &size)])
    }
}
