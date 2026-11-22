//! A uniform grid over data coordinates for hit testing points that are not
//! sorted by x (scatter, cells, XY lines): built when data changes, queried
//! without allocating.
//!

/// Points bucketed into `nx × ny` equal cells over their extent.
#[derive(Clone, Debug, Default)]
pub(crate) struct GridIndex {
    x0: f64,
    y0: f64,
    /// Cells per data unit (0 for a zero-width extent).
    kx: f64,
    ky: f64,
    nx: usize,
    ny: usize,
    /// `items[starts[c]..starts[c + 1]]` are the points of cell `c`.
    starts: Vec<u32>,
    items: Vec<u32>,
}

/// Largest grid side.
const MAX_SIDE: usize = 1024;

impl GridIndex {
    /// The index of the first `n` points of `xs`, `ys` (points with a
    /// non-finite coordinate are left out). `None` when there is nothing to
    /// index.
    pub fn build(xs: &[f64], ys: &[f64], n: usize) -> Option<Self> {
        let n = n.min(xs.len()).min(ys.len());
        let ok = |i: usize| xs[i].is_finite() && ys[i].is_finite();
        let (mut x0, mut x1, mut y0, mut y1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        let mut count = 0_usize;
        for i in (0..n).filter(|&i| ok(i)) {
            x0 = x0.min(xs[i]);
            x1 = x1.max(xs[i]);
            y0 = y0.min(ys[i]);
            y1 = y1.max(ys[i]);
            count += 1;
        }
        if count == 0 || u32::try_from(n).is_err() {
            return None;
        }
        // About two points per cell.
        let side = ((count as f64 / 2.0).sqrt().ceil() as usize).clamp(1, MAX_SIDE);
        let k = |lo: f64, hi: f64| {
            if hi > lo {
                side as f64 / (hi - lo)
            } else {
                0.0
            }
        };
        let mut g = Self {
            x0,
            y0,
            kx: k(x0, x1),
            ky: k(y0, y1),
            nx: side,
            ny: side,
            starts: vec![0; side * side + 1],
            items: vec![0; count],
        };
        for i in (0..n).filter(|&i| ok(i)) {
            let c = g.cell(xs[i], ys[i]);
            g.starts[c + 1] += 1;
        }
        for c in 0..side * side {
            g.starts[c + 1] += g.starts[c];
        }
        let mut fill = g.starts.clone();
        for i in (0..n).filter(|&i| ok(i)) {
            let c = g.cell(xs[i], ys[i]);
            g.items[fill[c] as usize] = i as u32;
            fill[c] += 1;
        }
        Some(g)
    }

    fn col(&self, x: f64) -> usize {
        (((x - self.x0) * self.kx).floor().max(0.0) as usize).min(self.nx - 1)
    }

    fn row(&self, y: f64) -> usize {
        (((y - self.y0) * self.ky).floor().max(0.0) as usize).min(self.ny - 1)
    }

    fn cell(&self, x: f64, y: f64) -> usize {
        self.row(y) * self.nx + self.col(x)
    }

    /// Calls `f` with every indexed point whose cell meets the box
    /// `x0..=x1` × `y0..=y1` (a superset of the points inside it).
    pub fn query(&self, (x0, x1): (f64, f64), (y0, y1): (f64, f64), mut f: impl FnMut(usize)) {
        if !(x0 <= x1 && y0 <= y1) {
            return;
        }
        let (c0, c1) = (self.col(x0), self.col(x1));
        let (r0, r1) = (self.row(y0), self.row(y1));
        for r in r0..=r1 {
            let a = self.starts[r * self.nx + c0] as usize;
            let b = self.starts[r * self.nx + c1 + 1] as usize;
            for &i in &self.items[a..b] {
                f(i as usize);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        /// A query returns every point inside its box.
        #[test]
        fn query_finds_all_points_in_the_box(
            pts in proptest::collection::vec((-100.0_f64..100.0, -100.0_f64..100.0), 1..300),
            bx in (-120.0_f64..120.0, 0.0_f64..80.0),
            by in (-120.0_f64..120.0, 0.0_f64..80.0),
        ) {
            let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
            let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
            let g = GridIndex::build(&xs, &ys, xs.len()).unwrap();
            let (xr, yr) = ((bx.0, bx.0 + bx.1), (by.0, by.0 + by.1));
            let mut found = vec![false; xs.len()];
            g.query(xr, yr, |i| found[i] = true);
            for i in 0..xs.len() {
                let inside = (xr.0..=xr.1).contains(&xs[i]) && (yr.0..=yr.1).contains(&ys[i]);
                prop_assert!(!inside || found[i]);
            }
        }
    }

    #[test]
    fn leaves_out_the_tail_and_non_finite_points() {
        let g = GridIndex::build(&[0.0, f64::NAN, 2.0, 3.0], &[0.0, 1.0, 2.0, 3.0], 3).unwrap();
        let mut seen = Vec::new();
        g.query((-10.0, 10.0), (-10.0, 10.0), |i| seen.push(i));
        seen.sort_unstable();
        assert_eq!(seen, [0, 2]);
        assert!(GridIndex::build(&[f64::NAN], &[0.0], 1).is_none());
    }
}
