//! Response cache of the data API. Every visitor asks for the same URLs
//! (the examples' requests are fixed), and history is deterministic, so a
//! response is reused instead of being generated again:
//!
//! - responses that end at "now" (the forming bar) for [`LIVE_TTL_MS`];
//!   they are at most that old, and the live stream's `since` catches up
//!   (`stream.rs`). They are sent with `s-maxage` so a CDN may keep them
//!   as long, and `max-age=0` so browsers revalidate;
//! - settled history (`max-age=86400` from the handler) for
//!   [`SETTLED_TTL_MS`].
//!
//! Only successful GET responses are kept, at most [`MAX_BYTES`] in all;
//! the stream and the star count (cached by itself) are left out.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes, to_bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// How long a response that ends at "now" is reused.
pub const LIVE_TTL_MS: i64 = 30_000;
/// How long a response of settled history is reused.
pub const SETTLED_TTL_MS: i64 = 10 * 60_000;
/// Most body bytes kept.
pub const MAX_BYTES: usize = 96 << 20;
/// Largest body kept (bigger ones are generated each time).
const MAX_ENTRY: usize = 8 << 20;
/// `Cache-Control` of a response that ends at "now".
const LIVE_CACHE: &str = "public, max-age=0, s-maxage=30";

struct Entry {
    expires: i64,
    headers: HeaderMap,
    body: Bytes,
}

#[derive(Default)]
pub struct ResponseCache {
    inner: Mutex<(HashMap<String, Entry>, usize)>,
}

impl ResponseCache {
    fn get(&self, key: &str, now: i64) -> Option<(HeaderMap, Bytes)> {
        let map = self.inner.lock().ok()?;
        let e = map.0.get(key).filter(|e| e.expires > now)?;
        Some((e.headers.clone(), e.body.clone()))
    }

    fn put(&self, key: String, entry: Entry, now: i64) {
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        let (map, bytes) = &mut *guard;
        if let Some(old) = map.remove(&key) {
            *bytes -= old.body.len();
        }
        if *bytes + entry.body.len() > MAX_BYTES {
            map.retain(|_, e| e.expires > now);
            *bytes = map.values().map(|e| e.body.len()).sum();
        }
        // Still full: drop the entries closest to expiring.
        while *bytes + entry.body.len() > MAX_BYTES {
            let Some(k) = map
                .iter()
                .min_by_key(|(_, e)| e.expires)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some(old) = map.remove(&k) {
                *bytes -= old.body.len();
            }
        }
        *bytes += entry.body.len();
        map.insert(key, entry);
    }

    /// Number of entries (for tests).
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |m| m.0.len())
    }

    /// Whether nothing is cached.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn cacheable(req: &Request) -> bool {
    let path = req.uri().path();
    req.method() == Method::GET && !path.ends_with("/stream") && !path.contains("/github/")
}

pub async fn layer(State(st): State<AppState>, req: Request, next: Next) -> Response {
    if !cacheable(&req) {
        return next.run(req).await;
    }
    let key = req
        .uri()
        .path_and_query()
        .map_or_else(String::new, |p| p.as_str().to_owned());
    let now = st.now();
    if let Some((headers, body)) = st.response_cache.get(&key, now) {
        let mut resp = Response::new(Body::from(body));
        *resp.headers_mut() = headers;
        resp.headers_mut()
            .insert("x-cache", HeaderValue::from_static("hit"));
        return resp;
    }
    let resp = next.run(req).await;
    if resp.status() != StatusCode::OK {
        return resp;
    }
    let settled = resp
        .headers()
        .get(header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("max-age=86400"));
    let (mut parts, body) = resp.into_parts();
    let Ok(body) = to_bytes(body, usize::MAX).await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "response failed").into_response();
    };
    if !settled {
        parts
            .headers
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(LIVE_CACHE));
    }
    if body.len() <= MAX_ENTRY {
        st.response_cache.put(
            key,
            Entry {
                expires: now + if settled { SETTLED_TTL_MS } else { LIVE_TTL_MS },
                headers: parts.headers.clone(),
                body: body.clone(),
            },
            now,
        );
    }
    parts
        .headers
        .insert("x-cache", HeaderValue::from_static("miss"));
    Response::from_parts(parts, Body::from(body))
}

/// The cache shared by the requests of an app.
pub fn shared() -> Arc<ResponseCache> {
    Arc::default()
}
