//! Writes `tests/fixtures/input.csv`, the OHLCV input of the reference data
//! (`docs/book/src/indicators.md`). Rust's `random_walk` uses transcendental
//! functions whose last bit may differ from numpy's, so the reference reads
//! this file instead of porting the generator.
//!
//! Regenerate with
//! `cargo test -p ferrochart-indicators --test fixtures_input -- --ignored`.

use std::fmt::Write as _;

use ferrochart_core::testing::random_walk;

/// Bars in the input.
const BARS: usize = 1000;

#[test]
#[ignore = "regenerates a checked-in fixture"]
fn write_input() {
    let d = random_walk(42, BARS, 1_704_067_200_000, 3_600_000).unwrap();
    let v = d.volume.unwrap();
    let mut out = String::from("time,open,high,low,close,volume\n");
    for (i, (t, vol)) in d.time.iter().zip(&v).enumerate() {
        // `{:?}` prints the shortest string that parses back to the same f64.
        let (o, h, l, c) = (d.open[i], d.high[i], d.low[i], d.close[i]);
        writeln!(out, "{t},{o:?},{h:?},{l:?},{c:?},{vol:?}").unwrap();
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/input.csv");
    std::fs::write(path, out).unwrap();
}
