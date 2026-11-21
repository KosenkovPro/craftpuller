//! Обновление программы по релизным бинарникам GitHub.
//!
//! Схема работы: спрашиваем у GitHub последний релиз, выбираем файл под
//! текущую систему, качаем его с проверкой SHA-256, распаковываем
//! исполняемый файл и заменяем им текущую программу.

use crate::config::{data_dir, Config};
use crate::lang::{t, tf};
use crate::manager::AppState;
use crate::model::LogLevel;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Репозиторий с релизными бинарниками по умолчанию.
pub const DEFAULT_REPO: &str = "KosenkovPro/craftpuller";

/// Адрес API GitHub.
const API: &str = "https://api.github.com";

/// Имя файла программы в архиве релиза (Windows).
const EXE_WINDOWS: &str = "craftpull.exe";

/// Имя файла программы в архиве релиза (Unix).
const EXE_UNIX: &str = "craftpull";

/// Текущая версия программы.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Операционная система, для которой выбирается файл релиза.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Linux,
    MacOs,
}

impl TargetOs {
    /// Система, на которой запущена программа.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// Файл релиза.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    /// Имя файла, например `craftpull-0.4.0-windows-x64.zip`.
    pub name: String,
    /// Адрес файла для скачивания.
    pub browser_download_url: String,
    /// Размер файла в байтах.
    pub size: u64,
    /// Контрольная сумма от GitHub: `sha256:<hex>`.
    #[serde(default)]
    pub digest: Option<String>,
}

impl ReleaseAsset {
    /// Ожидаемая сумма SHA-256 из поля `digest`.
    ///
    /// Пустая строка — GitHub не отдал сумму, проверять нечего.
    pub fn sha256(&self) -> Option<String> {
        let raw = self.digest.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        let hex = raw.strip_prefix("sha256:").unwrap_or(raw).trim();
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(hex.to_ascii_lowercase())
    }
}

/// Ответ API о последнем релизе.
#[derive(Debug, Deserialize)]
struct GhLatestRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

/// Последний релиз: тег и список файлов.
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// Тег релиза, например `v0.4.0`.
    pub tag: String,
    /// Файлы релиза.
    pub assets: Vec<ReleaseAsset>,
}

/// Разобрать версию в числа: «v1.10.0» → `[1, 10, 0]`.
pub fn parse_version(text: &str) -> Vec<u64> {
    let s = text.trim();
    let s = s
        .strip_prefix('v')
        .or_else(|| s.strip_prefix('V'))
        .unwrap_or(s);
    // Суффиксы предрелиза и сборки отбрасываем: «1.2.0-rc1+abc» → «1.2.0».
    let core = s.split(['-', '+']).next().unwrap_or(s);
    core.split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

/// Новее ли релиз, чем установленная версия.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let a = parse_version(candidate);
    let b = parse_version(current);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Выбрать файл релиза под нужную систему.
///
/// Windows — zip с «windows» в имени, Linux — tar.gz с «linux» и
/// «x86_64», macOS — zip с «macos» (в приоритете universal).
pub fn pick_asset(assets: &[ReleaseAsset], os: TargetOs) -> Option<&ReleaseAsset> {
    let candidates: Vec<&ReleaseAsset> = match os {
        TargetOs::Windows => assets
            .iter()
            .filter(|a| {
                let n = a.name.to_ascii_lowercase();
                n.contains("windows") && n.ends_with(".zip")
            })
            .collect(),
        TargetOs::Linux => assets
            .iter()
            .filter(|a| {
                let n = a.name.to_ascii_lowercase();
                n.contains("linux") && n.contains("x86_64") && !n.ends_with(".deb")
            })
            .collect(),
        TargetOs::MacOs => assets
            .iter()
            .filter(|a| a.name.to_ascii_lowercase().contains("macos"))
            .collect(),
    };
    // Приоритет: tar.gz для Linux, universal для macOS.
    candidates.into_iter().min_by_key(|a| {
        let n = a.name.to_ascii_lowercase();
        match os {
            TargetOs::Linux => u8::from(!n.ends_with(".tar.gz")),
            TargetOs::MacOs => u8::from(!n.contains("universal")),
            TargetOs::Windows => 0,
        }
    })
}

/// Вид архива релиза.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    TarGz,
}

impl ArchiveKind {
    /// Вид архива по имени файла.
    pub fn by_name(name: &str) -> Option<Self> {
        let n = name.to_ascii_lowercase();
        if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if n.ends_with(".zip") {
            Some(Self::Zip)
        } else {
            None
        }
    }
}

/// Имя файла без пути.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Похоже ли имя на исполняемый файл.
fn looks_executable(path: &str) -> bool {
    let base = file_name(path);
    match base.rsplit_once('.') {
        Some((_, ext)) => ext.eq_ignore_ascii_case("exe"),
        None => true,
    }
}

/// Выбрать исполняемый файл архива.
///
/// Сначала ищем по имени (`craftpull.exe`, `craftpull`), а если такого
/// нет — берём единственный исполняемый файл архива.
pub fn choose_executable(entries: &[(String, bool)], wanted: &str) -> Option<usize> {
    if let Some(i) = entries
        .iter()
        .position(|(name, is_dir)| !*is_dir && file_name(name).eq_ignore_ascii_case(wanted))
    {
        return Some(i);
    }
    let executables: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, (name, is_dir))| !*is_dir && looks_executable(name))
        .map(|(i, _)| i)
        .collect();
    if executables.len() == 1 {
        Some(executables[0])
    } else {
        None
    }
}

/// Имя исполняемого файла внутри архива релиза.
fn exe_name(os: TargetOs) -> &'static str {
    match os {
        TargetOs::Windows => EXE_WINDOWS,
        TargetOs::Linux | TargetOs::MacOs => EXE_UNIX,
    }
}

/// Сверить посчитанную сумму с ожидаемой.
fn check_digest(actual: &str, expected: &str) -> Result<(), String> {
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(tf(
            "err.sha_mismatch",
            &[("{expected}", expected), ("{actual}", actual)],
        ))
    }
}

/// Сверить сумму файла и удалить его при несовпадении.
fn digest_or_fail(path: &Path, actual: &str, expected: &str) -> Result<(), String> {
    match check_digest(actual, expected) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(path);
            Err(e)
        }
    }
}

/// Записать содержимое записи архива в файл.
fn write_entry<R: std::io::Read>(reader: &mut R, dest: &Path) -> Result<PathBuf, String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| tf("err.mkdir", &[("{e}", &e.to_string())]))?;
    }
    let mut out =
        std::fs::File::create(dest).map_err(|e| tf("err.write", &[("{e}", &e.to_string())]))?;
    std::io::copy(reader, &mut out).map_err(|e| tf("err.write", &[("{e}", &e.to_string())]))?;
    Ok(dest.to_path_buf())
}

/// Достать из zip-архива исполняемый файл программы.
fn extract_zip(archive: &Path, os: TargetOs, dest: &Path) -> Result<PathBuf, String> {
    let file = std::fs::File::open(archive)
        .map_err(|e| tf("err.open_file", &[("{e}", &e.to_string())]))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
    // Список имён: нужный файл ищем по имени, а не по первому попавшемуся.
    let mut entries: Vec<(String, bool)> = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
        entries.push((entry.name().to_string(), entry.is_dir()));
    }
    let index = choose_executable(&entries, exe_name(os))
        .ok_or_else(|| t("err.update_no_exe").to_string())?;
    let mut entry = zip
        .by_index(index)
        .map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
    write_entry(&mut entry, dest)
}

/// Достать из tar.gz-архива исполняемый файл программы.
fn extract_targz(archive: &Path, os: TargetOs, dest: &Path) -> Result<PathBuf, String> {
    // Первый проход: собираем имена файлов архива.
    let mut entries: Vec<(String, bool)> = Vec::new();
    {
        let file = std::fs::File::open(archive)
            .map_err(|e| tf("err.open_file", &[("{e}", &e.to_string())]))?;
        let decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
        let mut tar = tar::Archive::new(decoder);
        let iter = tar
            .entries()
            .map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
        for entry in iter {
            let entry = entry.map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
            let path = entry
                .path()
                .map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?
                .to_string_lossy()
                .to_string();
            // Флаг — это «это каталог», как и в zip: так список сравним.
            entries.push((
                path,
                entry.header().entry_type() == tar::EntryType::Directory,
            ));
        }
    }
    let index = choose_executable(&entries, exe_name(os))
        .ok_or_else(|| t("err.update_no_exe").to_string())?;
    // Второй проход: достаём нужный файл.
    let file = std::fs::File::open(archive)
        .map_err(|e| tf("err.open_file", &[("{e}", &e.to_string())]))?;
    let decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    let mut tar = tar::Archive::new(decoder);
    let iter = tar
        .entries()
        .map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
    for (i, entry) in iter.enumerate() {
        let mut entry = entry.map_err(|e| tf("err.update_unpack", &[("{e}", &e.to_string())]))?;
        if i == index {
            return write_entry(&mut entry, dest);
        }
    }
    Err(t("err.update_no_exe").to_string())
}

/// Достать из архива релиза исполняемый файл программы.
pub fn extract_executable(
    archive: &Path,
    kind: ArchiveKind,
    os: TargetOs,
    dest: &Path,
) -> Result<PathBuf, String> {
    match kind {
        ArchiveKind::Zip => extract_zip(archive, os, dest),
        ArchiveKind::TarGz => extract_targz(archive, os, dest),
    }
}

/// Путь рядом с файлом с добавленным окончанием.
///
/// `craftpull.exe` → `craftpull.exe.new`.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// Выдать файлу права на запуск.
#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

/// Заменить программу на новую и перезапустить её (Unix).
///
/// Новый бинарник кладём рядом с текущим, старый убираем в сторону,
/// на его место ставим новый и запускаем.
#[cfg(unix)]
fn replace_unix(current: &Path, new_bin: &Path) -> Result<(), String> {
    let new_path = with_suffix(current, ".new");
    let old_path = with_suffix(current, ".old");
    let _ = std::fs::remove_file(&new_path);
    std::fs::copy(new_bin, &new_path)
        .map_err(|e| tf("err.update_replace", &[("{e}", &e.to_string())]))?;
    std::fs::rename(current, &old_path)
        .map_err(|e| tf("err.update_replace", &[("{e}", &e.to_string())]))?;
    if let Err(e) = std::fs::rename(&new_path, current) {
        // Не получилось — возвращаем старую программу на место.
        let _ = std::fs::rename(&old_path, current);
        return Err(tf("err.update_replace", &[("{e}", &e.to_string())]));
    }
    set_executable(current);
    let _ = std::fs::remove_file(&old_path);
    // Новая копия запускается с теми же аргументами, что и текущая.
    std::process::Command::new(current)
        .args(std::env::args().skip(1))
        .spawn()
        .map_err(|e| tf("err.update_replace", &[("{e}", &e.to_string())]))?;
    Ok(())
}

/// Заменить программу на новую и перезапустить её (Windows).
///
/// Запущенный файл перезаписать нельзя, поэтому заменой занимается
/// вспомогательный скрипт: он ждёт конца программы и запускает новую.
#[cfg(windows)]
fn replace_windows(current: &Path, new_bin: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    let staged = with_suffix(current, ".new");
    let _ = std::fs::remove_file(&staged);
    std::fs::copy(new_bin, &staged)
        .map_err(|e| tf("err.update_replace", &[("{e}", &e.to_string())]))?;
    let script = write_helper_script(current, &staged, std::process::id())?;
    // Скрипт ждёт конца программы — окно консоли мигать не должно.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/c", &script.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| tf("err.update_replace", &[("{e}", &e.to_string())]))?;
    Ok(())
}

/// Написать вспомогательный скрипт замены программы.
///
/// Скрипт ждёт, пока закончится процесс с номером `pid`, убирает старую
/// программу в сторону, ставит на её место новую и запускает.
#[cfg(windows)]
fn write_helper_script(current: &Path, staged: &Path, pid: u32) -> Result<PathBuf, String> {
    let target = current.display().to_string();
    let new = staged.display().to_string();
    let old = with_suffix(current, ".old").display().to_string();
    let script = format!(
        "@echo off\r\n\
rem CraftPull: zamen programmy na novuyu versiyu.\r\n\
setlocal\r\n\
set \"TARGET={target}\"\r\n\
set \"NEW={new}\"\r\n\
set \"OLD={old}\"\r\n\
:wait\r\n\
tasklist /FI \"PID eq {pid}\" /NH | find /I \"No tasks\" >nul\r\n\
if not errorlevel 1 goto replace\r\n\
ping -n 2 127.0.0.1 >nul\r\n\
goto wait\r\n\
:replace\r\n\
if exist \"%OLD%\" del /F /Q \"%OLD%\"\r\n\
move /Y \"%TARGET%\" \"%OLD%\" >nul 2>&1\r\n\
if exist \"%TARGET%\" goto fail\r\n\
move /Y \"%NEW%\" \"%TARGET%\" >nul 2>&1\r\n\
if not exist \"%TARGET%\" goto fail\r\n\
start \"\" \"%TARGET%\"\r\n\
start \"\" /B cmd /C del /F /Q \"%~f0\"\r\n\
exit /b 0\r\n\
:fail\r\n\
if exist \"%OLD%\" move /Y \"%OLD%\" \"%TARGET%\" >nul 2>&1\r\n\
start \"\" /B cmd /C del /F /Q \"%~f0\"\r\n\
exit /b 1\r\n"
    );
    let path = std::env::temp_dir().join(format!("craftpull-update-{pid}.cmd"));
    std::fs::write(&path, script)
        .map_err(|e| tf("err.update_replace", &[("{e}", &e.to_string())]))?;
    Ok(path)
}

/// Заменить текущую программу новым бинарником и перезапустить её.
pub fn replace_current_exe(new_bin: &Path) -> Result<(), String> {
    let current = std::env::current_exe()
        .map_err(|e| tf("err.update_current", &[("{e}", &e.to_string())]))?;
    #[cfg(windows)]
    {
        replace_windows(&current, new_bin)
    }
    #[cfg(unix)]
    {
        replace_unix(&current, new_bin)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (current, new_bin);
        Err(t("err.update_unsupported").to_string())
    }
}

/// Репозиторий с релизами программы: из настроек или по умолчанию.
pub fn update_repo(cfg: &Config) -> String {
    let raw = cfg.self_update_repo.trim();
    if raw.is_empty() {
        return DEFAULT_REPO.to_string();
    }
    // Ссылку на репозиторий приводим к виду owner/repo.
    if let Some(rest) = raw.split("github.com/").nth(1) {
        let slug: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.' || *c == '/')
            .collect();
        let slug = slug.trim_end_matches('/');
        if slug.contains('/') {
            return slug.to_string();
        }
    }
    raw.to_string()
}

/// Клиент для API GitHub с нужными заголовками.
fn build_client(token: &str) -> Result<Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&user_agent())
            .map_err(|e| tf("err.http_client", &[("{e}", &e.to_string())]))?,
    );
    if !token.trim().is_empty() {
        let value = format!("Bearer {}", token.trim());
        if let Ok(v) = HeaderValue::from_str(&value) {
            headers.insert(AUTHORIZATION, v);
        }
    }
    Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| tf("err.http_client", &[("{e}", &e.to_string())]))
}

/// User-Agent вида «CraftPull/0.4» — major.minor текущей версии.
fn user_agent() -> String {
    let short = current_version()
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");
    format!("CraftPull/{short}")
}

/// Запросить последний релиз репозитория.
pub async fn fetch_latest_release(client: &Client, repo: &str) -> Result<ReleaseInfo, String> {
    let url = format!("{API}/repos/{repo}/releases/latest");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| tf("err.network", &[("{e}", &e.to_string())]))?;
    let status = resp.status();
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return Err(tf("err.rate_limit", &[("{slug}", repo)]));
    }
    if !status.is_success() {
        return Err(tf(
            "err.http_status_short",
            &[("{status}", &status.to_string())],
        ));
    }
    let raw: GhLatestRelease = resp
        .json()
        .await
        .map_err(|e| tf("err.parse", &[("{e}", &e.to_string())]))?;
    Ok(ReleaseInfo {
        tag: raw.tag_name,
        assets: raw.assets,
    })
}

/// Скачать файл релиза и сверить его SHA-256.
///
/// Возвращает путь к файлу и признак того, что сумма была проверена:
/// если GitHub не отдал сумму, файл принимается как есть.
pub async fn download_asset(
    client: &Client,
    asset: &ReleaseAsset,
    dest_dir: &Path,
) -> Result<(PathBuf, bool), String> {
    std::fs::create_dir_all(dest_dir).map_err(|e| tf("err.mkdir", &[("{e}", &e.to_string())]))?;
    let dest = dest_dir.join(&asset.name);
    let resp = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| tf("err.download", &[("{e}", &e.to_string())]))?;
    if !resp.status().is_success() {
        return Err(tf(
            "err.http_status_short",
            &[("{status}", &resp.status().to_string())],
        ));
    }
    let mut file = tokio::fs::File::create(&dest)
        .await
        .map_err(|e| tf("err.write", &[("{e}", &e.to_string())]))?;
    // Считаем SHA-256 на лету — файл не перечитываем.
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| tf("err.download", &[("{e}", &e.to_string())]))?;
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| tf("err.write", &[("{e}", &e.to_string())]))?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| tf("err.write", &[("{e}", &e.to_string())]))?;
    drop(file);

    let actual = format!("{:x}", hasher.finalize());
    match asset.sha256() {
        Some(expected) => {
            digest_or_fail(&dest, &actual, &expected)?;
            Ok((dest, true))
        }
        // GitHub не отдал сумму — принимаем файл, но это отмечаем в журнале.
        None => Ok((dest, false)),
    }
}

/// Состояние проверки обновлений программы.
#[derive(Default, Clone)]
pub struct UpdateState {
    /// Идёт ли фоновая проверка.
    pub checking: bool,
    /// Идёт ли загрузка и замена программы.
    pub updating: bool,
    /// Есть ли новая версия.
    pub available: bool,
    /// Тег новой версии.
    pub tag: String,
    /// Файл релиза под текущую систему.
    pub asset: Option<ReleaseAsset>,
    /// Строка состояния для интерфейса.
    pub status: String,
    /// Уведомление, которое надо показать в окне.
    pub toast: Option<(String, LogLevel)>,
}

static STATE: OnceLock<Mutex<UpdateState>> = OnceLock::new();

fn state() -> &'static Mutex<UpdateState> {
    STATE.get_or_init(|| Mutex::new(UpdateState::default()))
}

/// Снимок состояния обновления.
pub fn snapshot() -> UpdateState {
    state().lock().map(|s| s.clone()).unwrap_or_default()
}

/// Изменить состояние обновления.
pub fn mutate(f: impl FnOnce(&mut UpdateState)) {
    if let Ok(mut s) = state().lock() {
        f(&mut s);
    }
}

/// Забрать отложенное уведомление для окна.
pub fn take_toast() -> Option<(String, LogLevel)> {
    let mut s = state().lock().ok()?;
    s.toast.take()
}

/// Проверить обновления в фоне.
pub fn spawn_check(state: Arc<AppState>) {
    let handle = state.handle.clone();
    handle.spawn(async move {
        if let Err(e) = check_for_update(state.clone()).await {
            state.shared.log(LogLevel::Error, e);
        }
    });
}

/// Скачать и установить обновление в фоне.
pub fn spawn_update(state: Arc<AppState>) {
    let handle = state.handle.clone();
    handle.spawn(async move {
        if let Err(e) = perform_update(state.clone()).await {
            state.shared.log(LogLevel::Error, e);
            mutate(|s| {
                s.updating = false;
                s.status = t("err.update_failed").to_string();
            });
        }
    });
}

/// Проверить, есть ли новая версия программы.
pub async fn check_for_update(state: Arc<AppState>) -> Result<(), String> {
    let cfg = state.cfg();
    let repo = update_repo(&cfg);
    let version = current_version();
    mutate(|s| {
        s.checking = true;
        s.status = t("log.update_check").to_string();
    });
    state
        .shared
        .log(LogLevel::Info, t("log.update_check").to_string());

    let client = build_client(&cfg.github_token)?;
    let release = fetch_latest_release(&client, &repo)
        .await
        .map_err(|e| tf("err.update_repo", &[("{e}", &e)]))?;
    if !is_newer(&release.tag, version) {
        let msg = tf("log.update_latest", &[("{v}", version)]);
        mutate(|s| {
            s.checking = false;
            s.available = false;
            s.tag = release.tag.clone();
            s.status = msg.clone();
        });
        state.shared.log(LogLevel::Ok, msg);
        return Ok(());
    }

    let os = TargetOs::current();
    let Some(asset) = pick_asset(&release.assets, os).cloned() else {
        let msg = tf("err.update_no_asset", &[("{tag}", &release.tag)]);
        mutate(|s| {
            s.checking = false;
            s.status = msg.clone();
        });
        state.shared.log(LogLevel::Error, msg.clone());
        return Err(msg);
    };

    let msg = tf("log.update_available", &[("{tag}", &release.tag)]);
    mutate(|s| {
        s.checking = false;
        s.available = true;
        s.tag = release.tag.clone();
        s.asset = Some(asset);
        s.status = msg.clone();
        s.toast = Some((
            tf("notify.update_available", &[("{tag}", &release.tag)]),
            LogLevel::Info,
        ));
    });
    state.shared.log(LogLevel::Ok, msg);
    Ok(())
}

/// Скачать новую версию, проверить сумму, заменить программу и перезапустить.
pub async fn perform_update(state: Arc<AppState>) -> Result<(), String> {
    let (tag, asset) = {
        let s = snapshot();
        match s.asset.clone() {
            Some(asset) => (s.tag.clone(), asset),
            None => return Err(t("err.update_no_asset_short").to_string()),
        }
    };
    mutate(|s| {
        s.updating = true;
        s.status = t("log.update_start").to_string();
    });
    state
        .shared
        .log(LogLevel::Info, tf("log.update_start", &[("{tag}", &tag)]));

    let cfg = state.cfg();
    let client = build_client(&cfg.github_token)?;
    let dir = data_dir().join("update").join(&tag);
    state.shared.log(
        LogLevel::Info,
        tf(
            "log.update_download",
            &[
                ("{tag}", &tag),
                ("{name}", &asset.name),
                ("{size}", &crate::model::human_size(asset.size)),
            ],
        ),
    );
    let (archive, checked) = download_asset(&client, &asset, &dir).await?;
    if checked {
        state.shared.log(
            LogLevel::Ok,
            tf(
                "log.update_sha_ok",
                &[("{tag}", &tag), ("{name}", &asset.name)],
            ),
        );
    } else {
        state.shared.log(
            LogLevel::Warn,
            tf(
                "log.update_no_digest",
                &[("{tag}", &tag), ("{name}", &asset.name)],
            ),
        );
    }

    let kind = ArchiveKind::by_name(&asset.name)
        .ok_or_else(|| tf("err.update_archive", &[("{name}", &asset.name)]))?;
    let os = TargetOs::current();
    let exe = dir.join(if os == TargetOs::Windows {
        EXE_WINDOWS
    } else {
        EXE_UNIX
    });
    let exe = extract_executable(&archive, kind, os, &exe)?;
    state.shared.log(
        LogLevel::Info,
        tf(
            "log.update_unpacked",
            &[("{tag}", &tag), ("{path}", &exe.display().to_string())],
        ),
    );

    replace_current_exe(&exe)?;
    state
        .shared
        .log(LogLevel::Ok, tf("log.update_restart", &[("{tag}", &tag)]));
    mutate(|s| s.updating = false);
    // Программа закрывается: на Windows её перезапустит вспомогательный
    // скрипт, на Unix — новый бинарник, который мы только что запустили.
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Файл релиза для проверок выбора.
    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_string(),
            browser_download_url: String::new(),
            size: 0,
            digest: None,
        }
    }

    /// Версии сравниваются по частям, а не как строки.
    #[test]
    fn version_comparison_uses_numbers() {
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("v0.4.0", "0.4.0"));
        assert!(is_newer("0.5.0", "0.4.10"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.9", "0.4.0"));
        assert_eq!(parse_version("v1.10.0"), vec![1, 10, 0]);
        assert_eq!(parse_version("0.4"), vec![0, 4]);
    }

    /// Для Windows выбирается zip с «windows» в имени.
    #[test]
    fn windows_asset_is_chosen() {
        let assets = vec![
            asset("craftpull-0.4.0-linux-x86_64.tar.gz"),
            asset("craftpull-0.4.0-windows-x64.zip"),
            asset("craftpull-0.4.0-macos-universal.zip"),
        ];
        let picked = pick_asset(&assets, TargetOs::Windows).expect("файл для Windows");
        assert_eq!(picked.name, "craftpull-0.4.0-windows-x64.zip");
    }

    /// Для Linux выбирается tar.gz, а не deb.
    #[test]
    fn linux_asset_prefers_tar_gz() {
        let assets = vec![
            asset("craftpull-0.4.0-linux-x86_64.deb"),
            asset("craftpull-0.4.0-linux-x86_64.tar.gz"),
        ];
        let picked = pick_asset(&assets, TargetOs::Linux).expect("файл для Linux");
        assert_eq!(picked.name, "craftpull-0.4.0-linux-x86_64.tar.gz");
    }

    /// Для macOS выбирается universal-сборка.
    #[test]
    fn macos_asset_prefers_universal() {
        let assets = vec![
            asset("craftpull-0.4.0-macos-x64.zip"),
            asset("craftpull-0.4.0-macos-universal.zip"),
        ];
        let picked = pick_asset(&assets, TargetOs::MacOs).expect("файл для macOS");
        assert_eq!(picked.name, "craftpull-0.4.0-macos-universal.zip");
    }

    /// Если файла под систему нет — выбор пустой, а не случайный.
    #[test]
    fn asset_for_another_system_is_not_chosen() {
        let assets = vec![asset("craftpull-0.4.0-linux-x86_64.tar.gz")];
        assert!(pick_asset(&assets, TargetOs::Windows).is_none());
        assert!(pick_asset(&assets, TargetOs::MacOs).is_none());
    }

    /// Исполняемый файл ищется по имени, а не по первому попавшемуся.
    #[test]
    fn executable_is_chosen_by_name() {
        let entries = vec![
            ("README.md".to_string(), false),
            ("craftpull.png".to_string(), false),
            ("craftpull.exe".to_string(), false),
        ];
        assert_eq!(choose_executable(&entries, "craftpull.exe"), Some(2));
    }

    /// Если имени нет — берётся единственный исполняемый файл архива.
    #[test]
    fn single_executable_is_taken_as_fallback() {
        let entries = vec![
            ("README.md".to_string(), false),
            ("craftpull.png".to_string(), false),
            ("craftpull-new".to_string(), false),
        ];
        assert_eq!(choose_executable(&entries, "craftpull.exe"), Some(2));
    }

    /// Если исполняемых файлов несколько — выбора нет.
    #[test]
    fn ambiguous_archive_has_no_executable() {
        let entries = vec![
            ("craftpull-a.exe".to_string(), false),
            ("craftpull-b.exe".to_string(), false),
        ];
        assert_eq!(choose_executable(&entries, "craftpull.exe"), None);
    }

    /// Из zip достаётся исполняемый файл.
    #[test]
    fn zip_executable_is_extracted() {
        let dir = tempfile::tempdir().expect("временная папка");
        let archive = dir.path().join("craftpull-0.4.0-windows-x64.zip");
        {
            let file = std::fs::File::create(&archive).expect("файл архива");
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("README.md", options).expect("запись");
            zip.write_all(b"readme").expect("запись");
            zip.start_file("craftpull.exe", options).expect("запись");
            zip.write_all(b"MZ fake binary").expect("запись");
            zip.finish().expect("готово");
        }
        let dest = dir.path().join("craftpull.exe");
        let exe = extract_executable(&archive, ArchiveKind::Zip, TargetOs::Windows, &dest)
            .expect("распаковка");
        assert_eq!(std::fs::read(&exe).expect("чтение"), b"MZ fake binary");
    }

    /// Из tar.gz достаётся исполняемый файл.
    #[test]
    fn targz_executable_is_extracted() {
        let dir = tempfile::tempdir().expect("временная папка");
        let archive = dir.path().join("craftpull-0.4.0-linux-x86_64.tar.gz");
        {
            let file = std::fs::File::create(&archive).expect("файл архива");
            let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut builder = tar::Builder::new(encoder);
            let data = b"#!/bin/sh\necho craftpull\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(
                    &mut header,
                    "craftpull-0.4.0-linux-x86_64/craftpull",
                    &data[..],
                )
                .expect("запись");
            // Поток gzip надо закрыть, иначе архив останется неполным.
            let encoder = builder.into_inner().expect("готово");
            encoder.finish().expect("сжатие");
        }
        let dest = dir.path().join("craftpull");
        let exe = extract_executable(&archive, ArchiveKind::TarGz, TargetOs::Linux, &dest)
            .expect("распаковка");
        assert!(std::fs::read(&exe)
            .expect("чтение")
            .starts_with(b"#!/bin/sh"));
    }

    /// Контрольная сумма: совпадение проходит, несовпадение — ошибка.
    #[test]
    fn digest_is_checked() {
        let dir = tempfile::tempdir().expect("временная папка");
        let path = dir.path().join("craftpull.exe");
        std::fs::write(&path, b"craftpull").expect("запись");
        let sum = {
            let mut hasher = Sha256::new();
            hasher.update(b"craftpull");
            format!("{:x}", hasher.finalize())
        };
        // Совпадение: всё в порядке, файл на месте.
        assert!(digest_or_fail(&path, &sum, &sum).is_ok());
        assert!(path.exists());
        // Несовпадение: ошибка, файл удаляется.
        let wrong = "0".repeat(64);
        assert!(digest_or_fail(&path, &sum, &wrong).is_err());
        assert!(!path.exists());
    }

    /// Сумма из поля digest читается в нужном формате.
    #[test]
    fn digest_is_read_from_api_field() {
        let mut a = asset("craftpull-0.4.0-windows-x64.zip");
        assert!(a.sha256().is_none());
        a.digest = Some(format!("sha256:{}", "a".repeat(64)));
        assert_eq!(a.sha256(), Some("a".repeat(64)));
        a.digest = Some(String::new());
        assert!(a.sha256().is_none());
    }

    /// Вспомогательный скрипт на Windows меняет файлы и запускает новую программу.
    #[cfg(windows)]
    #[test]
    fn helper_script_replaces_the_program() {
        // Настоящие файлы программы не нужны, но скрипт в конце запускает новую
        // программу — берём безобидные системные файлы.
        let system = std::env::var("SystemRoot").expect("папка Windows");
        let old_source = std::path::Path::new(&system)
            .join("System32")
            .join("where.exe");
        let new_source = std::path::Path::new(&system)
            .join("System32")
            .join("whoami.exe");
        let dir = tempfile::tempdir().expect("временная папка");
        let current = dir.path().join("craftpull.exe");
        let staged = dir.path().join("craftpull.exe.new");
        std::fs::copy(&old_source, &current).expect("копия старой программы");
        std::fs::copy(&new_source, &staged).expect("копия новой программы");
        let old_size = std::fs::metadata(&current).expect("размер").len();
        let new_size = std::fs::metadata(&staged).expect("размер").len();
        // Номер процесса, которого точно нет: скрипт сразу переходит к замене.
        let script = write_helper_script(&current, &staged, 0x7FFF_FFFE).expect("скрипт");
        let output = std::process::Command::new("cmd")
            .args(["/c", script.to_string_lossy().as_ref()])
            .output()
            .expect("запуск скрипта");
        assert!(
            output.status.success(),
            "скрипт не отработал: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // На месте новой должна стоять новая программа, старая — в .old.
        assert_eq!(std::fs::metadata(&current).expect("размер").len(), new_size);
        assert_eq!(
            std::fs::metadata(dir.path().join("craftpull.exe.old"))
                .expect("размер")
                .len(),
            old_size
        );
    }
}
