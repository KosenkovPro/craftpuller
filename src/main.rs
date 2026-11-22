//! CraftPull — менеджер загрузок релизов и исходников репозиториев.
//!
//! Возможности:
//!  * следит за репозиториями и сам качает новые релизы (стабильные и ночные);
//!  * удаляет старые версии, оставляя закреплённые пользователем;
//!  * ограничивает скорость, умеет паузу, остановку и фоновую работу в трее;
//!  * обновляет сам себя: тянет исходники репозитория и пересобирает программу.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cache;
mod config;
mod download;
mod git;
mod github;
mod icon;
mod icons;
mod lang;
mod manager;
mod model;
mod selfupdate;
mod sources;
mod storage;
mod tray;
mod ui;
mod updater;
mod webfeed;

use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        attach_console();
        print_help();
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        attach_console();
        println!("CraftPull {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--install-shortcut") {
        attach_console();
        match icon::install_shortcut() {
            Ok(path) => {
                println!("Shortcut created: {}", path.display());
            }
            Err(e) => {
                eprintln!("Error: {e}");
                exit(1);
            }
        }
        return;
    }

    // Экспорт иконки в PNG (для сборок под Linux и оформления).
    if let Some(pos) = args.iter().position(|a| a == "--export-icon") {
        attach_console();
        let target = args.get(pos + 1).cloned().unwrap_or_else(|| "craftpull.png".into());
        let rgba = icon::make_rgba(256, icon::IconKind::Idle);
        match icon::write_png(std::path::Path::new(&target), 256, &rgba) {
            Ok(()) => println!("Icon saved: {target}"),
            Err(e) => {
                eprintln!("Error: {e}");
                exit(1);
            }
        }
        return;
    }

    // Обновление самой программы из командной строки: без окна.
    if args.iter().any(|a| a == "--update") {
        attach_console();
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("Error: {e}");
                exit(1);
            }
        };
        match rt.block_on(crate::updater::update_from_console()) {
            Ok(message) => println!("{message}"),
            Err(e) => {
                eprintln!("Error: {e}");
                exit(1);
            }
        }
        return;
    }

    // Отложенное обновление: подменяем файл программы и перезапускаем.
    if let Some(path) = selfupdate::apply_pending_update() {
        let _ = std::process::Command::new(&path).spawn();
        return;
    }

    if let Err(e) = run(&args) {
        attach_console();
        eprintln!("Could not start CraftPull: {e}");
        exit(1);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    // Без иконок сторонних приложений: чужие логотипы не должны попасть
    // ни в репозиторий, ни на скриншоты документации.
    if args.iter().any(|a| a == "--no-icons") {
        crate::icons::set_disabled(true);
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .map_err(|e| e.to_string())?;

    let state = manager::AppState::new(rt.handle().clone())?;

    // Сквозная проверка: опросить GitHub, скачать целевые релизы, показать результат.
    if let Some(pos) = args.iter().position(|a| a == "--selftest") {
        attach_console();
        let slug = args
            .get(pos + 1)
            .cloned()
            .unwrap_or_else(|| "storytold/pdfcraft".to_string());
        state.shared.limiter.set_limit(4 * 1024 * 1024);
        println!("CraftPull: self-test of {slug}…");
        let started = std::time::Instant::now();
        rt.block_on(async {
            let _ = state.refresh_repo(&slug).await;
            // Лимит ставим после проверки: обновление описания перечитывает его из настроек.
            state.shared.limiter.set_limit(4 * 1024 * 1024);
            state
                .download_repo(&slug, manager::DownloadRequest::Targets)
                .await;
            if let Some(p) = state.ensure_icon(&slug).await {
                println!("Icon: {}", p.display());
            }
        });
        let elapsed = started.elapsed().as_secs_f64();
        let cfg = state.cfg();
        let installed = storage::scan_installed(&cfg.root_dir, &slug);
        if installed.is_empty() {
            println!("Nothing was downloaded. See the log for details.");
            return Err("the self-test failed".to_string());
        }
        let mut bytes = 0u64;
        for v in &installed {
            bytes += v.size;
            println!(
                "{slug} {} -> {} ({} files, {})",
                v.tag,
                v.dir,
                v.files.len(),
                model::human_size(v.size)
            );
            for f in &v.files {
                println!("    {f}");
            }
        }
        println!(
            "Done in {:.1} s, average speed {} (limit 4 MB/s).",
            elapsed,
            model::human_speed(bytes as f64 / elapsed.max(0.001))
        );
        return Ok(());
    }

    // Проверка источников: что видно у каждого не-GitHub источника.
    if let Some(pos) = args.iter().position(|a| a == "--check-sources") {
        attach_console();
        println!("CraftPull: checking release sources…");
        let given = args.get(pos + 1).cloned();
        rt.block_on(async {
            // Проверяем либо указанный проект, либо все источники из каталога.
            let mut checks: Vec<(String, Option<crate::sources::SourceSpec>)> = Vec::new();
            match given {
                Some(a) => {
                    let (slug, src) =
                        crate::sources::SourceSpec::detect(&a).unwrap_or((a.clone(), None));
                    checks.push((slug, src));
                }
                None => {
                    for slug in crate::config::DEFAULT_REPOS {
                        if let Some(src) = crate::config::default_source(slug) {
                            checks.push((slug.to_string(), Some(src)));
                        }
                    }
                }
            }
            for (slug, source) in checks {
                let label = source
                    .as_ref()
                    .map(|s| s.label().to_string())
                    .unwrap_or_else(|| "GitHub".to_string());
                println!("\n=== {slug} — {label} ===");
                match crate::sources::list_releases(&state.shared.client, &slug, source.as_ref())
                    .await
                {
                    Ok(releases) => {
                        println!("  releases: {}", releases.len());
                        for r in releases.iter().take(3) {
                            println!(
                                "  {} ({})",
                                r.tag,
                                r.published_at.clone().unwrap_or_else(|| "—".into())
                            );
                            for a in &r.assets {
                                println!(
                                    "      {:<46} {:>10}  {}",
                                    a.name,
                                    model::human_size(a.size),
                                    a.digest.as_deref().unwrap_or("без суммы")
                                );
                            }
                        }
                    }
                    Err(e) => println!("  ОШИБКА: {e}"),
                }
            }
        });
        return Ok(());
    }

    let hidden = args.iter().any(|a| a == "--tray") || state.cfg().start_minimized;
    // Открыть сразу нужную вкладку / режим / окно релизов.
    let tab_arg = arg_value(args, "--tab");
    let view_arg = arg_value(args, "--view");
    let popup_arg = arg_value(args, "--popup");
    let hero_arg = arg_value(args, "--hero");
    if let Some(view) = &view_arg {
        let mut cfg = state.cfg();
        cfg.view_mode = match view.as_str() {
            "list" => crate::config::ViewMode::List,
            "light" => crate::config::ViewMode::Light,
            _ => crate::config::ViewMode::Grid,
        };
        state.save_cfg(cfg);
    }
    let tab = match tab_arg.as_deref() {
        Some("settings") => Some(ui::Tab::Settings),
        Some("logs") | Some("log") => Some(ui::Tab::Logs),
        _ => None,
    };
    let launch = ui::Launch {
        hidden,
        tab,
        popup: popup_arg.clone(),
        auto_refresh: !args.iter().any(|a| a == "--no-refresh"),
        force_refresh: args.iter().any(|a| a == "--refresh"),
        hero: hero_arg.clone(),
    };

    // Скачать конкретный репозиторий сразу при запуске.
    if let Some(slug) = arg_value(args, "--download") {
        let st = state.clone();
        let handle = st.handle.clone();
        handle.spawn(async move {
            st.download_fresh(&slug, manager::DownloadRequest::Targets)
                .await;
        });
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CraftPull")
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([960.0, 620.0])
            .with_icon(icon::window_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "CraftPull",
        options,
        Box::new(move |_cc| Ok(Box::new(ui::CraftApp::new(state.clone(), launch)))),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Значение аргумента вида `--ключ значение`.
fn arg_value(args: &[String], key: &str) -> Option<String> {
    let pos = args.iter().position(|a| a == key)?;
    args.get(pos + 1).cloned()
}

fn print_help() {
    println!(
        "CraftPull {}\n\n\
         Usage:\n  \
         craftpull                 — open the program window\n  \
         craftpull --tray          — start minimized in the tray\n  \
         craftpull --install-shortcut — create a desktop shortcut\n  \
         craftpull --export-icon <file.png> — save the 256×256 icon\n  \
         craftpull --selftest [owner/repo] — check the download pipeline without a window\n  \
         craftpull --check-sources — check every non-GitHub release source\n  \
         craftpull --update          — check for a new version of the program and install it\n  \
         craftpull --tab <repos|settings|logs> — open a specific tab\n  \
         craftpull --view <grid|list|light> — open a specific view mode\n  \
         craftpull --popup <owner/repo> — open the release list of a repository\n  \
         craftpull --hero <owner/repo> — open the grid with that card expanded\n  \
         craftpull --download <owner/repo> — start downloading that repository\n  \
         craftpull --no-icons    — do not load third-party application icons\n  \
         craftpull --no-refresh     — do not contact GitHub on start, use the cache\n  \
         craftpull --refresh        — refresh the repositories even if the cache is fresh\n  \
         craftpull --version       — show the version",
        env!("CARGO_PKG_VERSION")
    );
}

/// В release-сборке консоли нет — подключаем родительскую, чтобы работал вывод CLI.
#[cfg(windows)]
fn attach_console() {
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
        fn AllocConsole() -> i32;
    }
    unsafe {
        if AttachConsole(u32::MAX) == 0 {
            let _ = AllocConsole();
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}
