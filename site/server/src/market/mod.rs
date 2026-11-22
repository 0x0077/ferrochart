//! Synthetic market data: a deterministic price path per symbol, and the
//! bars, trades, order books and derived series cut from it.

pub mod altbars;
pub mod bars;
pub mod flow;
pub mod metrics;
pub mod noise;
pub mod series;
pub mod symbols;
