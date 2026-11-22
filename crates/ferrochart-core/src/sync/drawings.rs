//! Drawing sync between charts of the same symbol.
//!
//! Drawing operations are not `Copy`, so they do not use the bus' slots:
//! they go through a bounded queue ([`MAX_OPS`] operations, the oldest are
//! dropped), and the bus keeps the current state of the group's drawings, a
//! table `SyncKey → (DrawingData, visible, locked)`. A member that fell
//! further behind than the queue reaches is rebuilt from the table.
//!
//! Each chart maps [`SyncKey`]s to its own [`DrawingId`]s. Only drawings of
//! the main pane are shared (other panes may show different indicators in
//! each chart), and received drawings go to the main pane. Received
//! operations bypass the undo stack; local undo and redo send their effect
//! like any other change, so the charts converge. Selection and the tool in
//! progress are not shared.
//!
//! Local changes are sent at the start and end of the chart's next frame
//! (and by [`Chart::take_drawing_sync_ops`]).

use std::collections::VecDeque;
use std::sync::Arc;

use super::SyncGroup;
use crate::chart::{Chart, ChartEvent};
use crate::data::DrawingId;
use crate::drawing::{DrawingData, LocalChange};
use crate::error::{Error, Result};

/// Length of the bus' operation queue and of a chart's outbox.
pub const MAX_OPS: usize = 1024;

/// Identity of a drawing within a sync group, assigned by the group's bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SyncKey(pub u64);

/// A change of a synced drawing.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DrawingSyncOp {
    /// A drawing was added (or is part of a full state).
    Add {
        /// Its key.
        key: SyncKey,
        /// Its saved form.
        data: DrawingData,
        /// Whether it is shown.
        visible: bool,
        /// Whether it is locked.
        locked: bool,
    },
    /// A drawing was moved, restyled or given new parameters.
    Update {
        /// Its key.
        key: SyncKey,
        /// Its new saved form.
        data: DrawingData,
    },
    /// A drawing was shown or hidden.
    SetVisible {
        /// Its key.
        key: SyncKey,
        /// Whether it is shown.
        visible: bool,
    },
    /// A drawing was locked or unlocked.
    SetLocked {
        /// Its key.
        key: SyncKey,
        /// Whether it is locked.
        locked: bool,
    },
    /// A drawing was removed.
    Remove {
        /// Its key.
        key: SyncKey,
    },
}

impl DrawingSyncOp {
    /// The drawing the operation applies to.
    pub fn key(&self) -> SyncKey {
        match self {
            Self::Add { key, .. }
            | Self::Update { key, .. }
            | Self::SetVisible { key, .. }
            | Self::SetLocked { key, .. }
            | Self::Remove { key } => *key,
        }
    }
}

/// A drawing of the bus' state table.
#[derive(Clone, Debug)]
struct TableEntry {
    key: SyncKey,
    data: DrawingData,
    visible: bool,
    locked: bool,
}

/// Drawing part of the bus.
#[derive(Debug, Default)]
pub(super) struct DrawingBus {
    /// High 32 bits of the keys.
    pub(super) origin: u32,
    next_key: u32,
    /// Sequence number of the last operation (0: none).
    seq: u64,
    /// `(seq, sender, op)`, oldest first.
    ops: VecDeque<(u64, u32, DrawingSyncOp)>,
    /// Current drawings, in the order they were added.
    table: Vec<TableEntry>,
}

impl DrawingBus {
    fn key(&mut self) -> SyncKey {
        self.next_key = self.next_key.wrapping_add(1);
        SyncKey(u64::from(self.origin) << 32 | u64::from(self.next_key))
    }

    /// Whether a member that saw operations up to `seen` must apply some.
    pub(super) fn pending(&self, member: u32, seen: u64) -> bool {
        if self.seq <= seen {
            return false;
        }
        self.behind(seen)
            || self
                .ops
                .iter()
                .rev()
                .take_while(|(s, ..)| *s > seen)
                .any(|(_, from, _)| *from != member)
    }

    /// Whether operations after `seen` were dropped from the queue.
    fn behind(&self, seen: u64) -> bool {
        self.ops.front().is_some_and(|(s, ..)| *s > seen + 1)
    }

    fn post(&mut self, member: u32, op: DrawingSyncOp) {
        let pos = self.table.iter().position(|e| e.key == op.key());
        match (&op, pos) {
            (
                DrawingSyncOp::Add {
                    key,
                    data,
                    visible,
                    locked,
                },
                _,
            ) => {
                let entry = TableEntry {
                    key: *key,
                    data: data.clone(),
                    visible: *visible,
                    locked: *locked,
                };
                match pos {
                    Some(i) => self.table[i] = entry,
                    None => self.table.push(entry),
                }
            }
            (DrawingSyncOp::Update { data, .. }, Some(i)) => self.table[i].data = data.clone(),
            (DrawingSyncOp::SetVisible { visible, .. }, Some(i)) => {
                self.table[i].visible = *visible;
            }
            (DrawingSyncOp::SetLocked { locked, .. }, Some(i)) => self.table[i].locked = *locked,
            (DrawingSyncOp::Remove { .. }, Some(i)) => {
                self.table.remove(i);
            }
            _ => {}
        }
        self.seq += 1;
        if self.ops.len() >= MAX_OPS {
            self.ops.pop_front();
        }
        self.ops.push_back((self.seq, member, op));
    }
}

/// Drawing sync state of a chart.
#[derive(Clone, Debug, Default)]
pub(crate) struct DrawingSyncState {
    /// Synced drawings of this chart.
    keys: Vec<(SyncKey, DrawingId)>,
    /// The group `keys` came from: kept when the chart leaves and joins the
    /// same group again, dropped for another group.
    key_group: Option<SyncGroup>,
    /// Operations this chart sent, not yet taken by the host.
    outbox: VecDeque<DrawingSyncOp>,
    /// `ChartEvent::DrawingSync` was reported and the outbox not taken since.
    reported: bool,
}

impl DrawingSyncState {
    fn id_of(&self, key: SyncKey) -> Option<DrawingId> {
        self.keys.iter().find(|(k, _)| *k == key).map(|&(_, id)| id)
    }

    fn key_of(&self, id: DrawingId) -> Option<SyncKey> {
        self.keys.iter().find(|(_, d)| *d == id).map(|&(k, _)| k)
    }

    fn forget(&mut self, key: SyncKey) {
        self.keys.retain(|(k, _)| *k != key);
    }
}

impl Chart {
    /// Joining with drawings: takes over the group's drawings, then shares
    /// the chart's own drawings of the main pane.
    pub(super) fn join_drawing_sync(&mut self, group: &SyncGroup) {
        let state = &mut self.sync.drawings;
        if !state
            .key_group
            .as_ref()
            .is_some_and(|g| Arc::ptr_eq(&g.bus, &group.bus))
        {
            state.keys.clear();
            state.key_group = Some(group.clone());
        }
        let (table, seq) = group.with(|bus| (bus.drawings.table.clone(), bus.drawings.seq));
        if let Some(link) = &mut self.sync.link {
            link.drawings_seen = seq;
        }
        self.resync_drawings(&table);
        self.drawings.sync_log_on = true;
        let main = self.main_pane();
        let creating = self.drawings.creating.map(|c| c.id);
        let local: Vec<_> = self
            .drawings
            .entries
            .iter()
            .filter(|e| e.pane == main && Some(e.id) != creating)
            .map(|e| e.id)
            .filter(|&id| self.sync.drawings.key_of(id).is_none())
            .collect();
        self.drawings
            .sync_log
            .extend(local.into_iter().map(LocalChange::Added));
        self.flush_drawing_sync();
    }

    /// Makes the synced drawings match `table`: drawings no longer in it are
    /// removed, the others added or updated. Received changes are not sent
    /// again.
    fn resync_drawings(&mut self, table: &[TableEntry]) {
        let drawings = &self.drawings;
        self.sync
            .drawings
            .keys
            .retain(|&(_, id)| drawings.entry(id).is_some());
        let gone: Vec<_> = self
            .sync
            .drawings
            .keys
            .iter()
            .filter(|(k, _)| !table.iter().any(|e| e.key == *k))
            .map(|&(k, _)| k)
            .collect();
        for key in gone {
            let _ = self.apply_drawing_sync(DrawingSyncOp::Remove { key });
        }
        for e in table {
            // A kind this chart does not know is skipped.
            let _ = self.apply_drawing_sync(DrawingSyncOp::Add {
                key: e.key,
                data: e.data.clone(),
                visible: e.visible,
                locked: e.locked,
            });
        }
    }

    /// Turns the local changes into operations: posts them to the group and
    /// keeps them for [`Chart::take_drawing_sync_ops`].
    pub(super) fn flush_drawing_sync(&mut self) {
        if self.drawings.sync_log.is_empty() {
            return;
        }
        let mut log = core::mem::take(&mut self.drawings.sync_log);
        let main = self.main_pane();
        for &change in &log {
            if let Some(op) = self.local_op(change, main) {
                self.send_drawing_op(op);
            }
        }
        log.clear();
        self.drawings.sync_log = log;
    }

    fn local_op(
        &mut self,
        change: LocalChange,
        main: crate::data::PaneId,
    ) -> Option<DrawingSyncOp> {
        let state = &mut self.sync.drawings;
        match change {
            LocalChange::Added(id) => {
                let e = self.drawings.entry(id).filter(|e| e.pane == main)?;
                if state.key_of(id).is_some() {
                    return None;
                }
                let link = self.sync.link.as_ref()?;
                let key = link.group.with(|bus| bus.drawings.key());
                state.keys.push((key, id));
                Some(DrawingSyncOp::Add {
                    key,
                    data: e.tool.save(),
                    visible: e.visible,
                    locked: e.locked,
                })
            }
            LocalChange::Modified(id) => Some(DrawingSyncOp::Update {
                key: state.key_of(id)?,
                data: self.drawings.entry(id)?.tool.save(),
            }),
            LocalChange::Visible(id) => Some(DrawingSyncOp::SetVisible {
                key: state.key_of(id)?,
                visible: self.drawings.entry(id)?.visible,
            }),
            LocalChange::Locked(id) => Some(DrawingSyncOp::SetLocked {
                key: state.key_of(id)?,
                locked: self.drawings.entry(id)?.locked,
            }),
            LocalChange::Removed(id) => {
                let key = state.key_of(id)?;
                state.forget(key);
                Some(DrawingSyncOp::Remove { key })
            }
        }
    }

    fn send_drawing_op(&mut self, op: DrawingSyncOp) {
        if let Some(link) = &self.sync.link {
            let member = link.member;
            link.group.with(|bus| bus.drawings.post(member, op.clone()));
        }
        let state = &mut self.sync.drawings;
        if state.outbox.len() >= MAX_OPS {
            state.outbox.pop_front();
        }
        state.outbox.push_back(op);
        if !state.reported {
            state.reported = true;
            self.push_event(ChartEvent::DrawingSync);
        }
    }

    /// Start of a frame: applies the drawing operations of the other members
    /// (or rebuilds from the group's table after falling behind).
    pub(super) fn pull_drawing_sync(&mut self) {
        let Some(link) = &mut self.sync.link else {
            return;
        };
        if !link.opts.drawings {
            return;
        }
        let (member, seen) = (link.member, link.drawings_seen);
        let mut ops = Vec::new();
        let mut table = None;
        let seq = link.group.with(|bus| {
            let d = &bus.drawings;
            if d.seq > seen {
                if d.behind(seen) {
                    table = Some(d.table.clone());
                } else {
                    ops.extend(
                        d.ops
                            .iter()
                            .filter(|(s, from, _)| *s > seen && *from != member)
                            .map(|(.., op)| op.clone()),
                    );
                }
            }
            d.seq
        });
        link.drawings_seen = seq;
        if let Some(table) = table {
            self.resync_drawings(&table);
        }
        for op in ops {
            // An operation on a drawing this chart cannot build is skipped.
            let _ = self.apply_drawing_sync(op);
        }
    }

    /// Takes the drawing operations this chart sent and the host has not
    /// taken yet (oldest first, at most [`MAX_OPS`]), e.g. to forward them to
    /// charts in other workers with [`Chart::apply_drawing_sync`]. A new
    /// [`ChartEvent::DrawingSync`] is reported for the next operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use ferrochart_core::drawing::{Anchor, DrawingData};
    /// use ferrochart_core::sync::{DrawingSyncOp, SyncGroup, SyncOptions};
    /// use ferrochart_core::{Chart, ChartOptions};
    ///
    /// let group = SyncGroup::new();
    /// let mut a = Chart::new(ChartOptions::default());
    /// a.join_sync(&group, SyncOptions { drawings: true, ..SyncOptions::default() });
    /// let line = DrawingData::new("horizontal_line", vec![Anchor { time: 0, price: 100.0 }]);
    /// let pane = a.main_pane();
    /// a.drawings().add(pane, &line)?;
    /// let mut ops = Vec::new();
    /// a.take_drawing_sync_ops(&mut ops);
    /// assert!(matches!(ops[..], [DrawingSyncOp::Add { .. }]));
    ///
    /// // A chart in another worker (here: not in the group) applies them.
    /// let mut b = Chart::new(ChartOptions::default());
    /// for op in ops {
    ///     b.apply_drawing_sync(op)?;
    /// }
    /// assert_eq!(b.drawings().ids().count(), 1);
    /// # Ok::<(), ferrochart_core::Error>(())
    /// ```
    pub fn take_drawing_sync_ops(&mut self, out: &mut Vec<DrawingSyncOp>) {
        self.flush_drawing_sync();
        let state = &mut self.sync.drawings;
        out.extend(state.outbox.drain(..));
        state.reported = false;
    }

    /// Applies a drawing operation of another chart, e.g. one the host
    /// forwarded from another worker. Received drawings go to the main pane;
    /// the change is not undoable and is not sent again. An `Add` of a key
    /// the chart already has updates that drawing.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] for a key the chart does not have (except for
    ///   `Add`);
    /// - [`Error::InvalidArgument`] for a drawing the chart cannot build
    ///   (unregistered kind, bad anchors or parameters).
    pub fn apply_drawing_sync(&mut self, op: DrawingSyncOp) -> Result<()> {
        self.drawings.sync_mute = true;
        let result = self.apply_drawing_op(op);
        self.drawings.sync_mute = false;
        result
    }

    fn apply_drawing_op(&mut self, op: DrawingSyncOp) -> Result<()> {
        let main = self.main_pane();
        let state = &mut self.sync.drawings;
        let id = state
            .id_of(op.key())
            .filter(|&id| self.drawings.entry(id).is_some());
        match op {
            DrawingSyncOp::Add {
                key,
                data,
                visible,
                locked,
            } => {
                if let Some(id) = id {
                    self.drawings.update_synced(id, &data)?;
                    self.drawings.set_visible(id, visible)?;
                    return self.drawings.set_locked(id, locked);
                }
                state.forget(key);
                let id = self.drawings.add_synced(main, &data, visible, locked)?;
                self.sync.drawings.keys.push((key, id));
                Ok(())
            }
            DrawingSyncOp::Update { data, .. } => self
                .drawings
                .update_synced(id.ok_or(Error::UnknownId)?, &data),
            DrawingSyncOp::SetVisible { visible, .. } => self
                .drawings
                .set_visible(id.ok_or(Error::UnknownId)?, visible),
            DrawingSyncOp::SetLocked { locked, .. } => self
                .drawings
                .set_locked(id.ok_or(Error::UnknownId)?, locked),
            DrawingSyncOp::Remove { key } => {
                state.forget(key);
                self.drawings.remove_synced(id.ok_or(Error::UnknownId)?)
            }
        }
    }

    /// The synced drawings of this chart as `Add` operations, bottom to top:
    /// the full state for [`Chart::load_drawing_sync_state`] on a chart that
    /// joins from another worker.
    pub fn drawing_sync_state(&self) -> Vec<DrawingSyncOp> {
        let state = &self.sync.drawings;
        self.drawings
            .entries
            .iter()
            .filter_map(|e| {
                Some(DrawingSyncOp::Add {
                    key: state.key_of(e.id)?,
                    data: e.tool.save(),
                    visible: e.visible,
                    locked: e.locked,
                })
            })
            .collect()
    }

    /// Makes the synced drawings match `state` (from
    /// [`Chart::drawing_sync_state`]): synced drawings not in it are removed,
    /// the others added or updated. Not undoable, not sent again.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] if an operation is not an `Add`, a key
    /// repeats, or a drawing cannot be built; the chart is unchanged.
    pub fn load_drawing_sync_state(&mut self, state: &[DrawingSyncOp]) -> Result<()> {
        let mut table = Vec::with_capacity(state.len());
        for op in state {
            let DrawingSyncOp::Add {
                key,
                data,
                visible,
                locked,
            } = op
            else {
                return Err(Error::InvalidArgument(
                    "a drawing sync state holds only Add",
                ));
            };
            if table.iter().any(|e: &TableEntry| e.key == *key) {
                return Err(Error::InvalidArgument("repeated sync key"));
            }
            self.drawings.build_saved(data)?;
            table.push(TableEntry {
                key: *key,
                data: data.clone(),
                visible: *visible,
                locked: *locked,
            });
        }
        self.resync_drawings(&table);
        Ok(())
    }
}
