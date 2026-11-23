//! `/api/live/*` with a fake upstream: paging, refresh, fallback, stream.

#![allow(clippy::unwrap_used)]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use ferrochart_site_server::live::{Candle, Trade, Upstream};
use ferrochart_site_server::{Config, app};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

const MIN: i64 = 60_000;
/// 2025-11-21 12:00 UTC.
const NOW: i64 = 1_763_726_400_000;

/// One-minute bars up to NOW, 2000 of them in all; price = minutes since the first.
struct Fake {
    calls: AtomicUsize,
    first: i64,
}

impl Fake {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            first: NOW - 1999 * MIN,
        })
    }
}

type Fut<T> = Pin<Box<dyn Future<Output = Option<T>> + Send>>;

impl Upstream for Fake {
    fn candles(
        &self,
        inst: &'static str,
        bar: &'static str,
        before: Option<i64>,
        limit: usize,
    ) -> Fut<Vec<Candle>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!((inst, bar), ("BTC-USDT", "1m"));
        let end = before.map_or(NOW, |t| t - MIN);
        let first = self.first;
        let out: Vec<Candle> = (0..limit as i64)
            .map(|k| end - k * MIN)
            .filter(|&t| t >= first)
            .map(|t| {
                let p = ((t - first) / MIN) as f64;
                (t, [p, p + 1.0, p - 1.0, p, 1.0])
            })
            .collect();
        Box::pin(async move { Some(out) })
    }

    fn trades(&self, _: &'static str) -> Fut<Vec<Trade>> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) as u64;
        Box::pin(async move {
            Some(vec![
                (n * 10 + 1, NOW, 100.0, 0.5, 1),
                (n * 10 + 2, NOW, 99.0, 0.1, -1),
            ])
        })
    }
}

fn test_app(live: Arc<dyn Upstream>, clock: i64) -> Router {
    app(Config {
        max_streams: 4,
        clock: Arc::new(move || clock),
        star_fetch: Arc::new(|| Box::pin(async { Some(1) })),
        live,
        ..Config::default()
    })
}

async fn get(app: &Router, uri: &str) -> (StatusCode, String, Value) {
    let resp = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let source = resp
        .headers()
        .get("x-data-source")
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        source,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn bars_page_back_and_are_shared() {
    let fake = Fake::new();
    let app = test_app(fake.clone(), NOW);
    let (s, src, v) = get(&app, "/api/live/bars?symbol=BTCUSD&interval=1m&count=1200").await;
    assert_eq!((s, src.as_str()), (StatusCode::OK, "okx"));
    let t = v["time"].as_array().unwrap();
    assert_eq!(t.len(), 1200);
    assert_eq!(t.last().unwrap().as_i64(), Some(NOW));
    assert!(
        t.windows(2)
            .all(|w| w[1].as_i64().unwrap() - w[0].as_i64().unwrap() == MIN)
    );
    // 300 newest, then three pages of 300 (the recent endpoint).
    assert_eq!(fake.calls.load(Ordering::SeqCst), 4);
    // More than upstream has: all 2000, then it stops asking.
    let (_, _, v) = get(&app, "/api/live/bars?symbol=BTCUSD&interval=1m&count=3000").await;
    assert_eq!(v["time"].as_array().unwrap().len(), 2000);
    let calls = fake.calls.load(Ordering::SeqCst);
    let _ = get(&app, "/api/live/bars?symbol=BTCUSD&interval=1m&count=2999").await;
    assert_eq!(
        fake.calls.load(Ordering::SeqCst),
        calls,
        "same clock: no refresh, no paging"
    );
}

#[tokio::test]
async fn bad_live_requests() {
    let app = test_app(Fake::new(), NOW);
    for q in [
        "symbol=ACME&interval=1m",
        "symbol=BTCUSD&interval=5s",
        "symbol=BTCUSD&interval=1m&count=0",
        "symbol=BTCUSD&interval=1m&count=3001",
    ] {
        let (s, _, _) = get(&app, &format!("/api/live/bars?{q}")).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{q}");
    }
}

struct Down;

impl Upstream for Down {
    fn candles(
        &self,
        _: &'static str,
        _: &'static str,
        _: Option<i64>,
        _: usize,
    ) -> Fut<Vec<Candle>> {
        Box::pin(async { None })
    }
    fn trades(&self, _: &'static str) -> Fut<Vec<Trade>> {
        Box::pin(async { None })
    }
}

#[tokio::test]
async fn unreachable_upstream_falls_back_to_simulated() {
    let app = test_app(Arc::new(Down), NOW);
    let (s, src, v) = get(&app, "/api/live/bars?symbol=ETHUSD&interval=1h&count=100").await;
    assert_eq!((s, src.as_str()), (StatusCode::OK, "simulated"));
    assert_eq!(v["close"].as_array().unwrap().len(), 100);
}

#[tokio::test]
async fn stream_sends_bars_and_new_trades() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = test_app(Fake::new(), NOW);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let url = format!(
        "ws://{addr}/api/live/stream?symbol=BTCUSD&interval=1m&since={}",
        NOW - 3 * MIN
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let mut times = Vec::new();
    let mut trades = 0;
    // The first tick catches up from `since`.
    for _ in 0..5 {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let v: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        match v["type"].as_str() {
            Some("bar") => times.push(v["time"].as_i64().unwrap()),
            Some("trades") => trades += v["time"].as_array().unwrap().len(),
            _ => panic!("unexpected {v}"),
        }
    }
    assert_eq!(&times[..4], &[NOW - 3 * MIN, NOW - 2 * MIN, NOW - MIN, NOW]);
    assert!(
        trades == 0 || trades == 2,
        "only trades after the stream started: {trades}"
    );
}
