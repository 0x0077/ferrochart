//! Reference data for `ferrochart_plot::compute` (`docs/book/src/plots.md`):
//! expected values from numpy 2.2 (`histogram`, `quantile`, `cumsum`) and
//! scipy 1.17 (`gaussian_kde` with the same bandwidth), plus properties.
#![allow(clippy::unwrap_used, clippy::float_cmp)]

use ferrochart_plot::compute::{Bins, cumsum, histogram, kde, quantiles};
use proptest::prelude::*;

/// 40 values from a fixed formula and an outlier.
fn data() -> Vec<f64> {
    let mut v: Vec<f64> = (0..40)
        .map(|i| {
            let i = f64::from(i);
            (i * 1.7).sin() * 3.0 + i * 0.05 + (i * 0.37).cos()
        })
        .collect();
    v.push(9.25);
    v
}

fn close(a: &[f64], b: &[f64], tol: f64) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() <= tol * y.abs().max(1.0), "{x} vs {y}");
    }
}

#[test]
fn histogram_matches_numpy() {
    let h = histogram(&data(), Bins::Count(7)).unwrap();
    close(
        &h.edges,
        &[
            -3.232_292_507_349_078_5,
            -1.449_107_863_442_067,
            0.334_076_780_464_944_27,
            2.117_261_424_371_955_4,
            3.900_446_068_278_967,
            5.683_630_712_185_978,
            7.466_815_356_092_989,
            9.25,
        ],
        1e-12,
    );
    assert_eq!(h.counts, [7.0, 9.0, 9.0, 9.0, 6.0, 0.0, 1.0]);
    let h = histogram(&data(), Bins::Width(0.5)).unwrap();
    assert_eq!(h.edges.len(), 27);
    assert_eq!((h.edges[0], h.edges[26]), (-3.5, 9.5));
    let want = [
        1, 1, 3, 2, 1, 1, 5, 4, 3, 2, 2, 2, 1, 5, 3, 2, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1,
    ];
    assert_eq!(h.counts, want.map(f64::from));
    // Equal values: ± 0.5, as numpy.
    let h = histogram(&[2.0, 2.0, f64::NAN], Bins::Count(2)).unwrap();
    assert_eq!((h.edges, h.counts), (vec![1.5, 2.0, 2.5], vec![0.0, 2.0]));
    assert_eq!(
        histogram(&[f64::NAN], Bins::Count(3)).unwrap().edges,
        Vec::<f64>::new()
    );
    assert!(histogram(&[1.0], Bins::Count(0)).is_err());
    assert!(histogram(&[1.0], Bins::Width(-1.0)).is_err());
    assert!(histogram(&[0.0, 1.0], Bins::Width(1e-9)).is_err());
    assert!(histogram(&[1.0], Bins::Count(1_000_001)).is_err());
}

#[test]
fn kde_matches_scipy() {
    let v = data();
    // The grid starts at min − 3h with h = 1.128 189 671 595 56 (R bw.nrd0).
    let (x, y) = kde(&v, None, 9).unwrap();
    close(
        &x,
        &[
            -6.616_861_522_135_759,
            -4.210_432_705_020_453_5,
            -1.804_003_887_905_149_2,
            0.602_424_929_210_155_1,
            3.008_853_746_325_460_3,
            5.415_282_563_440_765_5,
            7.821_711_380_556_069,
            10.228_140_197_671_374,
            12.634_569_014_786_68,
        ],
        1e-12,
    );
    close(
        &y,
        &[
            0.000_138_346_632_205_501_54,
            0.017_775_057_342_202_964,
            0.090_304_753_541_693_93,
            0.133_603_021_728_175_87,
            0.118_159_073_876_508_29,
            0.044_339_754_706_322_38,
            0.005_498_032_904_257_207,
            0.005_924_396_094_210_872,
            0.000_095_811_806_721_292_56,
        ],
        1e-10,
    );
    assert!(kde(&v, Some(0.0), 9).is_err());
    assert!(kde(&v, None, 1).is_err());
    assert_eq!(kde(&[f64::NAN], None, 4).unwrap().0.len(), 0);
}

#[test]
fn quantiles_and_cumsum_match_numpy() {
    let v = data();
    let q = quantiles(&v, &[0.0, 0.1, 0.25, 0.5, 0.9, 1.0]).unwrap();
    close(
        &q,
        &[
            -3.232_292_507_349_078_5,
            -2.182_782_530_241_490_3,
            -0.465_199_722_479_489_93,
            0.966_957_176_970_585_1,
            4.003_840_050_816_043,
            9.25,
        ],
        1e-12,
    );
    assert!(quantiles(&v, &[1.5]).is_err());
    assert!(quantiles(&[], &[0.5]).unwrap()[0].is_nan());
    let s = cumsum(&v);
    assert!((s[40] - 53.248_351_556_564_806).abs() < 1e-10);
}

proptest! {
    /// Every finite value lands in the bin whose edges hold it.
    #[test]
    fn histogram_bins_hold_their_values(
        v in proptest::collection::vec(prop_oneof![9 => -1e3_f64..1e3, 1 => Just(f64::NAN)], 1..200),
        n in 1_usize..50,
        w in 0.1_f64..100.0,
        by_width in any::<bool>(),
    ) {
        let h = histogram(&v, if by_width { Bins::Width(w) } else { Bins::Count(n) }).unwrap();
        let finite: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
        prop_assert_eq!(h.counts.iter().sum::<f64>(), finite.len() as f64);
        if finite.is_empty() {
            return Ok(());
        }
        prop_assert!(h.edges.windows(2).all(|e| e[0] < e[1]));
        let last = h.counts.len() - 1;
        let mut counts = vec![0.0; h.counts.len()];
        for x in finite {
            let k = (0..=last)
                .find(|&k| x >= h.edges[k] && (x < h.edges[k + 1] || (k == last && x <= h.edges[k + 1])))
                .unwrap();
            counts[k] += 1.0;
        }
        prop_assert_eq!(counts, h.counts);
    }

    /// The density integrates to about 1 over its grid.
    #[test]
    fn kde_integrates_to_one(v in proptest::collection::vec(-100.0_f64..100.0, 1..50)) {
        let (x, y) = kde(&v, None, 2001).unwrap();
        let area: f64 = x.windows(2).zip(y.windows(2)).map(|(x, y)| (x[1] - x[0]) * (y[0] + y[1]) / 2.0).sum();
        // ±3 bandwidths keep 99.73% of each kernel.
        prop_assert!((area - 0.9973).abs() < 0.003, "{area}");
    }

    /// Quantiles increase with the probability and stay within the data.
    #[test]
    fn quantiles_are_monotone(v in proptest::collection::vec(-1e6_f64..1e6, 1..100), mut p in proptest::collection::vec(0.0_f64..=1.0, 1..10)) {
        p.sort_unstable_by(f64::total_cmp);
        let q = quantiles(&v, &p).unwrap();
        prop_assert!(q.windows(2).all(|w| w[0] <= w[1]));
        let (lo, hi) = v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
        prop_assert!(q.iter().all(|&x| x >= lo && x <= hi));
    }
}
