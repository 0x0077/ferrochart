//! Real market data for a few examples: `/api/live/bars` and the WebSocket
//! `/api/stream?…&live=true` (the simulated stream's path, which proxies
//! already upgrade), from the public OKX spot market (no key).
//!
//! - Symbols `BTCUSD`, `ETHUSD`, `SOLUSD` (OKX `BTC-USDT` …); intervals
//!   from one minute to one week.
//! - One store per symbol and interval, shared by every visitor: the bars
//!   fetched so far. A request refreshes the newest bars at most once per
//!   [`REFRESH_MS`] and fetches older ones only as far as some request
//!   asked; every upstream call goes through one pacer
//!   ([`UPSTREAM_GAP_MS`]) to stay well inside OKX's limits however many
//!   visitors there are.
//! - When OKX cannot be reached and nothing is stored yet, the response is
//!   the simulated market of the same symbol (`x-data-source: simulated`
//!   instead of `okx`) and the stream is the simulated stream, so the
//!   examples still run.
//!
//! The JSON is that of `/api/bars` and `/api/stream`.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::AppState;
use crate::api::{ApiError, interval, symbol};
use crate::market::bars::{self, Bars, Interval};
use crate::market::flow::Trades;
use crate::market::symbols::Symbol;

/// Most bars per request.
pub const MAX_BARS: usize = 3000;
/// Bars OKX serves from its recent endpoint, 300 a call; older ones come
/// from history, 100 a call.
pub const RECENT: usize = 1440;
/// How often the newest bars of a store are fetched again, at most.
pub const REFRESH_MS: i64 = 1000;
/// Least time between two upstream calls (all stores together).
pub const UPSTREAM_GAP_MS: u64 = 70;
/// How often the stream sends the forming bar.
pub const TICK: Duration = Duration::from_millis(1000);
/// Bars of history a stream's `since` may ask for.
pub const MAX_CATCH_UP: usize = 120;

/// One upstream bar: time (UTC ms), open, high, low, close, volume.
pub type Candle = (i64, [f64; 5]);
/// One upstream trade: id, time, price, size, side (1 buy, -1 sell).
pub type Trade = (u64, i64, f64, f64, i8);

type Fut<T> = Pin<Box<dyn Future<Output = Option<T>> + Send>>;

/// Where the data comes from (OKX; a fake in tests).
pub trait Upstream: Send + Sync {
    /// The newest `limit` bars (`before` = None) or `limit` bars older than
    /// `before`, any order. `None` when the source cannot be reached.
    fn candles(
        &self,
        inst: &'static str,
        bar: &'static str,
        before: Option<i64>,
        limit: usize,
    ) -> Fut<Vec<Candle>>;
    /// The newest trades, any order.
    fn trades(&self, inst: &'static str) -> Fut<Vec<Trade>>;
}

/// OKX instrument of a symbol.
pub fn instrument(sym: &str) -> Option<&'static str> {
    match sym {
        "BTCUSD" => Some("BTC-USDT"),
        "ETHUSD" => Some("ETH-USDT"),
        "SOLUSD" => Some("SOL-USDT"),
        _ => None,
    }
}

/// OKX bar name of an interval (UTC-aligned).
pub fn okx_bar(iv: Interval) -> Option<&'static str> {
    const M: i64 = 60_000;
    Some(match iv.ms() {
        x if x == M => "1m",
        x if x == 3 * M => "3m",
        x if x == 5 * M => "5m",
        x if x == 15 * M => "15m",
        x if x == 30 * M => "30m",
        x if x == 60 * M => "1H",
        x if x == 120 * M => "2H",
        x if x == 240 * M => "4H",
        x if x == 1440 * M => "1Dutc",
        x if x == 7 * 1440 * M => "1Wutc",
        _ => return None,
    })
}

/// The bars of one symbol and interval fetched so far.
#[derive(Default)]
struct Store {
    /// Sorted by time, no duplicates.
    bars: Vec<Candle>,
    /// When the newest bars were last fetched.
    refreshed: i64,
    /// Upstream has nothing older.
    exhausted: bool,
}

impl Store {
    /// Puts `new` in, replacing bars of the same time.
    fn merge(&mut self, mut new: Vec<Candle>) {
        new.retain(|(_, v)| v.iter().all(|x| x.is_finite()));
        if new.is_empty() {
            return;
        }
        // Stable sort: of two bars at one time the new one comes last and stays.
        self.bars.extend(new);
        self.bars.sort_by_key(|c| c.0);
        let mut out: Vec<Candle> = Vec::with_capacity(self.bars.len());
        for c in self.bars.drain(..) {
            match out.last_mut() {
                Some(last) if last.0 == c.0 => *last = c,
                _ => out.push(c),
            }
        }
        self.bars = out;
    }
}

#[derive(Default)]
struct TradeStore {
    /// The newest trades seen, oldest first, at most [`TRADE_KEEP`].
    trades: Vec<Trade>,
    refreshed: i64,
}
const TRADE_KEEP: usize = 2000;

/// OKX instrument and bar name.
type StoreKey = (&'static str, &'static str);

/// The stores and the upstream they share.
pub struct LiveHub {
    upstream: Arc<dyn Upstream>,
    stores: Mutex<HashMap<StoreKey, Arc<Mutex<Store>>>>,
    trades: Mutex<HashMap<&'static str, Arc<Mutex<TradeStore>>>>,
    /// When the next upstream call may start (tokio clock).
    pacer: Mutex<tokio::time::Instant>,
}

impl LiveHub {
    pub fn new(upstream: Arc<dyn Upstream>) -> Self {
        Self {
            upstream,
            stores: Mutex::default(),
            trades: Mutex::default(),
            pacer: Mutex::new(tokio::time::Instant::now()),
        }
    }

    /// Waits for the next free slot for an upstream call.
    async fn pace(&self) {
        let at = {
            let mut next = self.pacer.lock().await;
            let now = tokio::time::Instant::now();
            let at = (*next).max(now);
            *next = at + Duration::from_millis(UPSTREAM_GAP_MS);
            at
        };
        tokio::time::sleep_until(at).await;
    }

    async fn store(&self, key: StoreKey) -> Arc<Mutex<Store>> {
        self.stores.lock().await.entry(key).or_default().clone()
    }

    /// The last `count` bars (fewer if upstream has fewer), refreshed if due.
    /// `None` when nothing could be fetched.
    pub async fn bars(
        &self,
        inst: &'static str,
        bar: &'static str,
        count: usize,
        now: i64,
    ) -> Option<Vec<Candle>> {
        let store = self.store((inst, bar)).await;
        let mut s = store.lock().await;
        if s.bars.is_empty() || now - s.refreshed >= REFRESH_MS {
            self.pace().await;
            if let Some(c) = self.upstream.candles(inst, bar, None, 300).await {
                s.merge(c);
                s.refreshed = now;
            }
        }
        // Older bars: 300 a call within the recent ones, then 100 (OKX's
        // page for history).
        let mut pages = 0;
        while s.bars.len() < count && !s.exhausted && !s.bars.is_empty() && pages < 40 {
            pages += 1;
            self.pace().await;
            let oldest = s.bars[0].0;
            let page = if s.bars.len() + 300 <= RECENT {
                300
            } else {
                100
            };
            match self.upstream.candles(inst, bar, Some(oldest), page).await {
                Some(c) if c.iter().any(|x| x.0 < oldest) => s.merge(c),
                Some(_) => s.exhausted = true,
                None => break,
            }
        }
        if s.bars.is_empty() {
            return None;
        }
        Some(s.bars[s.bars.len().saturating_sub(count)..].to_vec())
    }

    /// Trades newer than `after_id`, refreshed if due.
    pub async fn trades_after(&self, inst: &'static str, after_id: u64, now: i64) -> Vec<Trade> {
        let store = self.trades.lock().await.entry(inst).or_default().clone();
        let mut s = store.lock().await;
        if now - s.refreshed >= REFRESH_MS {
            self.pace().await;
            if let Some(mut t) = self.upstream.trades(inst).await {
                s.refreshed = now;
                let last = s.trades.last().map_or(0, |x| x.0);
                t.retain(|x| x.0 > last);
                t.sort_by_key(|x| x.0);
                s.trades.extend(t);
                let extra = s.trades.len().saturating_sub(TRADE_KEEP);
                s.trades.drain(..extra);
            }
        }
        s.trades
            .iter()
            .filter(|x| x.0 > after_id)
            .copied()
            .collect()
    }
}

/// The stores the site's live examples use, and how many bars each needs
/// (`catalog/*.ts`, `live: true`): filled when the server starts.
pub const WARM: &[(&str, &str, usize)] = &[
    ("BTC-USDT", "1m", 1440),
    ("BTC-USDT", "5m", 1440),
    ("BTC-USDT", "15m", 1440),
    ("BTC-USDT", "1H", 1440),
    ("BTC-USDT", "4H", 1200),
    ("ETH-USDT", "5m", 1440),
    ("ETH-USDT", "1H", 1440),
    ("SOL-USDT", "15m", 1200),
    ("SOL-USDT", "1H", 1440),
];

/// Fills the [`WARM`] stores, one after the other.
pub async fn warm(hub: Arc<LiveHub>, now: i64) {
    for &(inst, bar, count) in WARM {
        let n = hub.bars(inst, bar, count, now).await.map_or(0, |b| b.len());
        tracing::info!("live: {inst} {bar}: {n} bars");
    }
}

fn columns(c: &[Candle]) -> Bars {
    let mut b = Bars {
        time: Vec::with_capacity(c.len()),
        open: Vec::with_capacity(c.len()),
        high: Vec::with_capacity(c.len()),
        low: Vec::with_capacity(c.len()),
        close: Vec::with_capacity(c.len()),
        volume: Vec::with_capacity(c.len()),
    };
    for &(t, [o, h, l, cl, v]) in c {
        b.time.push(t);
        b.open.push(o);
        b.high.push(h);
        b.low.push(l);
        b.close.push(cl);
        b.volume.push(v);
    }
    b
}

fn bar_msg(&(t, [o, h, l, c, v]): &Candle) -> String {
    json!({ "type": "bar", "time": t, "open": o, "high": h, "low": l, "close": c, "volume": v })
        .to_string()
}

fn source(resp: &mut Response, okx: bool) {
    resp.headers_mut().insert(
        "x-data-source",
        HeaderValue::from_static(if okx { "okx" } else { "simulated" }),
    );
}

#[derive(Deserialize)]
pub struct BarsQuery {
    symbol: Option<String>,
    interval: Option<String>,
    /// Default 500.
    count: Option<usize>,
}

/// Resolves the symbol and interval of a live request.
fn live_key(
    sym: Option<&str>,
    iv: Option<&str>,
) -> Result<(&'static Symbol, Interval, &'static str, &'static str), ApiError> {
    let s = symbol(sym)?;
    let i = interval(iv)?;
    let inst =
        instrument(s.id).ok_or_else(|| ApiError::bad("live data has BTCUSD, ETHUSD and SOLUSD"))?;
    let bar = okx_bar(i).ok_or_else(|| ApiError::bad("live data has intervals 1m to 1w"))?;
    Ok((s, i, inst, bar))
}

pub async fn get_bars(
    State(st): State<AppState>,
    Query(q): Query<BarsQuery>,
) -> Result<Response, ApiError> {
    let (sym, iv, inst, bar) = live_key(q.symbol.as_deref(), q.interval.as_deref())?;
    let count = q.count.unwrap_or(500);
    if count == 0 || count > MAX_BARS {
        return Err(ApiError::bad(format!("count must be 1..={MAX_BARS}")));
    }
    let now = st.now();
    if let Some(c) = st.live.bars(inst, bar, count, now).await {
        let mut resp = Json(columns(&c)).into_response();
        source(&mut resp, true);
        return Ok(resp);
    }
    tracing::warn!("live: {inst} {bar} unavailable, simulated data");
    let to = now.saturating_add(1);
    let from = to - (count as i64 + 2) * iv.ms();
    let mut b = bars::bars(sym, iv, from, to, now);
    b.keep_last(count);
    let mut resp = Json(b).into_response();
    source(&mut resp, false);
    Ok(resp)
}

/// `/api/stream?…&live=true`.
pub(crate) async fn stream(
    st: AppState,
    q: crate::stream::StreamQuery,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let (sym, iv, inst, bar) = live_key(q.symbol.as_deref(), q.interval.as_deref())?;
    let Ok(permit) = st.streams.clone().try_acquire_owned() else {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "too many live streams".into(),
        ));
    };
    let with_trades = q.trades.unwrap_or(true);
    Ok(ws
        .max_message_size(4096)
        .on_upgrade(move |socket| async move {
            // Nothing from OKX: the simulated stream, like the history.
            if st.live.bars(inst, bar, 2, st.now()).await.is_none() {
                crate::stream::run(socket, st, sym, iv, with_trades, q.since).await;
            } else {
                run(socket, st, inst, bar, with_trades, q.since).await;
            }
            drop(permit);
        })
        .into_response())
}

async fn run(
    mut socket: WebSocket,
    st: AppState,
    inst: &'static str,
    bar: &'static str,
    with_trades: bool,
    since: Option<i64>,
) {
    // Bars from this time on are sent on the next tick.
    let mut from = since.unwrap_or(i64::MAX);
    // Only trades after the stream starts.
    let mut last_trade = if with_trades {
        st.live
            .trades_after(inst, 0, st.now())
            .await
            .last()
            .map_or(0, |t| t.0)
    } else {
        0
    };
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let now = st.now();
                let mut msgs = Vec::new();
                if let Some(c) = st.live.bars(inst, bar, MAX_CATCH_UP, now).await {
                    if from == i64::MAX {
                        from = c.last().map_or(i64::MAX, |x| x.0);
                    }
                    for b in c.iter().filter(|b| b.0 >= from) {
                        msgs.push(bar_msg(b));
                    }
                    // Next time: the forming bar and anything after it.
                    from = c.last().map_or(from, |x| x.0);
                }
                if with_trades {
                    let t = st.live.trades_after(inst, last_trade, now).await;
                    if let Some(x) = t.last() {
                        last_trade = x.0;
                        let tr = Trades {
                            time: t.iter().map(|x| x.1).collect(),
                            price: t.iter().map(|x| x.2).collect(),
                            size: t.iter().map(|x| x.3).collect(),
                            side: t.iter().map(|x| x.4).collect(),
                        };
                        msgs.push(json!({ "type": "trades", "time": tr.time, "price": tr.price, "size": tr.size, "side": tr.side }).to_string());
                    }
                }
                for m in msgs {
                    if socket.send(Message::Text(m.into())).await.is_err() {
                        return;
                    }
                }
            }
            msg = socket.recv() => match msg {
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                _ => return,
            },
        }
    }
}

/// The real upstream: OKX's public REST API.
pub fn okx() -> Arc<dyn Upstream> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .user_agent("ferrochart-site")
        .timeout(Duration::from_secs(5))
        .build()
        .ok();
    Arc::new(Okx { client })
}

struct Okx {
    client: Option<reqwest::Client>,
}

impl Okx {
    fn get(&self, url: String) -> Fut<Vec<Vec<serde_json::Value>>> {
        let client = self.client.clone();
        Box::pin(async move {
            let resp = client?.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                tracing::warn!("okx: {} {url}", resp.status());
                return None;
            }
            let body: serde_json::Value = serde_json::from_slice(&resp.bytes().await.ok()?).ok()?;
            if body.get("code").and_then(|c| c.as_str()) != Some("0") {
                tracing::warn!(
                    "okx: {} {url}",
                    body.get("msg").and_then(|m| m.as_str()).unwrap_or("?")
                );
                return None;
            }
            let data = body.get("data")?.as_array()?;
            Some(
                data.iter()
                    .map(|row| match row {
                        serde_json::Value::Array(a) => a.clone(),
                        serde_json::Value::Object(o) => ["tradeId", "ts", "px", "sz", "side"]
                            .iter()
                            .map(|k| o.get(*k).cloned().unwrap_or_default())
                            .collect(),
                        _ => Vec::new(),
                    })
                    .collect(),
            )
        })
    }
}

fn num(v: Option<&serde_json::Value>) -> Option<f64> {
    v?.as_str()?.parse().ok()
}

impl Upstream for Okx {
    fn candles(
        &self,
        inst: &'static str,
        bar: &'static str,
        before: Option<i64>,
        limit: usize,
    ) -> Fut<Vec<Candle>> {
        // OKX's `after` is "older than". Pages of more than 100 come from the
        // recent endpoint, others from history-candles.
        let url = match before {
            Some(t) if limit > 100 => format!(
                "https://www.okx.com/api/v5/market/candles?instId={inst}&bar={bar}&after={t}&limit={limit}"
            ),
            None => format!(
                "https://www.okx.com/api/v5/market/candles?instId={inst}&bar={bar}&limit={limit}"
            ),
            Some(t) => format!(
                "https://www.okx.com/api/v5/market/history-candles?instId={inst}&bar={bar}&after={t}&limit={limit}"
            ),
        };
        let rows = self.get(url);
        Box::pin(async move {
            Some(
                rows.await?
                    .iter()
                    .filter_map(|r| {
                        let t = num(r.first())? as i64;
                        Some((
                            t,
                            [
                                num(r.get(1))?,
                                num(r.get(2))?,
                                num(r.get(3))?,
                                num(r.get(4))?,
                                num(r.get(5))?,
                            ],
                        ))
                    })
                    .collect(),
            )
        })
    }

    fn trades(&self, inst: &'static str) -> Fut<Vec<Trade>> {
        let rows = self.get(format!(
            "https://www.okx.com/api/v5/market/trades?instId={inst}&limit=100"
        ));
        Box::pin(async move {
            Some(
                rows.await?
                    .iter()
                    .filter_map(|r| {
                        let id = r.first()?.as_str()?.parse().ok()?;
                        let side = if r.get(4)?.as_str()? == "buy" { 1 } else { -1 };
                        Some((
                            id,
                            num(r.get(1))? as i64,
                            num(r.get(2))?,
                            num(r.get(3))?,
                            side,
                        ))
                    })
                    .collect(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_replaces_and_sorts() {
        let mut s = Store::default();
        s.merge(vec![(3, [1.0; 5]), (1, [1.0; 5])]);
        s.merge(vec![(2, [2.0; 5]), (3, [3.0; 5]), (4, [f64::NAN; 5])]);
        assert_eq!(s.bars.iter().map(|c| c.0).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(s.bars[2].1[0], 3.0);
    }

    #[test]
    fn instruments_and_bars() {
        assert_eq!(instrument("ETHUSD"), Some("ETH-USDT"));
        assert_eq!(instrument("ACME"), None);
        assert_eq!(okx_bar(Interval::parse("4h").unwrap()), Some("4H"));
        assert_eq!(okx_bar(Interval::parse("1d").unwrap()), Some("1Dutc"));
        assert_eq!(okx_bar(Interval::parse("5s").unwrap()), None);
    }
}
