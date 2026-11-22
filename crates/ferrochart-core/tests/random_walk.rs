//! Tests for `testing::random_walk`.
#![cfg(feature = "testing")]
// Test code may unwrap. `clippy.toml`'s allow-*-in-tests only
// covers `#[test]` fns, not helpers in integration tests. Single-letter OHLC names
// and index loops over parallel columns read better here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::many_single_char_names,
    clippy::needless_range_loop
)]

use ferrochart_core::Error;
use ferrochart_core::data::OhlcColumns;
use ferrochart_core::testing::random_walk;
use proptest::prelude::*;

const T0: i64 = 1_704_067_200_000; // 2024-01-01T00:00:00Z
const MINUTE: i64 = 60_000;

fn check_invariants(b: &OhlcColumns, n: usize, start: i64, interval: i64) {
    let volume = b.volume.as_ref().expect("random_walk always emits volume");
    assert_eq!(b.len(), n);
    for col in [&b.open, &b.high, &b.low, &b.close, volume] {
        assert_eq!(col.len(), n);
    }
    for i in 0..n {
        let k = i64::try_from(i).unwrap();
        assert_eq!(b.time[i], start + k * interval, "time at {i}");
        let (o, h, l, c, v) = (b.open[i], b.high[i], b.low[i], b.close[i], volume[i]);
        assert!(l.is_finite() && h.is_finite(), "finite at {i}");
        assert!(l > 0.0 && v > 0.0, "positive at {i}");
        assert!(l <= o.min(c) && o.max(c) <= h, "ohlc order at {i}");
        if i > 0 {
            assert_eq!(
                o.to_bits(),
                b.close[i - 1].to_bits(),
                "open == prev close at {i}"
            );
        }
    }
}

#[test]
fn deterministic_for_same_seed() {
    let a = random_walk(7, 5000, T0, MINUTE).unwrap();
    let b = random_walk(7, 5000, T0, MINUTE).unwrap();
    assert_eq!(a, b);
}

#[test]
fn different_seeds_differ() {
    let a = random_walk(1, 100, T0, MINUTE).unwrap();
    let b = random_walk(2, 100, T0, MINUTE).unwrap();
    assert_ne!(a.close, b.close);
}

#[test]
fn prefix_is_stable_when_n_grows() {
    let short = random_walk(3, 100, T0, MINUTE).unwrap();
    let long = random_walk(3, 1000, T0, MINUTE).unwrap();
    assert_eq!(short.close[..], long.close[..100]);
    assert_eq!(short.high[..], long.high[..100]);
}

#[test]
fn empty() {
    let b = random_walk(0, 0, T0, MINUTE).unwrap();
    assert!(b.is_empty());
    assert_eq!(b.volume, Some(Vec::new()));
}

#[test]
fn rejects_non_positive_interval() {
    for interval in [0, -1, i64::MIN] {
        assert!(matches!(
            random_walk(0, 10, T0, interval),
            Err(Error::InvalidArgument(_))
        ));
    }
}

#[test]
fn rejects_time_overflow() {
    assert!(matches!(
        random_walk(0, 3, i64::MAX - 1, 1),
        Err(Error::InvalidArgument(_))
    ));
    // Exactly reaching i64::MAX is fine.
    let b = random_walk(0, 2, i64::MAX - 1, 1).unwrap();
    assert_eq!(b.time, vec![i64::MAX - 1, i64::MAX]);
}

#[test]
fn huge_n_is_an_error_not_a_panic() {
    // Time fits in i64 (interval 1), but the allocation is impossible.
    assert_eq!(
        random_walk(0, usize::MAX / 2, i64::MIN, 1),
        Err(Error::OutOfMemory)
    );
}

#[test]
fn long_series_stays_in_range() {
    let b = random_walk(11, 200_000, T0, MINUTE).unwrap();
    check_invariants(&b, 200_000, T0, MINUTE);
    let lo = b.low.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = b.high.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // Measured on 10M bars: extremes roughly 40..400. Generous bounds, the point is
    // that mean reversion prevents drifting by orders of magnitude.
    assert!(lo > 10.0 && hi < 1000.0, "range {lo}..{hi}");
}

/// Pins the exact bit patterns of the first bars of seed 42. Golden images depend on
/// this data being identical on every platform and over time: if this test fails,
/// the generator changed and every golden image must be regenerated (with a reason
/// in the commit message).
#[test]
fn pinned_values_seed_42() {
    let b = random_walk(42, 3, T0, MINUTE).unwrap();
    let bits = |v: &[f64]| -> [u64; 3] { [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()] };
    let volume = b.volume.as_ref().unwrap();
    let actual = [
        bits(&b.open),
        bits(&b.high),
        bits(&b.low),
        bits(&b.close),
        bits(volume),
    ];
    let expected: [[u64; 3]; 5] = [
        // open:   100.0,    99.176,   98.1143
        [
            0x4059_0000_0000_0000,
            0x4058_CB43_B0AD_07A7,
            0x4058_8750_93FB_2EA9,
        ],
        // high:   100.0647, 99.3907,  99.5757
        [
            0x4059_0423_FB56_FD75,
            0x4058_D901_E72D_7BBA,
            0x4058_E4D8_42CD_26C3,
        ],
        // low:    98.8808,  98.0117,  97.891
        [
            0x4058_B85F_2FC4_F8FF,
            0x4058_80BE_F852_DFDF,
            0x4058_7905_6E31_FC0A,
        ],
        // close:  99.176,   98.1143,  98.8755
        [
            0x4058_CB43_B0AD_07A7,
            0x4058_8750_93FB_2EA9,
            0x4058_B807_B781_1AAD,
        ],
        // volume: 957.5665, 1548.4234, 660.808
        [
            0x408D_EC88_1BF3_9EA6,
            0x4098_31B1_967C_BB02,
            0x4084_A676_C3F0_DE0D,
        ],
    ];
    assert_eq!(actual, expected);
}

proptest! {
    #[test]
    fn invariants_hold(seed: u64, n in 0usize..2000, start in -1_000_000_000_000i64..4_000_000_000_000, interval in 1i64..86_400_000) {
        let b = random_walk(seed, n, start, interval).unwrap();
        check_invariants(&b, n, start, interval);
    }
}
