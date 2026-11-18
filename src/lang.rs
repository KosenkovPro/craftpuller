//! Локализация интерфейса: английский по умолчанию, русский по выбору.
//!
//! Ключи хранятся в таблице `TABLE`, текущий язык — в атомарной переменной.
//! Форматирование с подстановками: `tf("repo.enabled_count", &[("{n}", "3")])`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

pub const EN: u8 = 0;
pub const RU: u8 = 1;

static CURRENT: AtomicU8 = AtomicU8::new(EN);

/// Установить язык интерфейса.
pub fn set(lang: u8) {
    CURRENT.store(lang, Ordering::SeqCst);
}

/// Текущий язык.
pub fn current() -> u8 {
    CURRENT.load(Ordering::SeqCst)
}

/// Перевести строку по ключу.
pub fn t(key: &str) -> &'static str {
    match table().get(key) {
        Some((en, ru)) => {
            if current() == RU {
                ru
            } else {
                en
            }
        }
        None => "",
    }
}

/// Перевести строку с подстановками вида {placeholder}.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key).to_string();
    for (k, v) in args {
        s = s.replace(k, v);
    }
    s
}

/// Название языка для выпадающего списка.
pub fn lang_name(code: u8) -> &'static str {
    match code {
        RU => "Русский",
        _ => "English",
    }
}

type Entry = (&'static str, &'static str, &'static str);

static TABLE: OnceLock<HashMap<&'static str, (&'static str, &'static str)>> = OnceLock::new();

fn table() -> &'static HashMap<&'static str, (&'static str, &'static str)> {
    TABLE.get_or_init(|| {
        const RAW: &[Entry] = &[
            // --- Общее ---
            ("app.name", "CraftPull", "CraftPull"),
            ("app.subtitle", "release manager", "менеджер релизов"),
            ("app.author", "Kosenkov Ivan", "Косенков Иван"),
            ("app.author_label", "Author", "Автор"),
            ("app.home", "Main menu", "Главное меню"),
            ("common.refresh", "Refresh", "Проверить"),
            ("common.download_new", "Download new", "Скачать новое"),
            ("common.pause", "Pause", "Пауза"),
            ("common.resume", "Resume", "Продолжить"),
            ("common.stop", "Stop", "Стоп"),
            ("common.cancel", "Cancel", "Отмена"),
            ("common.close", "Close", "Закрыть"),
            ("common.save", "Save", "Сохранить"),
            ("common.open", "Open", "Открыть"),
            ("common.pick", "Select…", "Выбрать…"),
            ("common.download", "Download", "Скачать"),
            ("common.releases", "Releases", "Релизы"),
            ("common.pin", "Pin", "Закрепить"),
            ("common.unpin", "Unpin", "Открепить"),
            ("common.delete", "Del", "Удалить"),
            ("common.enabled", "selected", "выбран"),
            ("common.disabled", "not selected", "не выбран"),
            ("common.files", "files", "файлов"),
            ("common.yes", "yes", "да"),
            ("common.no", "no", "нет"),
            // --- Навигация ---
            ("nav.repos", "Repositories", "Репозитории"),
            ("nav.settings", "Settings", "Настройки"),
            ("nav.logs", "Log", "Журнал"),
            ("nav.footer", "running in background · tray active", "работает в фоне · трей активен"),
            ("nav.version", "version", "версия"),
            // --- Статусы ---
            ("status.ready", "Ready", "Готов"),
            ("status.downloading", "Downloading", "Загрузка"),
            ("status.paused", "Paused", "Пауза"),
            ("status.total", "Total", "Всего"),
            ("status.of", "of", "из"),
            ("status.files", "{n} files", "файлов: {n}"),
            // --- Загрузки ---
            ("download.mode", "Download mode", "Режим загрузки"),
            ("download.mode_parallel", "Parallel", "Параллельно"),
            ("download.mode_sequential", "Sequential", "Последовательно"),
            ("download.mode_hint", "Parallel: several files at once, limited by the concurrency setting. Sequential: strictly one file at a time.", "Параллельно: несколько файлов сразу, в пределах лимита. Последовательно: строго один файл за раз."),
            ("downloads.title", "Downloads", "Загрузки"),
            ("downloads.empty", "Nothing is downloading right now", "Сейчас ничего не скачивается"),
            ("downloads.state_queued", "Queued", "В очереди"),
            ("downloads.state_running", "Downloading", "Качается"),
            ("downloads.state_paused", "Paused", "Пауза"),
            ("downloads.state_done", "Ready", "Готово"),
            ("downloads.state_failed", "Failed", "Ошибка"),
            ("downloads.state_cancelled", "Stopped", "Остановлено"),
            // --- Репозитории ---
            ("repo.title", "Repositories", "Репозитории"),
            ("repo.selected_count", "{n} of {m} selected", "выбрано {n} из {m}"),
            ("repo.on_disk", "On disk:", "На диске:"),
            ("repo.no_releases", "no releases found", "релизов не найдено"),
            ("repo.checked", "checked", "проверено"),
            ("repo.group", "Author", "Автор"),
            ("repo.add", "Add repository", "Добавить репозиторий"),
            ("repo.remove", "Remove", "Убрать"),
            ("repo.import", "Import JSON", "Импорт JSON"),
            ("repo.export", "Export JSON", "Экспорт JSON"),
            ("repo.import_ok", "Imported repositories: {n}", "Импортировано репозиториев: {n}"),
            ("repo.import_err", "Import failed: {e}", "Ошибка импорта: {e}"),
            ("repo.export_ok", "Repository list saved: {p}", "Список репозиториев сохранён: {p}"),
            ("repo.desc_hint", "short description", "краткое описание"),
            ("repo.desc_missing", "no description yet", "описания пока нет"),
            ("repo.filter", "Files", "Файлы"),
            // --- Фильтр файлов и карточка-герой ---
            ("filter.portable", "Portable (Windows + Ubuntu)", "Portable (Windows + Ubuntu)"),
            ("filter.all", "All release files", "Все файлы релиза"),
            ("filter.source", "Sources only", "Только исходники"),
            ("filter.portable_short", "Portable", "Portable"),
            ("filter.all_short", "All files", "Все файлы"),
            ("filter.source_short", "Sources", "Исходники"),
            ("hero.open_folder", "Folder", "Папка"),
            ("hero.hint", "Click a card to open details", "Нажмите на карточку — откроются детали"),
            ("repo.pinned_n", "pinned: {n}", "закреплено: {n}"),
            ("repo.select_all", "Select all", "Выбрать все"),
            ("repo.select_none", "Clear selection", "Снять выделение"),
            ("repo.view_list", "List", "Список"),
            ("repo.view_grid", "Grid", "Сетка"),
            ("repo.view_light", "Light", "Без картинок"),
            ("repo.icon_hint", "icon from the repository", "иконка из репозитория"),
            ("repo.slug_hint", "owner/repository", "владелец/репозиторий"),
            ("repo.add_err", "Invalid repository name", "Некорректное имя репозитория"),
            ("repo.added", "Repository added: {s}", "Репозиторий добавлен: {s}"),
            ("repo.removed", "Repository removed: {s}", "Репозиторий убран: {s}"),
            ("repo.download_started", "Download started: {s}", "Загрузка запущена: {s}"),
            ("repo.update_started", "Update started: {s}", "Обновление запущено: {s}"),
            ("repo.version_deleted", "Version {t} deleted", "Версия {t} удалена"),
            ("repo.version_kept", "Version {t} is pinned and cannot be deleted", "Версия {t} закреплена — удаление запрещено"),
            ("repo.pin_changed", "Pinned releases are never deleted automatically", "Закреплённые релизы не удаляются автоматически"),
            // --- Окно релизов ---
            ("popup.title", "Releases — {s}", "Релизы — {s}"),
            ("popup.stable", "STABLE", "СТАБИЛЬНЫЙ"),
            ("popup.edge", "UNSTABLE", "НЕСТАБИЛЬНЫЙ"),
            ("popup.assets", "Files in the release", "Файлы релиза"),
            ("popup.download_selected", "Download selected", "Скачать выбранные"),
            ("popup.download_all", "Download every file", "Скачать все файлы"),
            ("popup.selected", "selected: {n}", "выбрано: {n}"),
            ("popup.size", "size", "размер"),
            // --- Настройки ---
            ("settings.title", "Settings", "Настройки"),
            ("settings.folder", "Download folder", "Папка загрузок"),
            ("settings.folder_hint", "where releases are stored", "куда складываются релизы"),
            ("settings.network", "Network and speed", "Сеть и скорость"),
            ("settings.no_limit", "No speed limit", "Без ограничения скорости"),
            ("settings.limit", "Limit:", "Ограничение:"),
            ("settings.parallel", "Parallel downloads:", "Параллельных загрузок:"),
            ("settings.sha", "Verify SHA-256 after download", "Проверять SHA-256 после загрузки"),
            ("settings.token", "GitHub token:", "GitHub-токен:"),
            ("settings.token_hint", "optional: raises the API rate limit", "необязательно: поднимает лимит API"),
            ("settings.channels", "Release channels", "Каналы релизов"),
            ("settings.stable", "Stable releases (latest without pre-release)", "Стабильные релизы (последний без pre-release)"),
            ("settings.edge", "Unstable releases (nightly / pre-release)", "Нестабильные релизы (nightly / pre-release)"),
            ("settings.channels_hint", "If only one release exists, a single new version is downloaded. When both channels exist, both are downloaded and the older versions are removed afterwards.", "Если доступен только один релиз — скачивается одна новая версия. Если доступны оба канала — скачиваются оба, а старые версии удаляются после загрузки."),
            ("settings.behaviour", "Behaviour", "Поведение"),
            ("settings.delete_old", "Delete previous versions after downloading new ones", "Удалять предыдущие версии после загрузки новых"),
            ("settings.close_tray", "Closing the window hides it in the tray", "Закрытие окна сворачивает в трей"),
            ("settings.start_min", "Start minimized in the tray", "Запускаться свёрнутым в трей"),
            ("settings.autostart", "Start together with Windows", "Автозапуск вместе с Windows"),
            ("settings.selfupdate", "Self-update from sources", "Самообновление из исходников"),
            ("settings.self_repo", "Repository:", "Репозиторий:"),
            ("settings.self_repo_hint", "owner/repo or a path with the sources", "owner/repo или путь к папке с исходниками"),
            ("settings.self_branch", "Branch:", "Ветка:"),
            ("settings.self_now", "Update the program now", "Обновить программу сейчас"),
            ("settings.self_hint", "The update pulls the sources with git, builds them with cargo and replaces this program after it exits. Rust must be installed.", "Обновление тянет исходники через git, собирает их cargo и заменяет эту программу после выхода. Требуется установленный Rust."),
            ("settings.repos", "Repositories", "Репозитории"),
            ("settings.sources", "sources", "исходники"),
            ("settings.install", "Launch and shortcut", "Запуск и ярлык"),
            ("settings.shortcut", "Create a desktop shortcut", "Создать ярлык на рабочем столе"),
            ("settings.open_data", "Open the data folder", "Открыть папку данных"),
            ("settings.open_log", "Open the log file", "Открыть файл журнала"),
            ("settings.language", "Language", "Язык"),
            ("settings.view_mode", "View mode", "Режим отображения"),
            ("settings.autostart_ok", "Auto-start configured", "Автозапуск настроен"),
            ("settings.autostart_err", "Could not configure auto-start: {e}", "Не удалось настроить автозапуск: {e}"),
            // --- О программе ---
            ("about.title", "About the author", "Об авторе"),
            ("about.email", "Email", "Почта"),
            ("about.site", "Website", "Сайт"),
            ("about.telegram", "Telegram", "Telegram"),
            ("about.note", "Questions and suggestions — write to me", "Вопросы и предложения — пишите"),
            // --- Журнал ---
            ("log.title", "Log", "Журнал"),
            ("log.clear", "Clear", "Очистить"),
            ("log.open", "Open file", "Открыть файл"),
            ("log.empty", "The log is empty", "Журнал пуст"),
            // --- Трей ---
            ("tray.show", "Show window", "Показать окно"),
            ("tray.pause", "Pause", "Пауза"),
            ("tray.resume", "Resume", "Продолжить"),
            ("tray.stop", "Stop downloads", "Остановить загрузки"),
            ("tray.exit", "Exit", "Выход"),
            ("tray.tooltip_idle", "CraftPull — idle", "CraftPull — свободен"),
            ("tray.tooltip_working", "CraftPull — downloading", "CraftPull — идёт загрузка"),
            ("tray.tooltip_paused", "CraftPull — paused", "CraftPull — пауза"),
            ("tray.tooltip_error", "CraftPull — errors", "CraftPull — есть ошибки"),
            // --- Уведомления ---
            ("notify.update_all", "Updating all repositories", "Запущено обновление всех репозиториев"),
            ("notify.stopped", "Downloads stopped", "Загрузки остановлены"),
            ("notify.paused", "Downloads paused", "Загрузки приостановлены"),
            ("notify.resumed", "Downloads resumed", "Загрузки продолжены"),
            ("notify.folder_changed", "Download folder updated", "Папка загрузок обновлена"),
            ("notify.pin_changed", "Pinned releases are never deleted automatically", "Закреплённые релизы не удаляются автоматически"),
            ("notify.shortcut_ok", "Shortcut created: {p}", "Ярлык создан: {p}"),
            ("notify.err", "Error: {e}", "Ошибка: {e}"),
            ("notify.selfupdate_started", "Self-update started — see the log", "Запущено самообновление — смотрите журнал"),
            ("notify.saved", "Saved", "Сохранено"),
            // --- Сообщения журнала ---
            ("log.started", "CraftPull started. Download folder: {dir}", "CraftPull запущен. Папка загрузок: {dir}"),
            ("log.exit", "CraftPull finished", "CraftPull завершён"),
            ("log.checked", "{slug}: found {n} releases, check completed", "{slug}: найдено релизов {n}, последняя проверка выполнена"),
            ("log.no_repo", "No repository selected — tick the ones you need in the list", "Не выбран ни один репозиторий — отметьте нужные в списке"),
            ("log.no_releases", "{slug}: no matching releases", "{slug}: нет подходящих релизов"),
            ("log.no_assets", "{slug} {tag}: no files to download", "{slug} {tag}: нет файлов для загрузки"),
            ("log.already", "{slug} {tag}: already downloaded, skipping", "{slug} {tag}: уже скачано, пропускаю"),
            ("log.downloading", "{slug}: downloading {tag} ({size}), files: {n}", "{slug}: скачиваю {tag} ({size}), файлов: {n}"),
            ("log.ready", "{slug} {tag}: {name} — ready", "{slug} {tag}: {name} — готов"),
            ("log.deleted", "{slug}: removed old version {path}", "{slug}: удалена старая версия {path}"),
            ("log.updating_sources", "{slug}: updating sources", "{slug}: обновляю исходники"),
            ("log.stopped", "Downloads stopped by user", "Загрузки остановлены пользователем"),
            ("log.cleanup_skipped", "{slug}: {n} file(s) failed to download — the old versions were kept", "{slug}: не скачано файлов: {n} — старые версии оставлены"),
            ("log.self_pull", "Update: pulling sources {url}", "Обновление: тяну исходники {url}"),
            ("log.self_pulled", "Sources updated", "Исходники обновлены"),
            ("log.self_build", "Building the new version (cargo build --release)…", "Собираю новую версию (cargo build --release)…"),
            ("log.self_build_fail", "Build failed:\n{tail}", "Сборка не удалась:\n{tail}"),
            ("log.self_ready", "Build ready. The program restarts with the new version after exit.", "Сборка готова. Программа перезапустится с новой версией после выхода."),
            ("git.updated", "updated: {out}", "обновлён: {out}"),
            ("git.cloned", "cloned: {out}", "склонирован: {out}"),
            // --- Ошибки ---
            ("err.http_client", "could not create the HTTP client: {e}", "не удалось создать HTTP-клиент: {e}"),
            ("err.timeout", "GitHub request timed out ({slug})", "таймаут при обращении к GitHub ({slug})"),
            ("err.offline", "no connection to GitHub ({slug})", "нет связи с GitHub ({slug})"),
            ("err.network", "network error: {e}", "ошибка сети: {e}"),
            ("err.rate_limit", "GitHub API rate limit reached for {slug}. Add a token in Settings", "лимит запросов GitHub исчерпан для {slug}. Укажите токен в настройках"),
            ("err.web_no_releases", "no releases found on the public GitHub page", "на публичной странице GitHub релизы не найдены"),
            ("err.http_status", "GitHub returned {status} for {slug}", "GitHub вернул {status} для {slug}"),
            ("err.http_status_short", "GitHub returned {status}", "GitHub вернул {status}"),
            ("err.parse", "could not parse the GitHub response: {e}", "не удалось разобрать ответ GitHub: {e}"),
            ("err.mkdir", "could not create the directory: {e}", "не удалось создать каталог: {e}"),
            ("err.save_file", "could not save the file: {e}", "не удалось сохранить файл: {e}"),
            ("err.download", "download error: {e}", "ошибка загрузки: {e}"),
            ("err.open_file", "could not open the file: {e}", "не удалось открыть файл: {e}"),
            ("err.stopped", "stopped by the user", "остановлено пользователем"),
            ("err.conn_drop", "connection dropped: {e}", "обрыв соединения: {e}"),
            ("err.write", "write error: {e}", "ошибка записи: {e}"),
            ("err.save", "save error: {e}", "ошибка сохранения: {e}"),
            ("err.rename", "could not rename the file: {e}", "не удалось переименовать файл: {e}"),
            ("err.sha_mismatch", "checksum mismatch: expected {expected}, got {actual}", "контрольная сумма не совпала: ожидалось {expected}, получено {actual}"),
            ("err.sha_read", "could not read the file for verification: {e}", "не удалось прочитать файл для проверки: {e}"),
            ("err.git_missing", "git is not available: {e}", "git недоступен: {e}"),
            ("err.git_code", "git exited with code {code}", "git завершился с кодом {code}"),
            ("err.folder", "could not create the download folder {dir}: {e}", "не удалось создать папку загрузок {dir}: {e}"),
            ("err.cfg_save", "could not save the settings: {e}", "не удалось сохранить настройки: {e}"),
            ("err.cache_save", "could not save the release cache: {e}", "не удалось сохранить кэш релизов: {e}"),
            ("err.tray", "could not create the tray icon: {e}", "не удалось создать значок в трее: {e}"),
            ("err.icon_build", "could not build the image", "не удалось собрать изображение"),
            ("err.shortcut_dir", "could not create {dir}: {e}", "не удалось создать {dir}: {e}"),
            ("err.shortcut_copy", "could not copy the program: {e}", "не удалось скопировать программу: {e}"),
            ("err.shortcut_ico", "could not write the icon: {e}", "не удалось записать иконку: {e}"),
            ("err.shortcut_desktop", "could not find the desktop folder", "не удалось найти рабочий стол"),
            ("err.shortcut_ps", "could not launch PowerShell: {e}", "не удалось запустить PowerShell: {e}"),
            ("err.shortcut_ps_fail", "PowerShell returned an error: {e}", "PowerShell вернул ошибку: {e}"),
            ("err.self_no_repo", "specify a source repository in Settings", "укажите репозиторий с исходниками в настройках"),
            ("err.self_pull", "could not update the sources: {e}", "не удалось обновить исходники: {e}"),
            ("err.self_build", "the build finished with errors", "сборка завершилась с ошибками"),
            ("err.self_nobin", "the built binary was not found at {path}", "не найден собранный файл {path}"),
            ("err.self_save", "could not save the build: {e}", "не удалось сохранить сборку: {e}"),
            ("err.self_prep", "could not prepare the replacement: {e}", "не удалось подготовить замену: {e}"),
            ("err.no_cargo", "cargo not found — install Rust (rustup) to build updates", "cargo не найден — установите Rust (rustup), чтобы собирать обновления"),
            // --- Единицы измерения ---
            ("unit.gb", "{v} GB", "{v} ГБ"),
            ("unit.mb", "{v} MB", "{v} МБ"),
            ("unit.kb", "{v} KB", "{v} КБ"),
            ("unit.b", "{v} B", "{v} Б"),
            ("unit.per_sec", "{v}/s", "{v}/с"),
            ("unit.secs", "{v} s", "{v} с"),
            ("unit.mins", "{v} min {s} s", "{v} мин {s} с"),
            ("unit.hours", "{v} h {m} min", "{v} ч {m} мин"),
        ];
        RAW.iter()
            .map(|(k, en, ru)| (*k, (*en, *ru)))
            .collect()
    })
}
