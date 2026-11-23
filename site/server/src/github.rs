//! `/api/github/stars`: the star count of the engine's GitHub repository,
//! for the site header's star button. The site is a static export, so the
//! count in its HTML is the one at build time; the page asks this endpoint
//! for a fresh one. Visitors' browsers never contact GitHub.
//!
//! The count is fetched at most once per [`TTL_MS`] (GitHub allows 60
//! unauthenticated requests an hour). When GitHub cannot be reached the last
//! count is kept, and the next attempt waits [`RETRY_MS`].

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tokio::sync::Mutex;

use crate::AppState;

/// The repository counted (the site's `lib/site.ts` links to the same one).
pub const REPO: &str = "0x0077/ferrochart";
/// How long a count is served before it is fetched again.
pub const TTL_MS: i64 = 10 * 60 * 1000;
/// How long to wait after a failed fetch before trying again.
pub const RETRY_MS: i64 = 60 * 1000;

/// Fetches the current count; `None` when GitHub cannot be reached.
pub type StarFetch =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Option<u64>> + Send>> + Send + Sync>;

/// The last count and when the next fetch is due.
#[derive(Default)]
pub struct StarCache {
    state: Mutex<(Option<u64>, i64)>,
}

/// The real fetch: `GET https://api.github.com/repos/{REPO}`.
pub fn github_fetch() -> StarFetch {
    // reqwest is built without a default crypto provider (ring, not aws-lc,
    // so the server cross-builds without CMake).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .user_agent("ferrochart-site")
        .timeout(Duration::from_secs(5))
        .build()
        .ok();
    Arc::new(move || {
        let client = client.clone();
        Box::pin(async move {
            let resp = client?
                .get(format!("https://api.github.com/repos/{REPO}"))
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .ok()?;
            if !resp.status().is_success() {
                tracing::warn!("github: {}", resp.status());
                return None;
            }
            let body: serde_json::Value = serde_json::from_slice(&resp.bytes().await.ok()?).ok()?;
            body.get("stargazers_count")?.as_u64()
        })
    })
}

pub async fn handler(State(s): State<AppState>) -> Response {
    let now = s.now();
    let stars = {
        // Held across the fetch: concurrent requests wait for one fetch.
        let mut guard = s.star_cache.state.lock().await;
        let (count, due) = *guard;
        if now >= due {
            match (s.star_fetch)().await {
                Some(n) => *guard = (Some(n), now + TTL_MS),
                None => *guard = (count, now + RETRY_MS),
            }
        }
        guard.0
    };
    match stars {
        Some(n) => {
            let mut r = Json(json!({ "repo": REPO, "stars": n })).into_response();
            r.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=300"),
            );
            r
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "GitHub cannot be reached" })),
        )
            .into_response(),
    }
}
