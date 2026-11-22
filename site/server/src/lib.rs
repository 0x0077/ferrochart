//! Backend of ferrochart.com.
//!
//! - `/api/*`: synthetic market data for the chart examples (bars, trades,
//!   order books, derived series, a live WebSocket feed). Deterministic in
//!   time: every visitor sees the same market, history never changes, and
//!   the live feed continues the history.
//! - everything else: the static site (`next build` output).

pub mod api;
pub mod market;
pub mod site;
pub mod stream;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::http::{HeaderValue, header};
use tokio::sync::Semaphore;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

/// Current time, UTC ms (replaceable in tests).
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    })
}

#[derive(Clone)]
pub struct AppState {
    clock: Clock,
    /// Live streams left.
    pub streams: Arc<Semaphore>,
}

impl AppState {
    pub fn now(&self) -> i64 {
        (self.clock)()
    }
}

pub struct Config {
    /// The static site; `None`: API only.
    pub site_dir: Option<PathBuf>,
    /// Allow cross-origin API calls (for `next dev` on another port).
    pub cors: bool,
    pub max_streams: usize,
    pub clock: Clock,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            site_dir: None,
            cors: false,
            max_streams: 500,
            clock: system_clock(),
        }
    }
}

/// Content-Security-Policy of the site. The examples run their code with
/// `new Function` (the code shown is the code that runs) and the engine is
/// WebAssembly; Next.js inlines its bootstrap scripts.
const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'; \
style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; \
frame-ancestors 'none'; base-uri 'self'; form-action 'none'";

pub fn app(config: Config) -> Router {
    let state = AppState {
        clock: config.clock,
        streams: Arc::new(Semaphore::new(config.max_streams)),
    };
    let mut api = api::router()
        .with_state(state)
        .layer(CompressionLayer::new().quality(CompressionLevel::Fastest));
    if config.cors {
        api = api.layer(CorsLayer::permissive());
    }
    let mut app = Router::new().nest("/api", api);
    if let Some(dir) = config.site_dir {
        app = app.merge(site::router(dir));
    }
    app.layer(SetResponseHeaderLayer::overriding(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    ))
    .layer(SetResponseHeaderLayer::overriding(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    ))
    .layer(TraceLayer::new_for_http())
}
