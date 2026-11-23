//! Backend of ferrochart.com.
//!
//! - `/api/*`: synthetic market data for the chart examples (bars, trades,
//!   order books, derived series, a live WebSocket feed). Deterministic in
//!   time: every visitor sees the same market, history never changes, and
//!   the live feed continues the history.
//! - responses are reused for a while (`cache`).
//! - `/api/live/*`: real bars and a live stream of a few crypto pairs
//!   (OKX), for the examples marked live (`live`).
//! - `/api/github/stars`: the repository's star count for the site header.
//! - everything else: the static site (`next build` output).

pub mod api;
pub mod cache;
pub mod github;
pub mod live;
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
    pub star_fetch: github::StarFetch,
    pub star_cache: Arc<github::StarCache>,
    pub response_cache: Arc<cache::ResponseCache>,
    /// Real data for `/api/live/*`.
    pub live: Arc<live::LiveHub>,
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
    /// Where `/api/github/stars` gets the count (GitHub; a stub in tests).
    pub star_fetch: github::StarFetch,
    /// Where `/api/live/*` gets real data (OKX; a fake in tests).
    pub live: Arc<dyn live::Upstream>,
    /// Fetch the live examples' bars when the app starts (needs a Tokio runtime).
    pub warm_live: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            site_dir: None,
            cors: false,
            max_streams: 500,
            clock: system_clock(),
            star_fetch: github::github_fetch(),
            live: live::okx(),
            warm_live: false,
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
        star_fetch: config.star_fetch,
        star_cache: Arc::default(),
        response_cache: cache::shared(),
        live: Arc::new(live::LiveHub::new(config.live)),
    };
    if config.warm_live {
        tokio::spawn(live::warm(state.live.clone(), state.now()));
    }
    let mut api = api::router()
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            cache::layer,
        ))
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
