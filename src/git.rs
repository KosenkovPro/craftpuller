//! Работа с git: клонирование и обновление исходников репозитория.

use std::path::Path;
use std::process::{Command, Stdio};

/// Клонировать или обновить репозиторий в `dir`.
pub fn sync_repo(repo_url: &str, dir: &Path, branch: &str) -> Result<String, String> {
    if dir.join(".git").exists() {
        let out = run_git(dir, &["pull", "--ff-only", "origin", branch])?;
        Ok(crate::lang::tf("git.updated", &[("{out}", &out)]))
    } else {
        if let Some(parent) = dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut args: Vec<&str> = vec!["clone", "--depth", "1"];
        if !branch.is_empty() {
            args.push("--branch");
            args.push(branch);
        }
        args.push(repo_url);
        let dir_str = dir.to_string_lossy().to_string();
        args.push(&dir_str);
        let out = run_git(Path::new("."), &args)?;
        Ok(crate::lang::tf("git.cloned", &[("{out}", &out)]))
    }
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| crate::lang::tf("err.git_missing", &[("{e}", &e.to_string())]))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(if stdout.is_empty() { stderr } else { stdout })
    } else {
        Err(if stderr.is_empty() {
            crate::lang::tf("err.git_code", &[("{code}", &output.status.to_string())])
        } else {
            stderr
        })
    }
}

/// URL репозитория из slug.
pub fn repo_url(slug: &str) -> String {
    format!("https://github.com/{slug}.git")
}
