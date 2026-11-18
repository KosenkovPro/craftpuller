//! Самообновление: тянем исходники репозитория, собираем и заменяем программу.

use crate::config::data_dir;
use crate::manager::AppState;
use crate::model::LogLevel;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Файл с отложенной заменой программы.
fn pending_bin() -> PathBuf {
    data_dir().join("pending_update.bin")
}

fn pending_json() -> PathBuf {
    data_dir().join("pending_update.json")
}

/// Применить отложенное обновление (вызывается при старте программы).
pub fn apply_pending_update() -> Option<PathBuf> {
    let bin = pending_bin();
    let json = pending_json();
    if !bin.exists() || !json.exists() {
        return None;
    }
    let text = std::fs::read_to_string(&json).ok()?;
    let target = text.trim().trim_matches('"').to_string();
    if target.is_empty() {
        let _ = std::fs::remove_file(&bin);
        let _ = std::fs::remove_file(&json);
        return None;
    }
    let target_path = PathBuf::from(&target);
    if std::fs::copy(&bin, &target_path).is_ok() {
        let _ = std::fs::remove_file(&bin);
        let _ = std::fs::remove_file(&json);
        return Some(target_path);
    }
    None
}

/// Полный цикл самообновления: git pull → cargo build → подготовка замены.
pub async fn run_self_update(state: Arc<AppState>) -> Result<String, String> {
    let cfg = state.cfg();
    let repo = cfg.self_update_repo.trim().to_string();
    if repo.is_empty() {
        return Err(crate::lang::t("err.self_no_repo").to_string());
    }

    // 1. Исходники.
    let src_dir = data_dir().join("self-source");
    let url = if repo.starts_with("http://") || repo.starts_with("https://") {
        repo.clone()
    } else if Path::new(&repo).exists() {
        Path::new(&repo)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&repo))
            .to_string_lossy()
            .to_string()
    } else {
        format!("https://github.com/{repo}.git")
    };

    state.shared.log(
        LogLevel::Info,
        crate::lang::tf("log.self_pull", &[("{url}", &url)]),
    );
    let branch = if cfg.self_update_branch.trim().is_empty() {
        "main".to_string()
    } else {
        cfg.self_update_branch.trim().to_string()
    };
    let src_c = src_dir.clone();
    let url_c = url.clone();
    let branch_c = branch.clone();
    let pull = tokio::task::spawn_blocking(move || {
        crate::git::sync_repo(&url_c, &src_c, &branch_c)
    })
    .await
    .map_err(|e| format!("{e}"))?;
    pull.map_err(|e| crate::lang::tf("err.self_pull", &[("{e}", &e)]))?;
    state.shared.log(LogLevel::Ok, crate::lang::t("log.self_pulled").to_string());

    // 2. Сборка.
    let cargo = cargo_path()?;
    state.shared.log(
        LogLevel::Info,
        crate::lang::t("log.self_build").to_string(),
    );
    let src_build = src_dir.clone();
    let out = tokio::task::spawn_blocking(move || {
        let output = Command::new(&cargo)
            .args(["build", "--release"])
            .current_dir(&src_build)
            .output()
            .map_err(|e| e.to_string())?;
        Ok::<String, String>(String::from_utf8_lossy(&output.stderr).to_string())
    })
    .await
    .map_err(|e| format!("{e}"))?;

    match out {
        Ok(stderr) => {
            if stderr.contains("error") || stderr.contains("error[") {
                let tail: String = stderr.lines().rev().take(12).collect::<Vec<_>>().join("\n");
                state.shared.log(
                    LogLevel::Error,
                    crate::lang::tf("log.self_build_fail", &[("{tail}", &tail)]),
                );
                return Err(crate::lang::t("err.self_build").to_string());
            }
        }
        Err(e) => return Err(e),
    }

    // 3. Подготовка замены.
    let built = src_dir.join("target").join("release").join("craftpull.exe");
    if !built.exists() {
        return Err(crate::lang::tf(
            "err.self_nobin",
            &[("{path}", &built.display().to_string())],
        ));
    }
    let current = std::env::current_exe().map_err(|e| format!("{e}"))?;
    let _ = std::fs::create_dir_all(data_dir());
    std::fs::copy(&built, pending_bin())
        .map_err(|e| crate::lang::tf("err.self_save", &[("{e}", &e.to_string())]))?;
    std::fs::write(pending_json(), current.to_string_lossy().as_bytes())
        .map_err(|e| crate::lang::tf("err.self_prep", &[("{e}", &e.to_string())]))?;

    Ok(crate::lang::t("log.self_ready").to_string())
}

/// Найти cargo: сначала в PATH, затем в стандартных местах.
fn cargo_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("CARGO") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }
    let candidates = [
        PathBuf::from("cargo"),
        dirs::home_dir()
            .unwrap_or_default()
            .join(".cargo")
            .join("bin")
            .join("cargo.exe"),
    ];
    for c in candidates {
        let ok = if c.is_absolute() {
            c.exists()
        } else {
            Command::new(&c)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if ok {
            return Ok(c);
        }
    }
    Err(crate::lang::t("err.no_cargo").to_string())
}

/// Включить или выключить автозапуск с Windows.
pub fn set_autostart(enabled: bool, exe: &Path) -> Result<(), String> {
    let key = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    let name = "CraftPull";
    let value = format!("\"{}\" --tray", exe.display());
    let mut cmd = Command::new("reg");
    if enabled {
        cmd.args(["add", key, "/v", name, "/t", "REG_SZ", "/d", &value, "/f"]);
    } else {
        cmd.args(["delete", key, "/v", name, "/f"]);
    }
    let output = cmd.output().map_err(|e| format!("{e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Проверить, включён ли автозапуск.
pub fn autostart_enabled(_exe: &Path) -> bool {
    let key = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    match Command::new("reg")
        .args(["query", key, "/v", "CraftPull"])
        .output()
    {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}
