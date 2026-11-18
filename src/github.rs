//! Клиент GitHub API: список релизов и разбор ассетов.

use crate::model::{AssetInfo, ReleaseInfo};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::Client;
use serde::Deserialize;

const API: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    prerelease: bool,
    draft: bool,
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
    zipball_url: String,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// Собрать HTTP-клиент. Токен нужен, чтобы поднять лимит запросов API.
pub fn build_client(token: &str) -> Result<Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("CraftPull/0.1"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
    if !token.trim().is_empty() {
        let value = format!("Bearer {}", token.trim());
        if let Ok(v) = HeaderValue::from_str(&value) {
            headers.insert(AUTHORIZATION, v);
        }
    }
    Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(20))
        .user_agent("CraftPull/0.1")
        .build()
        .map_err(|e| crate::lang::tf("err.http_client", &[("{e}", &e.to_string())]))
}

/// Ошибка обращения к REST API.
struct ApiError {
    message: String,
    /// Исчерпан лимит запросов — есть смысл идти запасным путём.
    rate_limited: bool,
}

/// Получить релизы репозитория, от свежих к старым.
///
/// Сначала пробуем REST API. Если лимит запросов исчерпан (у анонимных
/// клиентов это 60 запросов в час на IP-адрес), читаем те же данные с
/// публичных страниц GitHub — там ограничений нет.
pub async fn list_releases(client: &Client, slug: &str) -> Result<Vec<ReleaseInfo>, String> {
    match list_releases_api(client, slug).await {
        Ok(releases) => Ok(releases),
        Err(e) if e.rate_limited => {
            crate::webfeed::list_releases(client, slug)
                .await
                .map_err(|fallback| {
                    format!("{fallback} ({})", e.message)
                })
        }
        Err(e) => Err(e.message),
    }
}

/// Получить релизы через REST API.
async fn list_releases_api(client: &Client, slug: &str) -> Result<Vec<ReleaseInfo>, ApiError> {
    let url = format!("{API}/repos/{slug}/releases?per_page=40");
    let resp = client.get(&url).send().await.map_err(|e| {
        let message = if e.is_timeout() {
            crate::lang::tf("err.timeout", &[("{slug}", slug)])
        } else if e.is_connect() {
            crate::lang::tf("err.offline", &[("{slug}", slug)])
        } else {
            crate::lang::tf("err.network", &[("{e}", &e.to_string())])
        };
        ApiError {
            message,
            rate_limited: false,
        }
    })?;

    let status = resp.status();
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let remaining = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("0");
        if remaining == "0" {
            return Err(ApiError {
                message: crate::lang::tf("err.rate_limit", &[("{slug}", slug)]),
                rate_limited: true,
            });
        }
    }
    if !status.is_success() {
        return Err(ApiError {
            message: crate::lang::tf(
                "err.http_status",
                &[("{status}", &status.to_string()), ("{slug}", slug)],
            ),
            rate_limited: false,
        });
    }

    let raw: Vec<GhRelease> = resp.json().await.map_err(|e| ApiError {
        message: crate::lang::tf("err.parse", &[("{e}", &e.to_string())]),
        rate_limited: false,
    })?;

    Ok(raw
        .into_iter()
        .filter(|r| !r.draft)
        .map(|r| {
            let assets = r
                .assets
                .into_iter()
                .map(|a| AssetInfo {
                    name: a.name,
                    url: a.browser_download_url,
                    size: a.size,
                    digest: a.digest,
                    synthetic: false,
                })
                .collect();
            ReleaseInfo {
                name: r.name.unwrap_or_else(|| r.tag_name.clone()),
                tag: r.tag_name,
                prerelease: r.prerelease,
                draft: r.draft,
                published_at: r.published_at,
                html_url: r.html_url,
                assets,
                source_zip: r.zipball_url,
            }
        })
        .collect())
}

/// Сведения о репозитории.
#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRepo {
    description: Option<String>,
}

/// Получить описание репозитория (используется, если описание ещё пустое).
pub async fn repo_info(client: &Client, slug: &str) -> Result<RepoInfo, String> {
    let url = format!("{API}/repos/{slug}");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| crate::lang::tf("err.network", &[("{e}", &e.to_string())]))?;
    if !resp.status().is_success() {
        return Err(crate::lang::tf(
            "err.http_status_short",
            &[("{status}", &resp.status().to_string())],
        ));
    }
    let raw: GhRepo = resp
        .json()
        .await
        .map_err(|e| crate::lang::tf("err.parse", &[("{e}", &e.to_string())]))?;
    Ok(RepoInfo {
        description: raw.description.filter(|d| !d.trim().is_empty()),
    })
}
