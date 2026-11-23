//! HTTP API under `/api`: symbols, bars, trades, order books, derived
//! series. Responses are columns (`{ time: [...], open: [...] }`), the shape
//! the engine's `setData` / `appendTrades` / `appendDepth` take.

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::AppState;
use crate::market::bars::{self, Interval};
use crate::market::series::{self, Cadence};
use crate::market::{altbars, flow, metrics, symbols};

/// Most bars, snapshots or rows per request.
pub const MAX_BARS: usize = 20_000;
pub const MAX_TRADES: f64 = 250_000.0;
pub const MAX_SNAPSHOTS: usize = 3_600;
pub const MAX_LEVELS: usize = 200;
const DEFAULT_COUNT: usize = 500;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/symbols", get(symbol_list))
        .route("/bars", get(get_bars))
        .route("/trades", get(get_trades))
        .route("/depth", get(get_depth))
        .route("/metrics", get(|| async { Json(json!(metrics::METRICS)) }))
        .route("/metrics/{name}", get(get_metric))
        .route(
            "/series",
            get(|| async { Json(json!(series::series_info())) }),
        )
        .route("/series/{name}", get(get_series))
        .route(
            "/events",
            get(|| async { Json(json!(series::events_info())) }),
        )
        .route("/events/{name}", get(get_events))
        .route("/forecast", get(get_forecast))
        .route("/altbars", get(get_altbars))
        .route("/stream", get(crate::stream::handler))
        .route("/live/bars", get(crate::live::get_bars))
        .route("/live/stream", get(crate::live::stream))
        .route("/github/stars", get(crate::github::handler))
}

/// A request error: 400 with `{ "error": message }`.
pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn bad(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

type ApiResult = Result<Response, ApiError>;

pub fn symbol(id: Option<&str>) -> Result<&'static symbols::Symbol, ApiError> {
    let id = id.unwrap_or("BTCUSD");
    symbols::find(id).ok_or_else(|| ApiError::bad(format!("unknown symbol '{id}'")))
}

pub fn interval(s: Option<&str>) -> Result<Interval, ApiError> {
    let s = s.unwrap_or("1m");
    Interval::parse(s).ok_or_else(|| {
        ApiError::bad(format!(
            "interval must be one of {}",
            Interval::ALLOWED.join(", ")
        ))
    })
}

async fn symbol_list() -> Json<Value> {
    Json(json!(symbols::SYMBOLS))
}

#[derive(Deserialize)]
pub struct RangeQuery {
    symbol: Option<String>,
    interval: Option<String>,
    /// UTC ms, inclusive.
    from: Option<i64>,
    /// UTC ms, exclusive; default now.
    to: Option<i64>,
    /// Bars ending at `to` (when `from` is absent).
    count: Option<usize>,
}

/// Resolves `from`/`to`/`count` to a range of bar starts.
fn bar_range(
    sym: &symbols::Symbol,
    iv: Interval,
    q: &RangeQuery,
    now: i64,
) -> Result<(i64, i64, Option<usize>), ApiError> {
    let to =
        q.to.unwrap_or(now.saturating_add(1))
            .min(now.saturating_add(1));
    match (q.from, q.count) {
        (Some(from), _) => {
            if from >= to {
                return Err(ApiError::bad("from must be before to"));
            }
            let n = ((to - from) as f64 / iv.ms() as f64 * sym.open_fraction()).ceil() as usize;
            if n > MAX_BARS {
                return Err(ApiError::bad(format!(
                    "at most {MAX_BARS} bars per request"
                )));
            }
            Ok((from, to, None))
        }
        (None, count) => {
            let count = count.unwrap_or(DEFAULT_COUNT);
            if count == 0 || count > MAX_BARS {
                return Err(ApiError::bad(format!("count must be 1..={MAX_BARS}")));
            }
            // Enough calendar time for `count` open bars, then keep the last ones.
            let span = (count as f64 / sym.open_fraction().max(0.05) * 1.25) as i64 * iv.ms()
                + 4 * symbols::DAY_MS;
            Ok((to - span, to, Some(count)))
        }
    }
}

/// History that cannot change may be cached; the forming bar may not.
fn cache(resp: &mut Response, settled: bool) {
    let v = if settled {
        "public, max-age=86400"
    } else {
        "no-store"
    };
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(v));
}

/// Runs CPU-heavy generation off the async workers.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    let task = tokio::task::spawn_blocking(f);
    match tokio::time::timeout(Duration::from_secs(20), task).await {
        Ok(Ok(v)) => Ok(v),
        _ => Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "generation failed".into(),
        )),
    }
}

async fn get_bars(State(st): State<AppState>, Query(q): Query<RangeQuery>) -> ApiResult {
    let sym = symbol(q.symbol.as_deref())?;
    let iv = interval(q.interval.as_deref())?;
    let now = st.now();
    let (from, to, keep) = bar_range(sym, iv, &q, now)?;
    let out = blocking(move || {
        let mut b = bars::bars(sym, iv, from, to, now);
        if let Some(n) = keep {
            b.keep_last(n);
        }
        b
    })
    .await?;
    let mut resp = Json(out).into_response();
    cache(&mut resp, to + iv.ms() < now);
    Ok(resp)
}

#[derive(Deserialize)]
struct TradesQuery {
    symbol: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    /// Milliseconds before `to` (when `from` is absent). Default 10 minutes.
    span: Option<i64>,
}

async fn get_trades(State(st): State<AppState>, Query(q): Query<TradesQuery>) -> ApiResult {
    let sym = symbol(q.symbol.as_deref())?;
    let now = st.now();
    let to =
        q.to.unwrap_or(now.saturating_add(1))
            .min(now.saturating_add(1));
    let from = q.from.unwrap_or(to - q.span.unwrap_or(600_000).max(1));
    if from >= to {
        return Err(ApiError::bad("from must be before to"));
    }
    if flow::expected_trades(sym, from, to) > MAX_TRADES {
        return Err(ApiError::bad(
            "range too long: request fewer trades (about 250 000 at most)",
        ));
    }
    let out = blocking(move || flow::trades(sym, from, to)).await?;
    let mut resp = Json(out).into_response();
    cache(&mut resp, to < now);
    Ok(resp)
}

#[derive(Deserialize)]
struct DepthQuery {
    symbol: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    /// Snapshot spacing, ms. Default 1000.
    every: Option<i64>,
    /// Snapshots ending at `to` (when `from` is absent). Default 300.
    count: Option<usize>,
    /// Levels per side. Default 40.
    levels: Option<usize>,
    /// Ticks per level. Default 1.
    step: Option<u32>,
}

async fn get_depth(State(st): State<AppState>, Query(q): Query<DepthQuery>) -> ApiResult {
    let sym = symbol(q.symbol.as_deref())?;
    let now = st.now();
    let every = q.every.unwrap_or(1000);
    if every < 100 {
        return Err(ApiError::bad("every must be at least 100 ms"));
    }
    let levels = q.levels.unwrap_or(40);
    if levels == 0 || levels > MAX_LEVELS {
        return Err(ApiError::bad(format!("levels must be 1..={MAX_LEVELS}")));
    }
    let step = q.step.unwrap_or(1).clamp(1, 100_000);
    let to =
        q.to.unwrap_or(now.saturating_add(1))
            .min(now.saturating_add(1));
    let from = q
        .from
        .unwrap_or(to - every * q.count.unwrap_or(300).min(MAX_SNAPSHOTS) as i64);
    if from >= to {
        return Err(ApiError::bad("from must be before to"));
    }
    if (to - from) / every > MAX_SNAPSHOTS as i64 {
        return Err(ApiError::bad(format!(
            "at most {MAX_SNAPSHOTS} snapshots per request"
        )));
    }
    let out = blocking(move || flow::depth(sym, from, to, every, levels, step)).await?;
    let mut resp = Json(out).into_response();
    cache(&mut resp, to < now);
    Ok(resp)
}

async fn get_metric(
    State(st): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<RangeQuery>,
) -> ApiResult {
    let metric = metrics::find(&name).ok_or_else(|| {
        ApiError(
            StatusCode::NOT_FOUND,
            format!("unknown metric '{name}' (see /api/metrics)"),
        )
    })?;
    let sym = symbol(q.symbol.as_deref())?;
    let iv = interval(q.interval.as_deref().or(Some("1h")))?;
    let now = st.now();
    let (from, to, keep) = bar_range(sym, iv, &q, now)?;
    let table = blocking(move || {
        let mut starts = bars::starts(sym, iv, from, to, now);
        if let Some(n) = keep {
            starts.drain(..starts.len().saturating_sub(n));
        }
        metrics::compute(sym, metric, iv, &starts, now)
    })
    .await?;
    let mut obj = Map::new();
    obj.insert("time".into(), json!(table.time));
    for (k, v) in table.columns {
        obj.insert(k.into(), json!(v));
    }
    let mut resp = Json(Value::Object(obj)).into_response();
    cache(&mut resp, to + iv.ms() < now);
    Ok(resp)
}

/// Most bars ahead of now for series, events and forecasts.
pub const MAX_AHEAD: usize = 2_000;

#[derive(Deserialize)]
pub struct SeriesQuery {
    symbol: Option<String>,
    /// A bar interval, or `1mo`, `1q`, `1y`. Default `1d`.
    interval: Option<String>,
    /// Bars up to now. Default 500.
    count: Option<usize>,
    /// Bars after now. Default 0.
    ahead: Option<usize>,
}

fn cadence(s: Option<&str>) -> Result<Cadence, ApiError> {
    let s = s.unwrap_or("1d");
    Cadence::parse(s).ok_or_else(|| {
        ApiError::bad(format!(
            "interval must be one of {}, {}",
            Interval::ALLOWED.join(", "),
            Cadence::EXTRA.join(", ")
        ))
    })
}

fn series_starts(
    q: &SeriesQuery,
    now: i64,
) -> Result<(&'static symbols::Symbol, Cadence, Vec<i64>), ApiError> {
    let sym = symbol(q.symbol.as_deref())?;
    let cad = cadence(q.interval.as_deref())?;
    let count = q.count.unwrap_or(DEFAULT_COUNT);
    let ahead = q.ahead.unwrap_or(0);
    if count > MAX_BARS || ahead > MAX_AHEAD || count + ahead == 0 {
        return Err(ApiError::bad(format!(
            "count must be 0..={MAX_BARS} and ahead 0..={MAX_AHEAD}, not both 0"
        )));
    }
    Ok((sym, cad, series::starts(sym, cad, count, ahead, now)))
}

fn no_store(v: Value) -> Response {
    let mut resp = Json(v).into_response();
    cache(&mut resp, false);
    resp
}

async fn get_series(
    State(st): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<SeriesQuery>,
) -> ApiResult {
    let spec = series::find(&name).ok_or_else(|| {
        ApiError(
            StatusCode::NOT_FOUND,
            format!("unknown series '{name}' (see /api/series)"),
        )
    })?;
    let now = st.now();
    let (sym, cad, starts) = series_starts(&q, now)?;
    let table = blocking(move || series::compute(sym, spec, cad, &starts, now)).await?;
    let mut obj = Map::new();
    obj.insert("time".into(), json!(table.time));
    for (k, v) in table.columns {
        obj.insert(k, json!(v));
    }
    Ok(no_store(Value::Object(obj)))
}

async fn get_events(
    State(st): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<SeriesQuery>,
) -> ApiResult {
    let spec = series::find_events(&name).ok_or_else(|| {
        ApiError(
            StatusCode::NOT_FOUND,
            format!("unknown events '{name}' (see /api/events)"),
        )
    })?;
    let now = st.now();
    let (sym, cad, starts) = series_starts(&q, now)?;
    let e = blocking(move || series::events(sym, spec, cad, &starts, now)).await?;
    Ok(no_store(
        json!({ "time": e.time, "label": e.label, "value": e.value }),
    ))
}

#[derive(Deserialize)]
struct ForecastQuery {
    symbol: Option<String>,
    interval: Option<String>,
    /// Bars ahead. Default 60.
    ahead: Option<usize>,
    /// Percentiles, comma-separated. Default `5,25,50,75,95`.
    quantiles: Option<String>,
}

async fn get_forecast(State(st): State<AppState>, Query(q): Query<ForecastQuery>) -> ApiResult {
    let sym = symbol(q.symbol.as_deref())?;
    let iv = interval(q.interval.as_deref().or(Some("1d")))?;
    let ahead = q.ahead.unwrap_or(60);
    if ahead == 0 || ahead > MAX_AHEAD {
        return Err(ApiError::bad(format!("ahead must be 1..={MAX_AHEAD}")));
    }
    let qs: Vec<f64> = q
        .quantiles
        .as_deref()
        .unwrap_or("5,25,50,75,95")
        .split(',')
        .map(|x| x.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| ApiError::bad("quantiles must be numbers"))?;
    if qs.is_empty() || qs.len() > 20 || qs.iter().any(|&x| !(x > 0.0 && x < 100.0)) {
        return Err(ApiError::bad("1 to 20 quantiles, each in (0, 100)"));
    }
    let now = st.now();
    let (time, cols) = blocking(move || series::forecast(sym, iv, ahead, &qs, now)).await?;
    let mut obj = Map::new();
    obj.insert("time".into(), json!(time));
    for (k, v) in cols {
        obj.insert(k, json!(v));
    }
    Ok(no_store(Value::Object(obj)))
}

#[derive(Deserialize)]
struct AltBarsQuery {
    /// `tick`, `volume`, `dollar`, `lineBreak` or `delta`.
    #[serde(rename = "type")]
    kind: Option<String>,
    symbol: Option<String>,
    /// Trades over the last `span` ms (tick, volume, dollar, delta). Default 1 hour.
    span: Option<i64>,
    /// Trades, volume or notional per bar.
    size: Option<f64>,
    /// Line break: time bars to build from, and lines to reverse (default 3).
    interval: Option<String>,
    count: Option<usize>,
    lines: Option<usize>,
}

async fn get_altbars(State(st): State<AppState>, Query(q): Query<AltBarsQuery>) -> ApiResult {
    let sym = symbol(q.symbol.as_deref())?;
    let now = st.now();
    let kind = q.kind.as_deref().unwrap_or("tick");
    let span = q.span.unwrap_or(3_600_000).max(1);
    let to = now.saturating_add(1);
    let from = to - span;
    let trades_ok = || {
        if flow::expected_trades(sym, from, to) > MAX_TRADES {
            Err(ApiError::bad("span too long: about 250 000 trades at most"))
        } else {
            Ok(())
        }
    };
    let out = match kind {
        "tick" | "volume" | "dollar" => {
            trades_ok()?;
            let k = match kind {
                "tick" => altbars::Kind::Tick(q.size.unwrap_or(200.0).clamp(1.0, 1e6) as usize),
                "volume" => altbars::Kind::Volume(
                    q.size
                        .unwrap_or(sym.avg_size * 400.0)
                        .max(sym.avg_size / 10.0),
                ),
                _ => altbars::Kind::Dollar(
                    q.size
                        .unwrap_or(sym.avg_size * sym.base * 400.0)
                        .max(sym.avg_size * sym.base / 10.0),
                ),
            };
            blocking(move || altbars::from_trades(sym, k, from, to)).await?
        }
        "delta" => {
            trades_ok()?;
            let iv = interval(q.interval.as_deref())?;
            blocking(move || altbars::delta(sym, iv, from, to, now)).await?
        }
        "lineBreak" => {
            let iv = interval(q.interval.as_deref().or(Some("1h")))?;
            let count = q.count.unwrap_or(3000).clamp(1, MAX_BARS);
            let lines = q.lines.unwrap_or(3).clamp(1, 10);
            let rq = RangeQuery {
                symbol: None,
                interval: None,
                from: None,
                to: None,
                count: Some(count),
            };
            let (f, t, keep) = bar_range(sym, iv, &rq, now)?;
            blocking(move || {
                let mut b = bars::bars(sym, iv, f, t, now);
                if let Some(n) = keep {
                    b.keep_last(n);
                }
                altbars::line_break(&b, lines)
            })
            .await?
        }
        other => {
            return Err(ApiError::bad(format!(
                "unknown type '{other}': tick, volume, dollar, lineBreak or delta"
            )));
        }
    };
    let mut resp = Json(out).into_response();
    cache(&mut resp, false);
    Ok(resp)
}
