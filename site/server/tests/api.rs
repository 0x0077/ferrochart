//! HTTP API, static site and live stream, through the real router.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use ferrochart_site_server::{Config, app};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// 2025-11-21 12:00 UTC, a Friday.
const NOW: i64 = 1_763_726_400_000;

fn test_app(site: Option<PathBuf>) -> Router {
    app(Config {
        site_dir: site,
        cors: false,
        max_streams: 4,
        clock: Arc::new(|| NOW),
    })
}

async fn get(app: &Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let resp = app
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, body)
}

async fn json(app: &Router, uri: &str) -> (StatusCode, Value) {
    let (s, _, b) = get(app, uri).await;
    (s, serde_json::from_slice(&b).unwrap())
}

#[tokio::test]
async fn health_and_symbols() {
    let app = test_app(None);
    let (s, v) = json(&app, "/api/health").await;
    assert_eq!((s, v["ok"].as_bool()), (StatusCode::OK, Some(true)));
    let (_, v) = json(&app, "/api/symbols").await;
    let ids: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["id"].as_str())
        .collect();
    assert!(ids.contains(&"BTCUSD") && ids.contains(&"CNA"));
}

#[tokio::test]
async fn bars_by_count_end_at_now() {
    let app = test_app(None);
    let (s, v) = json(&app, "/api/bars?symbol=BTCUSD&interval=1m&count=300").await;
    assert_eq!(s, StatusCode::OK);
    let time = v["time"].as_array().unwrap();
    assert_eq!(time.len(), 300);
    assert_eq!(time.last().and_then(Value::as_i64), Some(NOW));
    for col in ["open", "high", "low", "close", "volume"] {
        assert_eq!(v[col].as_array().map(Vec::len), Some(300), "{col}");
    }
    // A session market: 1000 bars back across nights and the weekend.
    let (_, v) = json(&app, "/api/bars?symbol=ES&interval=5m&count=1000").await;
    assert_eq!(v["time"].as_array().map(Vec::len), Some(1000));
}

#[tokio::test]
async fn history_is_cacheable_and_stable() {
    let app = test_app(None);
    let uri = format!(
        "/api/bars?symbol=ETHUSD&interval=1h&from={}&to={}",
        NOW - 10 * 86_400_000,
        NOW - 86_400_000
    );
    let (s, h, a) = get(&app, &uri).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h["cache-control"].to_str().unwrap().contains("max-age"));
    let (_, _, b) = get(&app, &uri).await;
    assert_eq!(a, b);
    let (_, h, _) = get(&app, "/api/bars?symbol=ETHUSD&interval=1h&count=10").await;
    assert_eq!(h["cache-control"], "no-store");
}

#[tokio::test]
async fn bad_requests() {
    let app = test_app(None);
    for uri in [
        "/api/bars?symbol=NOPE",
        "/api/bars?interval=7m",
        "/api/bars?count=0",
        "/api/bars?count=999999",
        "/api/bars?interval=1s&from=0",
        "/api/trades?span=86400000000",
        "/api/depth?levels=1000",
        "/api/depth?every=1",
    ] {
        let (s, v) = json(&app, uri).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{uri}");
        assert!(v["error"].is_string(), "{uri}");
    }
    let (s, _) = json(&app, "/api/metrics/nope").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn trades_depth_and_metrics() {
    let app = test_app(None);
    let (_, v) = json(&app, "/api/trades?symbol=BTCUSD&span=60000").await;
    let n = v["time"].as_array().unwrap().len();
    assert!(n > 100, "{n} trades");
    assert_eq!(v["side"].as_array().map(Vec::len), Some(n));

    let (_, v) = json(&app, "/api/depth?symbol=BTCUSD&count=30&levels=20&step=4").await;
    assert_eq!(v["time"].as_array().map(Vec::len), Some(30));
    assert_eq!(v["prices"][0].as_array().map(Vec::len), Some(40));

    let (_, list) = json(&app, "/api/metrics").await;
    for m in list.as_array().unwrap() {
        let name = m["name"].as_str().unwrap();
        let (s, v) = json(
            &app,
            &format!("/api/metrics/{name}?symbol=BTCUSD&interval=4h&count=50"),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{name}");
        assert_eq!(v["time"].as_array().map(Vec::len), Some(50), "{name}");
        for col in m["columns"].as_array().unwrap() {
            assert_eq!(
                v[col.as_str().unwrap()].as_array().map(Vec::len),
                Some(50),
                "{name}"
            );
        }
    }
}

#[tokio::test]
async fn static_site() {
    let dir = std::env::temp_dir().join(format!("fc-site-test-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("_next/static")).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>home</h1>").unwrap();
    std::fs::write(dir.join("thumb.html"), "<h1>thumb</h1>").unwrap();
    std::fs::write(dir.join("404.html"), "<h1>missing</h1>").unwrap();
    std::fs::write(dir.join("_next/static/a.js"), "1").unwrap();
    let app = test_app(Some(dir.clone()));

    let (s, h, b) = get(&app, "/").await;
    assert_eq!((s, b.as_slice()), (StatusCode::OK, &b"<h1>home</h1>"[..]));
    assert_eq!(h["cache-control"], "no-cache");
    assert!(
        h["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("wasm-unsafe-eval")
    );
    assert_eq!(h["x-content-type-options"], "nosniff");
    let (s, _, b) = get(&app, "/thumb").await;
    assert_eq!((s, b.as_slice()), (StatusCode::OK, &b"<h1>thumb</h1>"[..]));
    let (s, _, b) = get(&app, "/nowhere").await;
    assert_eq!(
        (s, b.as_slice()),
        (StatusCode::NOT_FOUND, &b"<h1>missing</h1>"[..])
    );
    let (s, _, _) = get(&app, "/../Cargo.toml").await;
    assert_ne!(s, StatusCode::OK);
    let (_, h, _) = get(&app, "/_next/static/a.js").await;
    assert!(h["cache-control"].to_str().unwrap().contains("immutable"));
    // The API wins over files.
    let (s, _) = json(&app, "/api/health").await;
    assert_eq!(s, StatusCode::OK);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn live_stream_sends_bars_and_trades() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // A real clock: the stream advances with time.
    let app = app(Config {
        max_streams: 1,
        ..Config::default()
    });
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let url = format!("ws://{addr}/api/stream?symbol=BTCUSD&interval=1m");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    let (mut bar, mut trades) = (false, false);
    for _ in 0..40 {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let v: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        match v["type"].as_str() {
            Some("bar") => bar = v["close"].is_f64(),
            Some("trades") => trades = !v["time"].as_array().unwrap().is_empty(),
            _ => panic!("unexpected message {v}"),
        }
        if bar && trades {
            break;
        }
    }
    assert!(bar && trades);
    // The only slot is taken.
    let second = tokio_tungstenite::connect_async(&url).await;
    assert!(second.is_err());
}

#[tokio::test]
async fn series_events_forecast_and_alt_bars() {
    let app = test_app(None);
    let (s, v) = json(&app, "/api/series").await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "equityCurve")
    );

    let (s, v) = json(
        &app,
        "/api/series/equityCurve?symbol=ACME&interval=1d&count=100&ahead=10",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["time"].as_array().unwrap().len(), 110);
    assert_eq!(v["strategy"].as_array().unwrap().len(), 110);

    let (s, v) = json(&app, "/api/series/equityCurve?interval=1q&count=12").await;
    assert_eq!(
        (s, v["time"].as_array().unwrap().len()),
        (StatusCode::OK, 12)
    );
    let (s, _) = json(&app, "/api/series/nope").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = json(&app, "/api/series/equityCurve?interval=2d").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    let (s, v) = json(&app, "/api/events/news?interval=1h&count=5000").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        v["time"].as_array().unwrap().len(),
        v["label"].as_array().unwrap().len()
    );

    let (s, v) = json(
        &app,
        "/api/forecast?symbol=ETHUSD&ahead=30&quantiles=10,50,90",
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["p50"].as_array().unwrap().len(), 31);

    for q in [
        "type=tick&size=100",
        "type=volume",
        "type=dollar",
        "type=delta&interval=1m",
        "type=lineBreak&interval=1h&count=2000",
    ] {
        let (s, v) = json(&app, &format!("/api/altbars?{q}")).await;
        assert_eq!(s, StatusCode::OK, "{q}");
        assert!(v["time"].as_array().unwrap().len() > 5, "{q}");
    }
    let (s, _) = json(&app, "/api/altbars?type=renko").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
