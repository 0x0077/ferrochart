//! Multi-chart sync: crosshair time, visible time range and drawings.
//!
//! A [`SyncGroup`] is a small shared bus (`Arc<Mutex<_>>`) with one slot per
//! kind of message, holding the latest message and who sent it. A chart that
//! joined a group posts the changes the user (or the host's API calls) made to
//! it, and picks up the other members' messages when it builds a frame;
//! [`Chart::needs_redraw`] reports pending messages, so hosts drive synced
//! charts exactly like single ones. Each posted message is also reported as
//! [`ChartEvent::Sync`] for hosts that forward messages themselves (charts in
//! different workers or processes) with [`Chart::apply_sync`].
//!
//! Messages are anchored by time, not logical index, so charts of different
//! symbols or intervals stay aligned. Applying a message never posts it again,
//! so messages cannot bounce between charts.
//!
//! Drawings (opt-in, [`SyncOptions::drawings`]) travel as [`DrawingSyncOp`]s
//! through a bounded queue on the bus; see [`drawings`].

use std::sync::{Arc, Mutex, PoisonError};

use crate::chart::{Chart, ChartEvent};
use crate::data::Timestamp;
use crate::drawing::{index_of_time, time_of_index};
use crate::frame::Invalidation;

pub mod drawings;
use drawings::{DrawingBus, DrawingSyncState};
pub use drawings::{DrawingSyncOp, SyncKey};

/// A message between synced charts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SyncMessage {
    /// The crosshair is at `time` (extrapolated past the data), or was
    /// removed (`None`).
    Crosshair {
        /// Time under the crosshair.
        time: Option<Timestamp>,
    },
    /// The plot shows times `from..to` (left and right edges, interpolated
    /// between bars and extrapolated past the data).
    VisibleRange {
        /// Time at the left edge.
        from: Timestamp,
        /// Time at the right edge.
        to: Timestamp,
    },
}

impl SyncMessage {
    /// Slot of the message in the bus.
    fn slot(self) -> usize {
        match self {
            Self::Crosshair { .. } => 0,
            Self::VisibleRange { .. } => 1,
        }
    }
}

/// What a chart shares with its group.
///
/// # Examples
///
/// ```
/// use ferrochart_core::sync::SyncOptions;
///
/// let crosshair_only = SyncOptions { visible_range: false, ..SyncOptions::default() };
/// assert!(crosshair_only.crosshair);
/// assert!(!crosshair_only.drawings);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncOptions {
    /// Share the crosshair time.
    pub crosshair: bool,
    /// Share the visible time range (pan and zoom).
    pub visible_range: bool,
    /// Share the drawings of the main pane (charts of the same symbol);
    /// off by default.
    pub drawings: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            crosshair: true,
            visible_range: true,
            drawings: false,
        }
    }
}

impl SyncOptions {
    fn allows(self, slot: usize) -> bool {
        match slot {
            0 => self.crosshair,
            _ => self.visible_range,
        }
    }
}

const SLOTS: usize = 2;

#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    /// Sequence number of `msg` (0: never posted).
    seq: u64,
    /// The member that posted it.
    from: u32,
    msg: Option<SyncMessage>,
}

#[derive(Debug, Default)]
struct Bus {
    members: u32,
    seq: u64,
    slots: [Slot; SLOTS],
    drawings: DrawingBus,
}

/// A group of synced charts. Cloning gives another
/// handle to the same group.
///
/// # Examples
///
/// ```
/// use ferrochart_core::sync::{SyncGroup, SyncOptions};
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let group = SyncGroup::new();
/// let mut a = Chart::new(ChartOptions::default());
/// let mut b = Chart::new(ChartOptions::default());
/// a.join_sync(&group, SyncOptions::default());
/// b.join_sync(&group, SyncOptions { visible_range: false, ..SyncOptions::default() });
/// // Moving the crosshair over `a` now shows it on `b` after `b` builds a frame.
/// ```
#[derive(Clone, Debug, Default)]
pub struct SyncGroup {
    bus: Arc<Mutex<Bus>>,
}

impl SyncGroup {
    /// Creates an empty group.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty group whose drawing [`SyncKey`]s start with
    /// `origin` (the high 32 bits). Hosts that forward drawing operations
    /// between groups (charts in different workers) give each group its own
    /// origin, so that keys do not collide.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::sync::SyncGroup;
    ///
    /// let worker_a = SyncGroup::with_origin(1);
    /// let worker_b = SyncGroup::with_origin(2);
    /// # let _ = (worker_a, worker_b);
    /// ```
    pub fn with_origin(origin: u32) -> Self {
        let group = Self::default();
        group.with(|bus| bus.drawings.origin = origin);
        group
    }

    fn with<R>(&self, f: impl FnOnce(&mut Bus) -> R) -> R {
        // A panic while holding the lock cannot leave the bus inconsistent
        // (every update is a plain store), so a poisoned lock is still usable.
        let mut bus = self.bus.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut bus)
    }

    fn join(&self) -> u32 {
        self.with(|bus| {
            bus.members = bus.members.wrapping_add(1);
            bus.members
        })
    }

    /// Posts `msg` from `member`. Removing the crosshair only clears the
    /// member's own crosshair (another chart may already show one). Returns
    /// whether the message was posted.
    fn post(&self, member: u32, msg: SyncMessage) -> bool {
        self.with(|bus| {
            let slot = &mut bus.slots[msg.slot()];
            if msg == (SyncMessage::Crosshair { time: None }) && slot.from != member {
                return false;
            }
            bus.seq += 1;
            *slot = Slot {
                seq: bus.seq,
                from: member,
                msg: Some(msg),
            };
            true
        })
    }
}

/// A chart's membership in a group.
#[derive(Clone, Debug)]
pub(crate) struct SyncLink {
    group: SyncGroup,
    member: u32,
    opts: SyncOptions,
    /// Sequence numbers already seen, per slot.
    seen: [u64; SLOTS],
    /// Last drawing operation seen.
    drawings_seen: u64,
}

/// Sync state of a chart.
#[derive(Clone, Debug)]
pub(crate) struct SyncState {
    pub(crate) link: Option<SyncLink>,
    /// Crosshair time received from another chart.
    pub(crate) remote_crosshair: Option<Timestamp>,
    /// The crosshair time this chart posted last (`None`: none shown).
    posted_crosshair: Option<Timestamp>,
    /// `(bar_spacing, right_offset)` after the last frame or applied message:
    /// a difference at the next frame is a local pan or zoom.
    view: (f64, f64),
    /// The view changed locally: post the range once the frame is laid out.
    post_view: bool,
    /// Drawing sync.
    pub(crate) drawings: DrawingSyncState,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            link: None,
            remote_crosshair: None,
            posted_crosshair: None,
            view: (f64::NAN, f64::NAN),
            post_view: false,
            drawings: DrawingSyncState::default(),
        }
    }
}

impl Chart {
    /// Joins `group` (leaving the current one, if any), sharing what `opts`
    /// selects. The chart takes over the group's
    /// current crosshair and range at its next frame. With
    /// [`SyncOptions::drawings`] it first takes over the group's drawings,
    /// then shares its own (of the main pane).
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::sync::{SyncGroup, SyncOptions};
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let group = SyncGroup::new();
    /// let mut charts: Vec<Chart> = (0..4).map(|_| Chart::new(ChartOptions::default())).collect();
    /// for chart in &mut charts {
    ///     chart.join_sync(&group, SyncOptions::default());
    /// }
    /// assert_eq!(charts[0].sync_options(), Some(SyncOptions::default()));
    /// ```
    pub fn join_sync(&mut self, group: &SyncGroup, opts: SyncOptions) {
        self.leave_sync();
        self.sync.link = Some(SyncLink {
            group: group.clone(),
            member: group.join(),
            opts,
            seen: [0; SLOTS],
            drawings_seen: 0,
        });
        self.sync.view = self.view_key();
        if opts.drawings {
            self.join_drawing_sync(group);
        }
    }

    /// Leaves the group; the chart's crosshair is removed from the others.
    pub fn leave_sync(&mut self) {
        if let Some(link) = self.sync.link.take()
            && self.sync.posted_crosshair.is_some()
        {
            link.group
                .post(link.member, SyncMessage::Crosshair { time: None });
        }
        self.sync.posted_crosshair = None;
        self.drawings.sync_log_on = false;
        self.drawings.sync_log.clear();
        if self.sync.remote_crosshair.take().is_some() {
            self.invalidation = self.invalidation.max(Invalidation::Cursor);
        }
    }

    /// What the chart shares, `None` outside a group.
    pub fn sync_options(&self) -> Option<SyncOptions> {
        self.sync.link.as_ref().map(|l| l.opts)
    }

    /// Applies a message from another chart, e.g. one the host forwarded
    /// from a [`ChartEvent::Sync`] of a chart in another worker. It is not
    /// posted again. A range is ignored while the chart has no data.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::sync::SyncMessage;
    /// use ferrochart_core::frame::Invalidation;
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let mut chart = Chart::new(ChartOptions::default());
    /// # let _ = chart.needs_redraw();
    /// chart.apply_sync(SyncMessage::Crosshair { time: Some(1_700_000_000_000) });
    /// assert_ne!(chart.needs_redraw(), Invalidation::None);
    /// ```
    pub fn apply_sync(&mut self, msg: SyncMessage) {
        match msg {
            SyncMessage::Crosshair { time } => {
                if time != self.sync.remote_crosshair {
                    self.sync.remote_crosshair = time;
                    self.invalidation = self.invalidation.max(Invalidation::Cursor);
                }
            }
            SyncMessage::VisibleRange { from, to } => self.apply_range(from, to),
        }
    }

    /// Shows times `from..to` across the plot, as far as the spacing limits
    /// allow (the right edge is kept).
    fn apply_range(&mut self, from: Timestamp, to: Timestamp) {
        let (Some(a), Some(b)) = (
            index_of_time(&self.time_index, from),
            index_of_time(&self.time_index, to),
        ) else {
            return;
        };
        if b > a {
            // Re-applied until the layout settles: the plot width may change
            // with the price axis labels of the new range.
            self.time_scale.show_span(a, b);
        } else {
            let last = (self.time_index.end_index() - 1) as f64;
            self.time_scale.set_right_offset(b - last - 0.5);
        }
        self.sync.view = self.view_key();
    }

    fn view_key(&self) -> (f64, f64) {
        (
            self.time_scale.bar_spacing(),
            self.time_scale.right_offset(),
        )
    }

    /// Pending messages of other members, as an invalidation.
    pub(crate) fn sync_pending(&self) -> Invalidation {
        let Some(link) = &self.sync.link else {
            return Invalidation::None;
        };
        link.group.with(|bus| {
            let mut inv = Invalidation::None;
            if link.opts.drawings && bus.drawings.pending(link.member, link.drawings_seen) {
                inv = Invalidation::Light;
            }
            for (k, slot) in bus.slots.iter().enumerate() {
                if slot.seq > link.seen[k] && slot.from != link.member && link.opts.allows(k) {
                    inv = inv.max(if k == 0 {
                        Invalidation::Cursor
                    } else {
                        Invalidation::Light
                    });
                }
            }
            inv
        })
    }

    /// Start of a frame: notes a local pan or zoom, then applies the other
    /// members' new messages (a local pan or zoom wins over a received range).
    pub(crate) fn pull_sync(&mut self) {
        self.flush_drawing_sync();
        self.pull_drawing_sync();
        let local = !same_bits(self.view_key(), self.sync.view);
        self.sync.post_view |= local;
        let Some(link) = &mut self.sync.link else {
            return;
        };
        let mut msgs = [None; SLOTS];
        link.group.with(|bus| {
            for (k, slot) in bus.slots.iter().enumerate() {
                if slot.seq > link.seen[k] {
                    link.seen[k] = slot.seq;
                    if slot.from != link.member && link.opts.allows(k) {
                        msgs[k] = slot.msg;
                    }
                }
            }
        });
        for msg in msgs.into_iter().flatten() {
            if matches!(msg, SyncMessage::VisibleRange { .. }) && local {
                continue;
            }
            self.apply_sync(msg);
        }
    }

    /// Posts `msg` to the group (if it shares that kind) and reports it.
    fn post_sync(&mut self, msg: SyncMessage) {
        let Some(link) = &self.sync.link else {
            return;
        };
        if link.opts.allows(msg.slot()) && link.group.post(link.member, msg) {
            self.push_event(ChartEvent::Sync(msg));
        }
    }

    /// End of a frame: posts the local crosshair time (`None` when the
    /// pointer is not over a plot) and the range after a local pan or zoom.
    pub(crate) fn push_sync(&mut self, local_crosshair: Option<f64>) {
        self.flush_drawing_sync();
        if self.sync.link.is_none() {
            self.sync.post_view = false;
            self.sync.view = self.view_key();
            return;
        }
        let time = local_crosshair.and_then(|i| time_of_index(&self.time_index, i));
        if time != self.sync.posted_crosshair {
            self.sync.posted_crosshair = time;
            self.post_sync(SyncMessage::Crosshair { time });
        }
        if self.sync.post_view {
            let (from, to) = self.time_scale.visible_range();
            if let (Some(from), Some(to)) = (
                time_of_index(&self.time_index, from),
                time_of_index(&self.time_index, to),
            ) {
                self.post_sync(SyncMessage::VisibleRange { from, to });
            }
        }
        self.sync.post_view = false;
        self.sync.view = self.view_key();
    }

    /// Logical index of the bar a received crosshair time falls in: the last
    /// bar at or before it, extrapolated past the data.
    pub(crate) fn remote_crosshair_index(&self) -> Option<i64> {
        let t = self.sync.remote_crosshair?;
        let index = &self.time_index;
        let last = index.end_index() - 1;
        if index.is_empty() || t > index.time_at(last)? || t < index.time_at(index.first_index())? {
            return index_of_time(index, t).map(|i| i.round() as i64);
        }
        let j = index.lower_bound(t);
        Some(if index.time_at(j) == Some(t) {
            j
        } else {
            j - 1
        })
    }
}

fn same_bits(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0.to_bits() == b.0.to_bits() && a.1.to_bits() == b.1.to_bits()
}
