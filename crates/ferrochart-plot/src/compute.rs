//! Pure helpers that turn raw values into plottable columns: histograms,
//! kernel density estimates, running sums and quantiles. They do not depend
//! on [`Plot`](crate::Plot). Non-finite inputs are ignored everywhere (see
//! each function). Definitions and reference data: `docs/book/src/plots.md`.
//!

use ferrochart_core::error::{Error, Result};

/// Most bins a histogram may have.
const MAX_BINS: usize = 1_000_000;

/// How a histogram divides its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bins {
    /// This many equal bins from the smallest to the largest value.
    Count(usize),
    /// Bins of this width on a grid through 0 (edges are multiples of it).
    Width(f64),
}

/// Bin edges and counts: bin `k` is `[edges[k], edges[k + 1])`, the last one
/// closed on both sides.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Histogram {
    /// `counts.len() + 1` ascending edges (empty without values).
    pub edges: Vec<f64>,
    /// Number of values in each bin (whole numbers, as f64 for plotting).
    pub counts: Vec<f64>,
}

fn finite(values: &[f64]) -> impl Iterator<Item = f64> + '_ {
    values.iter().copied().filter(|v| v.is_finite())
}

fn extent(values: &[f64]) -> Option<(f64, f64)> {
    finite(values).fold(None, |acc, v| match acc {
        None => Some((v, v)),
        Some((a, b)) => Some((a.min(v), b.max(v))),
    })
}

/// Counts the finite `values` in `bins`.
///
/// `Count(n)`: `n` equal bins from the smallest to the largest value (when
/// all values are equal, the range is that value ± 0.5), as `numpy.histogram`.
/// `Width(w)`: edges at multiples of `w` from the one at or below the
/// smallest value until the largest value is covered (a largest value on an
/// edge falls in the last bin).
///
/// ```
/// use ferrochart_plot::compute::{Bins, histogram};
///
/// let h = histogram(&[1.0, 2.0, 2.5, 4.0], Bins::Count(3))?;
/// assert_eq!(h.edges, [1.0, 2.0, 3.0, 4.0]);
/// assert_eq!(h.counts, [1.0, 2.0, 1.0]);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
///
/// # Errors
///
/// `InvalidArgument` for zero bins, a width that is not finite and
/// positive, or more than 1 000 000 bins.
pub fn histogram(values: &[f64], bins: Bins) -> Result<Histogram> {
    match bins {
        Bins::Count(n) if n == 0 || n > MAX_BINS => {
            return Err(Error::InvalidArgument("bin count must be 1 to 1 000 000"));
        }
        Bins::Width(w) if !(w.is_finite() && w > 0.0) => {
            return Err(Error::InvalidArgument(
                "bin width must be finite and positive",
            ));
        }
        _ => {}
    }
    let Some((lo, hi)) = extent(values) else {
        return Ok(Histogram::default());
    };
    let (e0, step, n) = match bins {
        Bins::Count(n) => {
            let (lo, hi) = if hi > lo {
                (lo, hi)
            } else {
                (lo - 0.5, hi + 0.5)
            };
            (lo, (hi - lo) / n as f64, n)
        }
        Bins::Width(w) => {
            let e0 = (lo / w).floor() * w;
            let n = ((hi - e0) / w).ceil().max(1.0);
            if n.is_nan() || n > MAX_BINS as f64 {
                return Err(Error::InvalidArgument("more than 1 000 000 bins"));
            }
            (e0, w, n as usize)
        }
    };
    let edges: Vec<f64> = (0..=n)
        .map(|k| match bins {
            // The last edge is exact, not accumulated.
            Bins::Count(_) if k == n => {
                if hi > lo {
                    hi
                } else {
                    hi + 0.5
                }
            }
            _ => e0 + step * k as f64,
        })
        .collect();
    let mut counts = vec![0.0; n];
    for v in finite(values) {
        let mut k = (((v - e0) / step).floor().max(0.0) as usize).min(n - 1);
        // Rounding near edges: settle on the bin whose edges hold v.
        if v < edges[k] && k > 0 {
            k -= 1;
        } else if k + 1 < n && v >= edges[k + 1] {
            k += 1;
        }
        counts[k] += 1.0;
    }
    Ok(Histogram { edges, counts })
}

/// Sample standard deviation (n − 1).
fn std_dev(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    if v.len() < 2 {
        return 0.0;
    }
    let mean = v.iter().sum::<f64>() / n;
    (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
}

/// Quantile `p` of ascending finite `sorted` (linear interpolation,
/// "type 7").
fn quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    let Some(&last) = sorted.last() else {
        return f64::NAN;
    };
    let h = (sorted.len() - 1) as f64 * p;
    let k = h.floor() as usize;
    match sorted.get(k + 1) {
        Some(&next) => sorted[k] + (h - k as f64) * (next - sorted[k]),
        None => last,
    }
}

/// Silverman's rule of thumb for the finite values `values` (as R's
/// `bw.nrd0`): `0.9 · min(sd, IQR / 1.34) · n^(-1/5)`, falling back to the
/// standard deviation, then `|first value|`, then 1 when the spread is zero.
/// `NaN` without values.
fn silverman_bandwidth(values: &[f64]) -> f64 {
    let mut v: Vec<f64> = finite(values).collect();
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_unstable_by(f64::total_cmp);
    let sd = std_dev(&v);
    let iqr = quantile_sorted(&v, 0.75) - quantile_sorted(&v, 0.25);
    let mut lo = sd.min(iqr / 1.34);
    if lo <= 0.0 {
        lo = if sd > 0.0 {
            sd
        } else if v[0] != 0.0 {
            v[0].abs()
        } else {
            1.0
        };
    }
    0.9 * lo * (v.len() as f64).powf(-0.2)
}

/// Gaussian kernel density estimate of the finite `values` at `points`
/// evenly spaced positions from `min − 3h` to `max + 3h`, where `h` is
/// `bandwidth` or Silverman's rule (R's `bw.nrd0`). Returns `(x, density)`; the
/// density integrates to 1. Empty columns without values. O(values ·
/// points).
///
/// ```
/// use ferrochart_plot::compute::kde;
///
/// let (x, y) = kde(&[0.0], Some(1.0), 7)?;
/// assert_eq!(x, [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]);
/// assert!((y[3] - 0.398_942_28).abs() < 1e-8); // 1/√(2π)
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
///
/// # Errors
///
/// `InvalidArgument` for fewer than 2 points or a bandwidth that is not
/// finite and positive.
pub fn kde(values: &[f64], bandwidth: Option<f64>, points: usize) -> Result<(Vec<f64>, Vec<f64>)> {
    if points < 2 {
        return Err(Error::InvalidArgument("kde needs at least 2 points"));
    }
    if bandwidth.is_some_and(|h| !(h.is_finite() && h > 0.0)) {
        return Err(Error::InvalidArgument(
            "bandwidth must be finite and positive",
        ));
    }
    let Some((lo, hi)) = extent(values) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let h = bandwidth.unwrap_or_else(|| silverman_bandwidth(values));
    let n = finite(values).count() as f64;
    let (a, b) = (lo - 3.0 * h, hi + 3.0 * h);
    let step = (b - a) / (points - 1) as f64;
    let norm = 1.0 / (n * h * (2.0 * core::f64::consts::PI).sqrt());
    let xs: Vec<f64> = (0..points)
        .map(|k| {
            if k + 1 == points {
                b
            } else {
                a + step * k as f64
            }
        })
        .collect();
    let ys = xs
        .iter()
        .map(|&x| {
            finite(values)
                .map(|v| {
                    let u = (x - v) / h;
                    (-0.5 * u * u).exp()
                })
                .sum::<f64>()
                * norm
        })
        .collect();
    Ok((xs, ys))
}

/// Running sum. Non-finite values are written as NaN (a gap) and do not
/// change the sum.
///
/// ```
/// use ferrochart_plot::compute::cumsum;
///
/// let s = cumsum(&[1.0, 2.0, f64::NAN, 4.0]);
/// assert_eq!((s[0], s[1], s[3]), (1.0, 3.0, 7.0));
/// assert!(s[2].is_nan());
/// ```
pub fn cumsum(values: &[f64]) -> Vec<f64> {
    let mut sum = 0.0;
    values
        .iter()
        .map(|&v| {
            if v.is_finite() {
                sum += v;
                sum
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Quantiles `probs` (0 to 1) of the finite `values`, by linear
/// interpolation between order statistics (`numpy.quantile` default, R
/// type 7). NaN for each probability without values.
///
/// ```
/// use ferrochart_plot::compute::quantiles;
///
/// assert_eq!(quantiles(&[4.0, 1.0, 3.0, 2.0], &[0.0, 0.5, 1.0])?, [1.0, 2.5, 4.0]);
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
///
/// # Errors
///
/// `InvalidArgument` for a probability outside 0 to 1.
pub fn quantiles(values: &[f64], probs: &[f64]) -> Result<Vec<f64>> {
    if probs.iter().any(|p| !(0.0..=1.0).contains(p)) {
        return Err(Error::InvalidArgument("probabilities must be 0 to 1"));
    }
    let mut v: Vec<f64> = finite(values).collect();
    v.sort_unstable_by(f64::total_cmp);
    Ok(probs.iter().map(|&p| quantile_sorted(&v, p)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silverman_matches_r() {
        // R: bw.nrd0(1:5)
        assert!((silverman_bandwidth(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 0.973_585).abs() < 1e-6);
        assert!(silverman_bandwidth(&[f64::NAN]).is_nan());
        // No spread: |first value|.
        assert!((silverman_bandwidth(&[2.0, 2.0]) - 0.9 * 2.0 * 2f64.powf(-0.2)).abs() < 1e-12);
    }
}
