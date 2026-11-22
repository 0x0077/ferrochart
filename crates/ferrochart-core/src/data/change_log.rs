//! Per-series generation counter and change log for pull-based incremental sync.
//!
//! Consumers outside the core (GPU buffers of a renderer, binding-layer caches)
//! remember which generation they last synced and ask [`ChangeLog::changes_since`]
//! what changed since then. Skipped frames, failed renders and several renderers
//! on one chart therefore never lose updates.
//!

use super::IdxSpan;

/// Number of entries in the fixed-size log.
const CAPACITY: usize = 64;

/// What changed in a series since a given generation.
///
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::{ChangeLog, DataDelta, IdxSpan};
///
/// let mut log = ChangeLog::new();
/// let synced = log.generation();
/// log.record(IdxSpan { start: 9, end: 10 }); // update_last on bar 9
/// assert_eq!(log.changes_since(synced), DataDelta::Range(IdxSpan { start: 9, end: 10 }));
/// assert_eq!(log.changes_since(log.generation()), DataDelta::None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataDelta {
    /// Nothing changed.
    None,
    /// Every change lies within this logical index span (based on the fixed
    /// logical index origin, so it stays valid after `Prepend`). It may be larger
    /// than the exact set of changed slots, never smaller.
    Range(IdxSpan),
    /// The data was replaced (or the consumer never synced): re-upload everything.
    Full,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Entry {
    /// Oldest generation folded into this entry.
    gen_lo: u64,
    /// Newest generation folded into this entry.
    gen_hi: u64,
    /// Hull of the spans of generations `gen_lo..=gen_hi`.
    span: IdxSpan,
}

/// Generation counter plus a fixed-size log of changed logical index spans.
///
/// - Generations start at 1, so a consumer that has never synced asks with
///   `since_gen = 0` and gets [`DataDelta::Full`].
/// - A change whose span intersects or touches the newest entry is folded into it,
///   so a stream of ticks and appends at the end of a series occupies one entry.
/// - When all 64 entries are used, the two oldest are merged into their hull.
///   Results only get more conservative, never wrong.
/// - Only a `Replace` (or never having synced) produces `Full`; staying away for a
///   long time does not.
///
/// Recording and querying never allocate (the log is a fixed array), so this is
/// safe on the `update_last` hot path.
///
/// # Examples
///
/// ```
/// use ferrochart_core::data::{ChangeLog, DataDelta, IdxSpan};
///
/// let mut log = ChangeLog::new();
/// let renderer_a = log.generation();
/// log.record(IdxSpan { start: 100, end: 101 }); // tick on the last bar
/// let renderer_b = log.generation();
/// log.record(IdxSpan { start: 101, end: 103 }); // two bars appended
///
/// assert_eq!(log.changes_since(renderer_a), DataDelta::Range(IdxSpan { start: 100, end: 103 }));
/// assert_eq!(log.changes_since(0), DataDelta::Full); // never synced
///
/// log.replace();
/// assert_eq!(log.changes_since(renderer_b), DataDelta::Full);
/// ```
#[derive(Clone, Debug)]
pub struct ChangeLog {
    generation: u64,
    /// Generation of the creation or of the last `Replace`.
    base: u64,
    entries: [Entry; CAPACITY],
    /// Number of used entries, oldest first.
    len: usize,
}

impl ChangeLog {
    /// Creates a log at generation 1 with no recorded changes.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::ChangeLog;
    ///
    /// assert_eq!(ChangeLog::new().generation(), 1);
    /// ```
    pub const fn new() -> Self {
        Self {
            generation: 1,
            base: 1,
            entries: [Entry {
                gen_lo: 0,
                gen_hi: 0,
                span: IdxSpan { start: 0, end: 0 },
            }; CAPACITY],
            len: 0,
        }
    }

    /// Current generation. Increases by one on every recorded change.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::{ChangeLog, IdxSpan};
    ///
    /// let mut log = ChangeLog::new();
    /// log.record(IdxSpan { start: 0, end: 1 });
    /// assert_eq!(log.generation(), 2);
    /// ```
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Records that the slots in `span` changed (`Append`, `UpdateLast`,
    /// `Prepend`) and returns the new generation. An empty span still advances
    /// the generation but logs nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::{ChangeLog, DataDelta, IdxSpan};
    ///
    /// let mut log = ChangeLog::new();
    /// for _ in 0..1000 {
    ///     log.record(IdxSpan { start: 41, end: 42 }); // 1000 ticks on one bar
    /// }
    /// assert_eq!(log.changes_since(1), DataDelta::Range(IdxSpan { start: 41, end: 42 }));
    /// ```
    pub fn record(&mut self, span: IdxSpan) -> u64 {
        self.generation += 1;
        let generation = self.generation;
        if span.is_empty() {
            return generation;
        }
        if let Some(newest) = self.len.checked_sub(1).map(|i| &mut self.entries[i])
            && newest.span.touches(span)
        {
            newest.gen_hi = generation;
            newest.span = newest.span.hull(span);
            return generation;
        }
        if self.len == CAPACITY {
            // Fold the two oldest entries together to free a slot.
            let (a, b) = (self.entries[0], self.entries[1]);
            self.entries[1] = Entry {
                gen_lo: a.gen_lo,
                gen_hi: b.gen_hi,
                span: a.span.hull(b.span),
            };
            self.entries.copy_within(1.., 0);
            self.len -= 1;
        }
        self.entries[self.len] = Entry {
            gen_lo: generation,
            gen_hi: generation,
            span,
        };
        self.len += 1;
        generation
    }

    /// Records a `Replace`: everything changed. Consumers synced to an earlier
    /// generation get [`DataDelta::Full`]. Returns the new generation.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::{ChangeLog, DataDelta};
    ///
    /// let mut log = ChangeLog::new();
    /// let synced = log.replace();
    /// assert_eq!(log.changes_since(synced), DataDelta::None);
    /// assert_eq!(log.changes_since(synced - 1), DataDelta::Full);
    /// ```
    pub fn replace(&mut self) -> u64 {
        self.generation += 1;
        self.base = self.generation;
        self.len = 0;
        self.generation
    }

    /// What changed after generation `since_gen`. O(64), no allocation.
    ///
    /// Returns [`DataDelta::Full`] if `since_gen` is older than the last `Replace`
    /// (or the creation), or newer than the current generation (an invalid value,
    /// treated conservatively).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::data::{ChangeLog, DataDelta, IdxSpan};
    ///
    /// let mut log = ChangeLog::new();
    /// log.record(IdxSpan { start: 0, end: 10 });
    /// log.record(IdxSpan { start: 50, end: 60 });
    /// assert_eq!(log.changes_since(1), DataDelta::Range(IdxSpan { start: 0, end: 60 }));
    /// assert_eq!(log.changes_since(2), DataDelta::Range(IdxSpan { start: 50, end: 60 }));
    /// ```
    pub fn changes_since(&self, since_gen: u64) -> DataDelta {
        if since_gen < self.base || since_gen > self.generation {
            return DataDelta::Full;
        }
        let mut hull: Option<IdxSpan> = None;
        for e in &self.entries[..self.len] {
            if e.gen_hi > since_gen {
                hull = Some(hull.map_or(e.span, |h| h.hull(e.span)));
            }
        }
        hull.map_or(DataDelta::None, DataDelta::Range)
    }
}

impl Default for ChangeLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn span(start: i64, end: i64) -> IdxSpan {
        IdxSpan { start, end }
    }

    #[test]
    fn disjoint_changes_use_separate_entries() {
        let mut log = ChangeLog::new();
        log.record(span(0, 1));
        log.record(span(10, 11));
        log.record(span(5, 6));
        assert_eq!(log.len, 3);
        assert_eq!(log.changes_since(3), DataDelta::Range(span(5, 6)));
    }

    #[test]
    fn realtime_stream_stays_in_one_entry() {
        let mut log = ChangeLog::new();
        log.record(span(500, 600)); // unrelated older change
        let mut last = 999;
        for i in 0..10_000 {
            if i % 10 == 0 {
                last += 1; // append a bar
            }
            log.record(span(last, last + 1)); // tick on the last bar
        }
        assert_eq!(log.len, 2);
        assert_eq!(log.changes_since(2), DataDelta::Range(span(1000, last + 1)));
    }

    #[test]
    fn overflow_merges_oldest_and_never_goes_full() {
        let mut log = ChangeLog::new();
        for i in 0..1000 {
            log.record(span(i * 10, i * 10 + 1)); // disjoint
        }
        assert_eq!(log.len, CAPACITY);
        // Very old consumer: still a range, not Full.
        assert_eq!(log.changes_since(1), DataDelta::Range(span(0, 9991)));
        // Recent consumer: exact, unaffected by merging of old entries.
        assert_eq!(log.changes_since(1000), DataDelta::Range(span(9990, 9991)));
    }

    #[test]
    fn future_generation_is_full() {
        let log = ChangeLog::new();
        assert_eq!(log.changes_since(2), DataDelta::Full);
    }

    #[test]
    fn empty_span_advances_generation_only() {
        let mut log = ChangeLog::new();
        assert_eq!(log.record(span(3, 3)), 2);
        assert_eq!(log.changes_since(1), DataDelta::None);
    }

    #[derive(Clone, Debug)]
    enum Op {
        Change(i64, i64),
        Replace,
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            20 => (-500i64..500, 0i64..20).prop_map(|(s, l)| Op::Change(s, s + l)),
            1 => Just(Op::Replace),
        ]
    }

    proptest! {
        /// For every consumer generation, the result covers every change after it
        /// (never misses data), stays within the hull of all changes since the last
        /// replace, and is `Full` exactly when a replace happened after it.
        #[test]
        fn never_misses_a_change(ops in prop::collection::vec(op(), 0..400)) {
            let mut log = ChangeLog::new();
            // (generation, span) of every change since the last replace.
            let mut history: Vec<(u64, IdxSpan)> = Vec::new();
            let mut base = 1;
            for op in ops {
                match op {
                    Op::Change(s, e) => {
                        let g = log.record(span(s, e));
                        if s < e { history.push((g, span(s, e))); }
                    }
                    Op::Replace => { base = log.replace(); history.clear(); }
                }
            }
            for since in 0..=log.generation() + 1 {
                let got = log.changes_since(since);
                if since < base || since > log.generation() {
                    prop_assert_eq!(got, DataDelta::Full);
                    continue;
                }
                let after: Vec<IdxSpan> =
                    history.iter().filter(|(g, _)| *g > since).map(|(_, s)| *s).collect();
                match got {
                    DataDelta::None => prop_assert!(after.is_empty()),
                    DataDelta::Range(r) => {
                        prop_assert!(!after.is_empty());
                        for s in &after {
                            prop_assert!(r.start <= s.start && s.end <= r.end);
                        }
                        let all = history.iter().map(|(_, s)| *s).reduce(IdxSpan::hull);
                        let all = all.unwrap_or_default();
                        prop_assert!(all.start <= r.start && r.end <= all.end);
                    }
                    DataDelta::Full => prop_assert!(false, "unexpected Full"),
                }
            }
        }
    }
}
