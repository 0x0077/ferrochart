//! Live feed over WebSocket: `/api/stream?symbol=BTCUSD&interval=1m`.
//!
//! Every 250 ms the server sends the forming bar and the trades since the
//! last message, read from the same deterministic path as the history, so a
//! chart loaded from `/api/bars` continues seamlessly:
//!
//! ```json
//! { "type": "bar", "time": 1763683200000, "open": …, "high": …, "low": …, "close": …, "volume": … }
//! { "type": "trades", "time": [...], "price": [...], "size": [...], "side": [...] }
//! ```

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;
use crate::api::{ApiError, interval, symbol};
use crate::market::bars::{self, Interval};
use crate::market::flow;
use crate::market::symbols::Symbol;

pub const TICK: Duration = Duration::from_millis(250);

#[derive(Deserialize)]
pub struct StreamQuery {
    symbol: Option<String>,
    interval: Option<String>,
    /// Send trades too. Default true.
    trades: Option<bool>,
}

pub async fn handler(
    State(st): State<AppState>,
    Query(q): Query<StreamQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let sym = symbol(q.symbol.as_deref())?;
    let iv = interval(q.interval.as_deref())?;
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
            run(socket, st, sym, iv, with_trades).await;
            drop(permit);
        })
        .into_response())
}

async fn run(
    mut socket: WebSocket,
    st: AppState,
    sym: &'static Symbol,
    iv: Interval,
    with_trades: bool,
) {
    let mut last = st.now();
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let now = st.now();
                let mut msgs = Vec::with_capacity(2);
                let start = iv.floor(now);
                if let Some([o, h, l, c, v]) = bars::bar(sym, iv, start, now) {
                    msgs.push(json!({ "type": "bar", "time": start, "open": o, "high": h, "low": l, "close": c, "volume": v }));
                }
                if with_trades && now > last {
                    let t = flow::trades(sym, last, now);
                    if !t.time.is_empty() {
                        msgs.push(json!({ "type": "trades", "time": t.time, "price": t.price, "size": t.size, "side": t.side }));
                    }
                }
                last = now;
                for m in msgs {
                    if socket.send(Message::Text(m.to_string().into())).await.is_err() {
                        return;
                    }
                }
            }
            msg = socket.recv() => match msg {
                // Clients have nothing to say; anything but a ping ends it.
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                _ => return,
            },
        }
    }
}
