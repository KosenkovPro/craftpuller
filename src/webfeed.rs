//! Запасной источник данных о релизах: публичные страницы GitHub.
//!
//! REST API GitHub отдаёт анонимным клиентам всего 60 запросов в час на
//! IP-адрес. За общим адресом (NAT в офисе, VPN, мобильный интернет) лимит
//! заканчивается очень быстро, и обычный список релизов перестаёт работать.
//!
//! Публичные страницы релизов этому ограничению не подвержены, поэтому при
//! отказе API программа читает список релизов и файлы прямо с них:
//!   * `releases.atom`         — список релизов с тегами, названиями и датами;
//!   * `releases/latest`        — редирект на последний стабильный релиз;
//!   * `releases/expanded_assets/<tag>` — список файлов релиза с размерами
//!     и контрольными суммами sha256.

use crate::model::{AssetInfo, ReleaseInfo};
use reqwest::Client;

/// Сколько релизов читать из ленты (GitHub отдаёт до 10).
const MAX_RELEASES: usize = 10;

/// Запись ленты релизов.
struct FeedEntry {
    tag: String,
    title: String,
    updated: String,
}

/// Собрать список релизов через публичные страницы GitHub.
pub async fn list_releases(client: &Client, slug: &str) -> Result<Vec<ReleaseInfo>, String> {
    let feed_url = format!("https://github.com/{slug}/releases.atom");
    let feed = fetch_text_retry(client, &feed_url).await?;
    let entries = parse_feed(&feed);
    if entries.is_empty() {
        return Err(crate::lang::t("err.web_no_releases").to_string());
    }

    // Последний стабильный релиз: страница releases/latest перенаправляет на него.
    let stable_tag = latest_stable_tag(client, slug).await;

    // Файлы каждого релиза читаем параллельно — по одному запросу на тег.
    let mut assets: Vec<Vec<AssetInfo>> = Vec::new();
    for chunk in entries.chunks(4) {
        let mut batch: Vec<Vec<AssetInfo>> = Vec::new();
        for entry in chunk {
            batch.push(fetch_assets(client, slug, &entry.tag).await);
        }
        assets.extend(batch);
    }

    let mut releases: Vec<ReleaseInfo> = Vec::new();
    let mut stable_seen = false;
    for (idx, entry) in entries.iter().enumerate() {
        // Релизы в ленте отсортированы от новых к старым. Всё, что опубликовано
        // позже последнего стабильного, считаем нестабильным (pre-release/nightly).
        let is_edge = match &stable_tag {
            Some(stable) => {
                if entry.tag == *stable {
                    stable_seen = true;
                    false
                } else if !stable_seen {
                    true
                } else {
                    false
                }
            }
            None => true,
        };
        releases.push(ReleaseInfo {
            name: entry.title.clone(),
            tag: entry.tag.clone(),
            prerelease: is_edge,
            draft: false,
            published_at: Some(entry.updated.clone()),
            html_url: format!("https://github.com/{slug}/releases/tag/{}", entry.tag),
            assets: assets.get(idx).cloned().unwrap_or_default(),
            source_zip: format!("https://github.com/{slug}/archive/refs/tags/{}.zip", entry.tag),
        });
    }

    // Стабильный релиз мог не попасть в ленту (например, когда свежие
    // релизы — только pre-release). Добавим его, чтобы канал stable был полон.
    if let Some(stable) = &stable_tag {
        if !releases.iter().any(|r| &r.tag == stable) {
            let files = fetch_assets(client, slug, stable).await;
            releases.push(ReleaseInfo {
                name: stable.clone(),
                tag: stable.clone(),
                prerelease: false,
                draft: false,
                published_at: None,
                html_url: format!("https://github.com/{slug}/releases/tag/{stable}"),
                assets: files,
                source_zip: format!("https://github.com/{slug}/archive/refs/tags/{stable}.zip"),
            });
        }
    }

    Ok(releases)
}

/// Скачать страницу и вернуть текст.
async fn fetch_text(client: &Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml,application/xml")
        .send()
        .await
        .map_err(|e| crate::lang::tf("err.network", &[("{e}", &e.to_string())]))?;
    if !resp.status().is_success() {
        return Err(crate::lang::tf(
            "err.http_status_short",
            &[("{status}", &resp.status().to_string())],
        ));
    }
    resp.text()
        .await
        .map_err(|e| crate::lang::tf("err.parse", &[("{e}", &e.to_string())]))
}

/// Скачать страницу с несколькими попытками: прокси и сети любят отдавать
/// разовые ошибки, а повтор с новым соединением часто проходит.
async fn fetch_text_retry(client: &Client, url: &str) -> Result<String, String> {
    let mut last = String::new();
    for attempt in 0..3 {
        match fetch_text(client, url).await {
            Ok(text) => return Ok(text),
            Err(e) => {
                last = e;
                if attempt < 2 {
                    tokio::time::sleep(std::time::Duration::from_millis(400 * (attempt + 1))).await;
                }
            }
        }
    }
    Err(format!("{last} ({url})"))
}

/// Тег последнего стабильного релиза (редирект с releases/latest).
async fn latest_stable_tag(client: &Client, slug: &str) -> Option<String> {
    let url = format!("https://github.com/{slug}/releases/latest");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let final_url = resp.url().to_string();
    let tag = final_url.rsplit("/releases/tag/").next().unwrap_or("");
    if tag.is_empty() || tag.contains('/') {
        None
    } else {
        Some(tag.to_string())
    }
}

/// Файлы релиза со страницы expanded_assets.
async fn fetch_assets(client: &Client, slug: &str, tag: &str) -> Vec<AssetInfo> {
    let url = format!(
        "https://github.com/{slug}/releases/expanded_assets/{}",
        tag.replace(' ', "%20")
    );
    let Ok(resp) = client.get(&url).send().await else {
        return Vec::new();
    };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(html) = resp.text().await else {
        return Vec::new();
    };
    parse_assets(&html, slug)
}

/// Разобрать HTML со списком файлов.
fn parse_assets(html: &str, slug: &str) -> Vec<AssetInfo> {
    const MARKER: &str = "/releases/download/";
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find(MARKER) {
        let after = &rest[pos + MARKER.len()..];
        let end = after.find('"').unwrap_or(after.len());
        let path = &after[..end]; // {tag}/{file}
        let name = path
            .split_once('/')
            .map(|(_, n)| percent_decode(n))
            .unwrap_or_default();
        // После ссылки идут контрольная сумма и размер — читаем их из окна.
        let window_end = after[end..]
            .find(MARKER)
            .map(|i| end + i)
            .unwrap_or(after.len())
            .min(end + 4000);
        let window = &after[end..window_end];
        if !name.is_empty() {
            out.push(AssetInfo {
                name: name.clone(),
                url: format!("https://github.com/{slug}{MARKER}{path}"),
                size: find_size(window),
                digest: find_digest(window),
                synthetic: false,
            });
        }
        rest = &after[window_end..];
    }
    out
}

/// Контрольная сумма sha256, если GitHub её указал.
fn find_digest(window: &str) -> Option<String> {
    let pos = window.find("sha256:")?;
    let hex: String = window[pos + 7..]
        .chars()
        .take(64)
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.len() == 64 {
        Some(format!("sha256:{hex}"))
    } else {
        None
    }
}

/// Размер файла: GitHub отдаёт его текстом рядом с именем.
fn find_size(window: &str) -> u64 {
    let text = strip_tags(window);
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let number = &text[start..i];
            let mut j = i;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            let unit_start = j;
            while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                j += 1;
            }
            let unit = &text[unit_start..j];
            if let Some(multiplier) = unit_multiplier(unit) {
                if let Ok(value) = number.parse::<f64>() {
                    return (value * multiplier) as u64;
                }
            }
            i = j.max(i);
            continue;
        }
        i += 1;
    }
    0
}

/// Во сколько байт равен одной единице GitHub.
fn unit_multiplier(unit: &str) -> Option<f64> {
    match unit {
        "Bytes" | "Byte" => Some(1.0),
        "KB" => Some(1024.0),
        "MB" => Some(1024.0 * 1024.0),
        "GB" => Some(1024.0 * 1024.0 * 1024.0),
        "TB" => Some(1024.0 * 1024.0 * 1024.0 * 1024.0),
        _ => None,
    }
}

/// Убрать HTML-теги, чтобы остался только текст.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut depth = 0i32;
    for ch in html.chars() {
        match ch {
            '<' => depth += 1,
            '>' => {
                if depth > 0 {
                    depth -= 1;
                    out.push(' ');
                }
            }
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Разобрать RSS-ленту релизов.
fn parse_feed(xml: &str) -> Vec<FeedEntry> {
    let mut out = Vec::new();
    for chunk in xml.split("<entry>").skip(1) {
        let body = chunk.split("</entry>").next().unwrap_or("");
        // Ссылка на страницу релиза: в ней же тег.
        let link = body
            .split_once("<link rel=\"alternate\"")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once("href=\""))
            .map(|(_, href)| href.split('"').next().unwrap_or(""))
            .unwrap_or("");
        let Some(tag_raw) = link.rsplit_once("/releases/tag/").map(|(_, t)| t) else {
            continue;
        };
        let tag = percent_decode(tag_raw);
        if tag.is_empty() {
            continue;
        }
        let title = body
            .split_once("<title>")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once("</title>").map(|(t, _)| t))
            .map(html_unescape)
            .unwrap_or_else(|| tag.clone());
        let updated = body
            .split_once("<updated>")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split_once("</updated>").map(|(t, _)| t))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        out.push(FeedEntry {
            tag,
            title,
            updated,
        });
        if out.len() >= MAX_RELEASES {
            break;
        }
    }
    out
}

/// Раскрыть HTML-сущности.
fn html_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Раскрыть percent-encoding в имени файла или теге.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                None => {}
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
