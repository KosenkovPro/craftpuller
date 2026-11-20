//! Загрузка файлов: лимит скорости, пауза с докачкой, отмена, проверка SHA-256.

use crate::model::{AssetInfo, TaskProgress, TaskState};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Notify};

/// Реестр активных задач: id -> прогресс.
pub type TaskRegistry = Arc<std::sync::Mutex<HashMap<String, TaskProgress>>>;

/// Глобальные команды пользователя: пауза, остановка.
#[derive(Debug, Default)]
pub struct Control {
    pub paused: AtomicBool,
    pub cancel: AtomicBool,
    /// Сигнал, который будит загрузчики в момент отмены: сеть может молчать,
    /// и ждать следующего чанка нельзя — «Стоп» должен срабатывать сразу.
    notify: Notify,
}

impl Control {
    pub fn toggle(&self) -> bool {
        let now = !self.paused.load(Ordering::SeqCst);
        self.paused.store(now, Ordering::SeqCst);
        now
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
    pub fn reset(&self) {
        self.cancel.store(false, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Ждёт отмены. Возвращается немедленно, если отмена уже запрошена.
    pub async fn wait_cancelled(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            // Регистрируем ожидание до проверки флага — иначе отмена,
            // случившаяся между проверкой и ожиданием, пропала бы.
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Классический token-bucket: ограничение общей скорости по всем загрузкам.
pub struct RateLimiter {
    /// Байт в секунду; 0 — без ограничения.
    limit: AtomicU64,
    bucket: Mutex<Bucket>,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            limit: AtomicU64::new(0),
            bucket: Mutex::new(Bucket {
                tokens: 0.0,
                last: Instant::now(),
            }),
        }
    }
}

impl RateLimiter {
    pub fn set_limit(&self, bytes_per_sec: u64) {
        self.limit.store(bytes_per_sec, Ordering::SeqCst);
    }

    pub fn limit(&self) -> u64 {
        self.limit.load(Ordering::SeqCst)
    }

    /// Забрать из квоты до `want` байт. Если квоты не хватает — ждёт и
    /// возвращает сколько удалось получить сейчас.
    pub async fn acquire(&self, want: u64) -> u64 {
        let limit = self.limit();
        if limit == 0 || want == 0 {
            return want;
        }
        let mut b = self.bucket.lock().await;
        let now = Instant::now();
        let dt = now.duration_since(b.last).as_secs_f64();
        b.last = now;
        b.tokens = (b.tokens + dt * limit as f64).min(limit as f64);
        let grant = b.tokens.min(want as f64).floor() as u64;
        b.tokens -= grant as f64;
        drop(b);

        if grant < want {
            let deficit = (want - grant) as f64;
            let wait = (deficit / limit as f64).clamp(0.02, 2.0);
            tokio::time::sleep(Duration::from_secs_f64(wait)).await;
        }
        grant
    }
}

/// Загрузчик одного ассета.
pub struct Downloader {
    pub client: Client,
    pub limiter: Arc<RateLimiter>,
    pub control: Arc<Control>,
    pub tasks: TaskRegistry,
    pub verify_sha256: bool,
    /// Порядковый номер задачи в очереди загрузки.
    pub seq: u64,
}

impl Downloader {
    /// Скачать ассет в каталог `dest_dir`.
    /// Возвращает Ok(()) при успехе, Err(текст) при ошибке или отмене.
    pub async fn fetch(
        &self,
        id: &str,
        repo: &str,
        tag: &str,
        asset: &AssetInfo,
        dest_dir: &Path,
    ) -> Result<(), String> {
        if let Err(e) = tokio::fs::create_dir_all(dest_dir).await {
            return Err(crate::lang::tf("err.mkdir", &[("{e}", &e.to_string())]));
        }

        let dest = dest_dir.join(&asset.name);
        let part = dest_dir.join(format!("{}.part", asset.name));

        // Докачка: если .part уже есть — продолжаем с его размера.
        let mut downloaded = match tokio::fs::metadata(&part).await {
            Ok(meta) => meta.len(),
            Err(_) => 0,
        };
        if downloaded > 0 && downloaded >= asset.size && asset.size > 0 {
            // Файл уже скачан целиком.
            if let Err(e) = tokio::fs::rename(&part, &dest).await {
                return Err(crate::lang::tf("err.save_file", &[("{e}", &e.to_string())]));
            }
            self.finish(id, TaskState::Done, downloaded, asset.size, None);
            return Ok(());
        }

        let mut req = self.client.get(&asset.url);
        if downloaded > 0 {
            req = req.header("Range", format!("bytes={downloaded}-"));
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                let msg = crate::lang::tf("err.download", &[("{e}", &e.to_string())]);
                self.finish(id, TaskState::Failed, downloaded, asset.size, Some(msg.clone()));
                return Err(msg);
            }
        };

        let status = resp.status();
        let mut append = false;
        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            append = true;
        } else if status.is_success() {
            // Сервер не поддержал Range — начинаем заново.
            downloaded = 0;
        } else {
            let msg = format!("HTTP {status}");
            self.finish(id, TaskState::Failed, downloaded, asset.size, Some(msg.clone()));
            return Err(msg);
        }

        let total = resp
            .content_length()
            .map(|c| c + downloaded)
            .unwrap_or(asset.size.max(downloaded));

        let file = if append {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(&part)
                .await
        } else {
            tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&part)
                .await
        };
        let mut file = match file {
            Ok(f) => f,
            Err(e) => {
                let msg = crate::lang::tf("err.open_file", &[("{e}", &e.to_string())]);
                self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
                return Err(msg);
            }
        };

        self.update(id, repo, tag, asset, &dest, total, downloaded, 0.0, TaskState::Running, None);

        let mut stream = resp.bytes_stream();
        let mut last_report = Instant::now();
        let mut last_bytes = downloaded;
        let mut speed = 0.0f64;

        loop {
            // Пауза: перестаём читать поток, пока пользователь не продолжит.
            while self.control.is_paused() {
                if self.control.is_cancelled() {
                    self.finish(id, TaskState::Cancelled, downloaded, total, None);
                    return Err(crate::lang::t("err.stopped").to_string());
                }
                self.update(
                    id,
                    repo,
                    tag,
                    asset,
                    &dest,
                    total,
                    downloaded,
                    speed,
                    TaskState::Paused,
                    None,
                );
                tokio::time::sleep(Duration::from_millis(120)).await;
            }

            // Останавливаемся мгновенно: ждать следующего чанка из сети нельзя,
            // соединение может молчать минутами. Ветка отмены выигрывает у чтения.
            let chunk = tokio::select! {
                _ = self.control.wait_cancelled() => {
                    self.finish(id, TaskState::Cancelled, downloaded, total, None);
                    return Err(crate::lang::t("err.stopped").to_string());
                }
                chunk = stream.next() => match chunk {
                    Some(c) => c,
                    None => break,
                },
            };

            if self.control.is_cancelled() {
                self.finish(id, TaskState::Cancelled, downloaded, total, None);
                return Err(crate::lang::t("err.stopped").to_string());
            }

            let chunk: Bytes = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let msg = crate::lang::tf("err.conn_drop", &[("{e}", &e.to_string())]);
                    self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
                    return Err(msg);
                }
            };

            let mut off = 0usize;
            while off < chunk.len() {
                let want = (chunk.len() - off).min(32 * 1024) as u64;
                // Ограничение скорости тоже умеет ждать — отмена прерывает и его.
                let granted = tokio::select! {
                    _ = self.control.wait_cancelled() => {
                        self.finish(id, TaskState::Cancelled, downloaded, total, None);
                        return Err(crate::lang::t("err.stopped").to_string());
                    }
                    g = self.limiter.acquire(want) => g,
                };
                if granted == 0 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
                let granted = granted.min(want) as usize;
                if let Err(e) = file.write_all(&chunk[off..off + granted]).await {
                    let msg = crate::lang::tf("err.write", &[("{e}", &e.to_string())]);
                    self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
                    return Err(msg);
                }
                off += granted;
                downloaded += granted as u64;
            }

            let now = Instant::now();
            if now.duration_since(last_report) >= Duration::from_millis(400) {
                let dt = now.duration_since(last_report).as_secs_f64().max(0.001);
                let instant = (downloaded - last_bytes) as f64 / dt;
                // Скорость сглаживаем: мгновенная скачет, и оценка остатка мигает.
                speed = if speed <= 0.0 {
                    instant
                } else {
                    speed * 0.6 + instant * 0.4
                };
                last_report = now;
                last_bytes = downloaded;
                self.update(
                    id,
                    repo,
                    tag,
                    asset,
                    &dest,
                    total,
                    downloaded,
                    speed,
                    TaskState::Running,
                    None,
                );
            }
        }
        if let Err(e) = file.flush().await {
            let msg = crate::lang::tf("err.save", &[("{e}", &e.to_string())]);
            self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
            return Err(msg);
        }
        drop(file);

        if let Err(e) = tokio::fs::rename(&part, &dest).await {
            let msg = crate::lang::tf("err.rename", &[("{e}", &e.to_string())]);
            self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
            return Err(msg);
        }

        // Проверка контрольной суммы, если GitHub её отдаёт.
        if self.verify_sha256 {
            if let Some(digest) = asset.digest.as_deref() {
                if let Some(expected) = digest.strip_prefix("sha256:") {
                    match sha256_file(&dest).await {
                        Ok(actual) => {
                            if !actual.eq_ignore_ascii_case(expected) {
                                let _ = tokio::fs::remove_file(&dest).await;
                                let msg = crate::lang::tf(
                                    "err.sha_mismatch",
                                    &[
                                        ("{expected}", &expected[..expected.len().min(16)]),
                                        ("{actual}", &actual),
                                    ],
                                );
                                self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
                                return Err(msg);
                            }
                        }
                        Err(e) => {
                            let msg =
                                crate::lang::tf("err.sha_read", &[("{e}", &e.to_string())]);
                            self.finish(id, TaskState::Failed, downloaded, total, Some(msg.clone()));
                            return Err(msg);
                        }
                    }
                }
            }
        }

        self.finish(id, TaskState::Done, downloaded, total, None);
        Ok(())
    }

    fn update(
        &self,
        id: &str,
        repo: &str,
        tag: &str,
        asset: &AssetInfo,
        dest: &Path,
        total: u64,
        downloaded: u64,
        speed: f64,
        state: TaskState,
        error: Option<String>,
    ) {
        if let Ok(mut reg) = self.tasks.lock() {
            let entry = reg.entry(id.to_string()).or_insert_with(|| TaskProgress {
                id: id.to_string(),
                repo: repo.to_string(),
                tag: tag.to_string(),
                asset: asset.name.clone(),
                dest: dest.to_string_lossy().to_string(),
                total,
                downloaded,
                speed,
                state,
                error: None,
                seq: self.seq,
                started_at: crate::model::now_ms(),
                queue: 0,
                queue_total: 0,
            });
            entry.total = total;
            entry.downloaded = downloaded;
            entry.speed = speed;
            entry.state = state;
            entry.error = error;
        }
    }

    fn finish(&self, id: &str, state: TaskState, downloaded: u64, total: u64, error: Option<String>) {
        if let Ok(mut reg) = self.tasks.lock() {
            if let Some(entry) = reg.get_mut(id) {
                entry.downloaded = downloaded;
                entry.total = total;
                entry.state = state;
                entry.error = error;
                entry.speed = 0.0;
            }
        }
    }
}

/// SHA-256 файла.
pub async fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("{e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Уникальный идентификатор задачи.
pub fn task_id(repo: &str, tag: &str, asset: &str) -> String {
    format!("{repo}::{tag}::{asset}")
}
