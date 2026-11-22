//! The sparse "time column × price level" grid shared by the footprint and
//! the heatmap: columns in CSR form, levels ascending
//! within a column, two values per cell. Only the last column changes; a new
//! time unit opens a new column.

use ferrochart_core::data::Timestamp;
use ferrochart_core::error::{Error, Result};
use ferrochart_core::plugin::MinMaxIndex;

use crate::levels::PriceLevels;

/// How a value is merged into a cell that already has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Combine {
    /// Add (traded volume).
    Sum,
    /// Keep the larger (resting size of several snapshots).
    Max,
}

/// One column of a [`SparseGrid`]: levels ascending, with their two values.
#[derive(Clone, Copy, Debug)]
pub struct Column<'a> {
    /// Level numbers, ascending.
    pub levels: &'a [i32],
    /// First value of each level (footprint: sell volume; heatmap: size).
    pub a: &'a [f32],
    /// Second value of each level (footprint: buy volume; heatmap: unused).
    pub b: &'a [f32],
}

impl Column<'_> {
    /// Number of levels.
    pub fn len(&self) -> usize {
        self.levels.len()
    }

    /// Whether the column has no levels.
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// The values at `level` (0 when absent).
    pub fn at(&self, level: i32) -> (f32, f32) {
        match self.levels.binary_search(&level) {
            Ok(k) => (self.a[k], self.b[k]),
            Err(_) => (0.0, 0.0),
        }
    }
}

/// Time columns of price levels with two `f32` values each.
#[derive(Clone, Debug)]
pub struct SparseGrid {
    levels: PriceLevels,
    col_times: Vec<Timestamp>,
    /// Offset of each column's first cell in `level`/`a`/`b`.
    col_start: Vec<u32>,
    level: Vec<i32>,
    a: Vec<f32>,
    b: Vec<f32>,
    /// Price extremes of each column (cell edges), for auto scaling.
    extremes: MinMaxIndex,
}

fn reserve<T>(v: &mut Vec<T>) -> Result<()> {
    v.try_reserve(1).map_err(|_| Error::OutOfMemory)
}

impl SparseGrid {
    pub(crate) fn new(levels: PriceLevels) -> Self {
        Self {
            levels,
            col_times: Vec::new(),
            col_start: Vec::new(),
            level: Vec::new(),
            a: Vec::new(),
            b: Vec::new(),
            extremes: MinMaxIndex::new(),
        }
    }

    /// The price grid.
    pub fn price_levels(&self) -> PriceLevels {
        self.levels
    }

    /// Number of columns.
    pub fn columns(&self) -> usize {
        self.col_times.len()
    }

    /// Number of cells in all columns.
    pub fn cells(&self) -> usize {
        self.level.len()
    }

    /// Column start times, strictly increasing.
    pub fn times(&self) -> &[Timestamp] {
        &self.col_times
    }

    fn bounds(&self, i: usize) -> (usize, usize) {
        let start = self.col_start[i] as usize;
        let end = self
            .col_start
            .get(i + 1)
            .map_or(self.level.len(), |&s| s as usize);
        (start, end)
    }

    /// Column `i`; `None` if out of range.
    pub fn column(&self, i: usize) -> Option<Column<'_>> {
        if i >= self.col_times.len() {
            return None;
        }
        let (s, e) = self.bounds(i);
        Some(Column {
            levels: &self.level[s..e],
            a: &self.a[s..e],
            b: &self.b[s..e],
        })
    }

    /// All cells as flat arrays (column `i` is `level[col_start[i]..]` up
    /// to the next column's start), for uploads.
    pub fn raw(&self) -> (&[u32], &[i32], &[f32], &[f32]) {
        (&self.col_start, &self.level, &self.a, &self.b)
    }

    /// Price extremes (cell edges) of columns `from..to`.
    pub fn extremes(&self, from: usize, to: usize) -> Option<(f64, f64)> {
        self.extremes.query(from, to)
    }

    /// Merges `(a, b)` into the cell at `level` of the column starting at
    /// `time`, opening a new column if `time` is after the last one.
    /// Returns whether a column was opened.
    ///
    /// # Errors
    ///
    /// [`Error::TimeNotIncreasing`] if `time` is before the last column;
    /// [`Error::OutOfMemory`].
    pub(crate) fn add(
        &mut self,
        time: Timestamp,
        level: i32,
        a: f32,
        b: f32,
        combine: Combine,
    ) -> Result<bool> {
        let opened = match self.col_times.last() {
            Some(&last) if time < last => {
                return Err(Error::TimeNotIncreasing {
                    index: self.col_times.len(),
                });
            }
            Some(&last) if time == last => false,
            _ => {
                let start = u32::try_from(self.level.len()).map_err(|_| Error::OutOfMemory)?;
                reserve(&mut self.col_times)?;
                reserve(&mut self.col_start)?;
                self.col_times.push(time);
                self.col_start.push(start);
                true
            }
        };
        let last = self.col_times.len() - 1;
        let (s, e) = self.bounds(last);
        match self.level[s..e].binary_search(&level) {
            Ok(k) => {
                let k = s + k;
                match combine {
                    Combine::Sum => {
                        self.a[k] += a;
                        self.b[k] += b;
                    }
                    Combine::Max => {
                        self.a[k] = self.a[k].max(a);
                        self.b[k] = self.b[k].max(b);
                    }
                }
            }
            Err(k) => {
                reserve(&mut self.level)?;
                reserve(&mut self.a)?;
                reserve(&mut self.b)?;
                self.level.insert(s + k, level);
                self.a.insert(s + k, a);
                self.b.insert(s + k, b);
            }
        }
        // The column's range: its lowest and highest level.
        let (s, e) = self.bounds(last);
        let lo = self.levels.edges(self.level[s]).0;
        let hi = self.levels.edges(self.level[e - 1]).1;
        if opened {
            if let Err(err) = self.extremes.push(lo, hi) {
                self.undo_open(s);
                return Err(err);
            }
        } else {
            self.extremes.set_last(lo, hi);
        }
        Ok(opened)
    }

    /// Drops a column just opened (after an allocation failure).
    fn undo_open(&mut self, start: usize) {
        self.col_times.pop();
        self.col_start.pop();
        self.level.truncate(start);
        self.a.truncate(start);
        self.b.truncate(start);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn columns_stay_sorted_and_sparse() {
        let mut g = SparseGrid::new(PriceLevels::new(0.5, 1).unwrap());
        assert!(g.add(60, 4, 1.0, 0.0, Combine::Sum).unwrap());
        assert!(!g.add(60, 2, 0.0, 2.0, Combine::Sum).unwrap());
        assert!(!g.add(60, 4, 1.0, 3.0, Combine::Sum).unwrap());
        assert!(g.add(120, 3, 5.0, 0.0, Combine::Max).unwrap());
        assert!(!g.add(120, 3, 4.0, 1.0, Combine::Max).unwrap());
        assert_eq!(g.columns(), 2);
        let c = g.column(0).unwrap();
        assert_eq!(c.levels, &[2, 4]);
        assert_eq!((c.a, c.b), (&[0.0, 2.0][..], &[2.0, 3.0][..]));
        assert_eq!(c.at(3), (0.0, 0.0));
        assert_eq!(g.column(1).unwrap().at(3), (5.0, 1.0));
        assert_eq!(g.extremes(0, 1), Some((0.75, 2.25)));
        assert_eq!(g.extremes(1, 2), Some((1.25, 1.75)));
        assert!(matches!(
            g.add(60, 1, 1.0, 1.0, Combine::Sum),
            Err(Error::TimeNotIncreasing { .. })
        ));
        assert!(g.column(2).is_none());
        let (start, level, _, _) = g.raw();
        assert_eq!((start, level), (&[0, 2][..], &[2, 4, 3][..]));
    }
}
