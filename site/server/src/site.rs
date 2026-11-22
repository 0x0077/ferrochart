//! Serves the static Next.js export (`site/out`): `/x` is `x.html` or
//! `x/index.html`, unknown paths get `404.html`. Precompressed `.br`/`.gz`
//! files next to the originals are used when present.

use std::path::{Component, Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tower_http::services::ServeDir;

pub fn router(dir: PathBuf) -> Router {
    let fallback_dir = dir.clone();
    let not_found =
        axum::routing::any(move |req: Request| html_fallback(fallback_dir.clone(), req));
    let files = ServeDir::new(&dir)
        .append_index_html_on_directories(true)
        .precompressed_br()
        .precompressed_gzip()
        .fallback(not_found);
    Router::new()
        .fallback_service(files)
        .layer(axum::middleware::from_fn(cache_headers))
}

/// `/thumb` → `thumb.html`; otherwise `404.html` with status 404.
async fn html_fallback(dir: PathBuf, req: Request) -> Response {
    let path = req.uri().path().trim_matches('/');
    let safe = Path::new(path)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if safe
        && !path.is_empty()
        && !path.contains('.')
        && let Ok(body) = tokio::fs::read(dir.join(format!("{path}.html"))).await
    {
        return html(StatusCode::OK, body);
    }
    match tokio::fs::read(dir.join("404.html")).await {
        Ok(body) => html(StatusCode::NOT_FOUND, body),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn html(status: StatusCode, body: Vec<u8>) -> Response {
    let mut r = (status, Body::from(body)).into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    r
}

/// Build assets have content hashes and never change; pages and data do.
async fn cache_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let mut resp = next.run(req).await;
    let value = if path.starts_with("/_next/static/") {
        "public, max-age=31536000, immutable"
    } else if path.starts_with("/thumbs/") || path.starts_with("/wasm/") {
        "public, max-age=3600"
    } else {
        "no-cache"
    };
    if resp.status().is_success() {
        resp.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    }
    resp
}
