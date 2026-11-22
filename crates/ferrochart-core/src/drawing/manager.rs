//! The drawings of a chart: registry of tool factories, z order, selection,
//! visibility, locking, and the undo/redo stack.

use crate::chart::ChartEvent;
use crate::data::{DrawingId, PaneId};
use crate::error::{Error, Result};
use crate::geom::RectPx;

use super::{Anchor, AnchorCount, DrawingData, DrawingStyle, DrawingTool, MAX_ANCHORS, Param};

/// Builds a drawing from its saved form.
pub type DrawingFactory = fn(&DrawingData) -> Result<Box<dyn DrawingTool>>;

/// Undo history length.
const MAX_HISTORY: usize = 256;

pub(crate) struct Entry {
    pub(crate) id: DrawingId,
    pub(crate) pane: PaneId,
    pub(crate) tool: Box<dyn DrawingTool>,
    pub(crate) visible: bool,
    pub(crate) locked: bool,
    /// Screen bounds of the last frame (empty when nothing was drawn): the
    /// coarse hit test.
    pub(crate) bbox: RectPx,
}

/// An undoable change.
enum Command {
    Add {
        id: DrawingId,
        pane: PaneId,
        z: usize,
        data: DrawingData,
    },
    Remove {
        id: DrawingId,
        pane: PaneId,
        z: usize,
        data: DrawingData,
        visible: bool,
        locked: bool,
    },
    Move {
        id: DrawingId,
        before: Vec<Anchor>,
        after: Vec<Anchor>,
    },
    Restyle {
        id: DrawingId,
        before: DrawingData,
        after: DrawingData,
    },
}

/// A local change of a drawing, for drawing sync.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalChange {
    Added(DrawingId),
    Modified(DrawingId),
    Visible(DrawingId),
    Locked(DrawingId),
    Removed(DrawingId),
}

/// A drawing being created by clicks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Creating {
    pub(crate) id: DrawingId,
    pub(crate) pane: PaneId,
    /// The anchor that follows the pointer.
    pub(crate) step: usize,
    /// Where the button went down for this step; releasing it far away
    /// counts as a click (press-drag-release creation).
    pub(crate) down: Option<crate::geom::PointPx>,
    /// How the drawing is created.
    pub(crate) count: AnchorCount,
    /// Freehand: where the last anchor was added.
    pub(crate) last: crate::geom::PointPx,
}

/// The drawings of a chart (`Chart::drawings`).
///
/// Changes made through the manager (and by the user with the mouse and
/// keyboard) are undoable: adding, removing, moving and restyling a drawing.
/// Selection, visibility, locking and z order are not part of the history.
///
/// # Examples
///
/// ```
/// use ferrochart_core::chart::{SeriesData, SeriesOptions};
/// use ferrochart_core::drawing::{Anchor, DrawingData};
/// use ferrochart_core::frame::SeriesKind;
/// use ferrochart_core::testing::random_walk;
/// use ferrochart_core::{Chart, ChartOptions};
///
/// let mut chart = Chart::new(ChartOptions::default());
/// let pane = chart.main_pane();
/// let candles = chart.add_series(pane, SeriesKind::Candlestick, SeriesOptions::default())?;
/// chart.set_data(candles, SeriesData::Ohlc(random_walk(1, 100, 0, 60_000)?))?;
///
/// let line = DrawingData::new("trend_line", vec![
///     Anchor { time: 0, price: 100.0 },
///     Anchor { time: 50 * 60_000, price: 110.0 },
/// ]);
/// let id = chart.drawings().add(pane, &line)?;
/// assert_eq!(chart.drawings().ids().count(), 1);
/// chart.drawings().undo();
/// assert!(chart.drawings().data(id).is_none());
/// chart.drawings().redo();
/// assert!(chart.drawings().data(id).is_some());
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub struct DrawingManager {
    factories: Vec<(&'static str, DrawingFactory)>,
    pub(crate) entries: Vec<Entry>,
    next_id: u64,
    panes: Vec<PaneId>,
    selected: Option<DrawingId>,
    undo: Vec<Command>,
    redo: Vec<Command>,
    /// Kind armed by `start_tool`: the next click on a plot creates one.
    pub(crate) armed: Option<&'static str>,
    pub(crate) creating: Option<Creating>,
    /// Anchors of the drawing being dragged, before the drag.
    pub(crate) edit_before: Vec<Anchor>,
    pub(crate) events: Vec<ChartEvent>,
    /// Something visible changed since the last frame.
    pub(crate) dirty: bool,
    /// Local changes not yet sent to the sync group.
    pub(crate) sync_log: Vec<LocalChange>,
    /// Record `sync_log` (the chart shares drawings).
    pub(crate) sync_log_on: bool,
    /// Applying a received change: do not record it.
    pub(crate) sync_mute: bool,
}

impl core::fmt::Debug for DrawingManager {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DrawingManager")
            .field("drawings", &self.entries.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl DrawingManager {
    pub(crate) fn new(main: PaneId) -> Self {
        let mut m = Self {
            factories: Vec::new(),
            entries: Vec::new(),
            next_id: 1,
            panes: vec![main],
            selected: None,
            undo: Vec::new(),
            redo: Vec::new(),
            armed: None,
            creating: None,
            edit_before: Vec::new(),
            events: Vec::new(),
            dirty: false,
            sync_log: Vec::new(),
            sync_log_on: false,
            sync_mute: false,
        };
        for kind in super::BUILTIN_KINDS {
            m.register(kind, super::builtin);
        }
        m
    }

    /// Registers (or replaces) the factory of `kind`.
    pub fn register(&mut self, kind: &'static str, factory: DrawingFactory) {
        match self.factories.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, f)) => *f = factory,
            None => self.factories.push((kind, factory)),
        }
    }

    /// Registered kinds.
    pub fn kinds(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.factories.iter().map(|(k, _)| *k)
    }

    fn factory(&self, kind: &str) -> Result<(&'static str, DrawingFactory)> {
        self.factories
            .iter()
            .find(|(k, _)| *k == kind)
            .copied()
            .ok_or(Error::InvalidArgument("unknown drawing kind"))
    }

    fn build(&self, data: &DrawingData) -> Result<Box<dyn DrawingTool>> {
        let (_, factory) = self.factory(&data.kind)?;
        if data.anchors.iter().any(|a| !a.price.is_finite()) {
            return Err(Error::InvalidArgument("anchor prices must be finite"));
        }
        let mut tool = factory(data)?;
        let count = tool.anchor_count();
        if tool.anchors().len() < count.min() {
            return Err(Error::InvalidArgument("too few anchors"));
        }
        if !matches!(count, AnchorCount::Fixed(_)) && tool.anchors().len() > count.max() {
            return Err(Error::InvalidArgument("too many anchors"));
        }
        tool.finish();
        Ok(tool)
    }

    /// Builds the tool of a saved drawing (validates it like `add`).
    pub(crate) fn build_saved(&self, data: &DrawingData) -> Result<Box<dyn DrawingTool>> {
        self.build(data)
    }

    /// Replaces every drawing with `items` (bottom to top): `(pane, tool,
    /// visible, locked)`. Cancels the tool in progress, clears the selection
    /// and the history (`Chart::load_state`).
    pub(crate) fn replace_all(&mut self, items: Vec<(PaneId, Box<dyn DrawingTool>, bool, bool)>) {
        self.cancel_creation();
        self.armed = None;
        for e in core::mem::take(&mut self.entries) {
            self.after_removal(e.id);
        }
        for (pane, tool, visible, locked) in items {
            let id = DrawingId(self.next_id);
            self.next_id += 1;
            let z = self.entries.len();
            self.insert(id, pane, z, tool);
            if let Some(e) = self.entries.last_mut() {
                e.visible = visible;
                e.locked = locked;
            }
            self.emit(ChartEvent::DrawingCreated(id));
        }
        self.undo.clear();
        self.redo.clear();
        self.dirty = true;
    }

    pub(crate) fn pane_added(&mut self, pane: PaneId) {
        self.panes.push(pane);
    }

    /// Removes the drawings of a removed pane. The history is cleared: its
    /// commands may refer to them.
    pub(crate) fn pane_removed(&mut self, pane: PaneId) {
        self.panes.retain(|&p| p != pane);
        if self.creating.is_some_and(|c| c.pane == pane) {
            self.cancel_creation();
        }
        let mut removed = false;
        let mut k = 0;
        while k < self.entries.len() {
            if self.entries[k].pane == pane {
                let id = self.entries.remove(k).id;
                self.after_removal(id);
                removed = true;
            } else {
                k += 1;
            }
        }
        if removed {
            self.undo.clear();
            self.redo.clear();
        }
    }

    fn position(&self, id: DrawingId) -> Result<usize> {
        self.entries
            .iter()
            .position(|e| e.id == id)
            .ok_or(Error::UnknownId)
    }

    pub(crate) fn entry(&self, id: DrawingId) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub(crate) fn entry_mut(&mut self, id: DrawingId) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Reports `e`, noting created, modified and removed drawings for sync.
    fn emit(&mut self, e: ChartEvent) {
        match e {
            ChartEvent::DrawingCreated(id) => self.log(LocalChange::Added(id)),
            ChartEvent::DrawingModified(id) => self.log(LocalChange::Modified(id)),
            ChartEvent::DrawingRemoved(id) => self.log(LocalChange::Removed(id)),
            _ => {}
        }
        self.events.push(e);
    }

    fn log(&mut self, c: LocalChange) {
        if self.sync_log_on && !self.sync_mute {
            self.sync_log.push(c);
        }
    }

    /// Adds a received drawing on top (not undoable).
    pub(crate) fn add_synced(
        &mut self,
        pane: PaneId,
        data: &DrawingData,
        visible: bool,
        locked: bool,
    ) -> Result<DrawingId> {
        if !self.panes.contains(&pane) {
            return Err(Error::UnknownId);
        }
        let tool = self.build(data)?;
        let id = DrawingId(self.next_id);
        self.next_id += 1;
        let z = self.entries.len();
        self.insert(id, pane, z, tool);
        if let Some(e) = self.entries.get_mut(z) {
            e.visible = visible;
            e.locked = locked;
        }
        self.emit(ChartEvent::DrawingCreated(id));
        Ok(id)
    }

    /// Replaces a drawing with received data (not undoable).
    pub(crate) fn update_synced(&mut self, id: DrawingId, data: &DrawingData) -> Result<()> {
        if self.data(id).ok_or(Error::UnknownId)? == *data {
            return Ok(());
        }
        self.replace(id, data)?;
        self.emit(ChartEvent::DrawingModified(id));
        Ok(())
    }

    /// Removes a drawing removed elsewhere (not undoable).
    pub(crate) fn remove_synced(&mut self, id: DrawingId) -> Result<()> {
        let z = self.position(id)?;
        if self.creating.is_some_and(|c| c.id == id) {
            self.cancel_creation();
            return Ok(());
        }
        self.entries.remove(z);
        self.after_removal(id);
        Ok(())
    }

    fn record(&mut self, cmd: Command) {
        if self.undo.len() >= MAX_HISTORY {
            self.undo.remove(0);
        }
        self.undo.push(cmd);
        self.redo.clear();
    }

    fn insert(&mut self, id: DrawingId, pane: PaneId, z: usize, tool: Box<dyn DrawingTool>) {
        let z = z.min(self.entries.len());
        self.entries.insert(
            z,
            Entry {
                id,
                pane,
                tool,
                visible: true,
                locked: false,
                bbox: RectPx::default(),
            },
        );
        self.dirty = true;
    }

    fn after_removal(&mut self, id: DrawingId) {
        if self.selected == Some(id) {
            self.selected = None;
            self.events.push(ChartEvent::DrawingSelected(None));
        }
        self.emit(ChartEvent::DrawingRemoved(id));
        self.dirty = true;
    }

    /// Adds a drawing on `pane` (on top of the others).
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] for an unknown pane;
    /// - [`Error::InvalidArgument`] for an unregistered kind, non-finite
    ///   prices, too few anchors, or parameters the factory refuses.
    pub fn add(&mut self, pane: PaneId, data: &DrawingData) -> Result<DrawingId> {
        if !self.panes.contains(&pane) {
            return Err(Error::UnknownId);
        }
        let tool = self.build(data)?;
        let id = DrawingId(self.next_id);
        self.next_id += 1;
        let z = self.entries.len();
        let data = tool.save();
        self.insert(id, pane, z, tool);
        self.record(Command::Add { id, pane, z, data });
        self.emit(ChartEvent::DrawingCreated(id));
        Ok(id)
    }

    /// Removes a drawing.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn remove(&mut self, id: DrawingId) -> Result<()> {
        let z = self.position(id)?;
        if self.creating.is_some_and(|c| c.id == id) {
            self.cancel_creation();
            return Ok(());
        }
        let e = self.entries.remove(z);
        self.record(Command::Remove {
            id,
            pane: e.pane,
            z,
            data: e.tool.save(),
            visible: e.visible,
            locked: e.locked,
        });
        self.after_removal(id);
        Ok(())
    }

    /// Ids of all drawings, bottom to top.
    pub fn ids(&self) -> impl Iterator<Item = DrawingId> + '_ {
        self.entries.iter().map(|e| e.id)
    }

    /// A drawing.
    pub fn get(&self, id: DrawingId) -> Option<&dyn DrawingTool> {
        self.entry(id).map(|e| e.tool.as_ref())
    }

    /// The saved form of a drawing.
    pub fn data(&self, id: DrawingId) -> Option<DrawingData> {
        self.get(id).map(DrawingTool::save)
    }

    /// The pane of a drawing.
    pub fn pane_of(&self, id: DrawingId) -> Option<PaneId> {
        self.entry(id).map(|e| e.pane)
    }

    /// Moves a drawing's anchors (undoable).
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] for an unknown drawing;
    /// - [`Error::InvalidArgument`] if the number of anchors differs or a
    ///   price is not finite.
    pub fn set_anchors(&mut self, id: DrawingId, anchors: &[Anchor]) -> Result<()> {
        let e = self.entry_mut(id).ok_or(Error::UnknownId)?;
        if anchors.len() != e.tool.anchors().len() || anchors.iter().any(|a| !a.price.is_finite()) {
            return Err(Error::InvalidArgument("anchors do not fit the drawing"));
        }
        let before = e.tool.anchors().to_vec();
        e.tool.anchors_mut().copy_from_slice(anchors);
        self.moved(id, before);
        Ok(())
    }

    /// Records a finished move of `id` from `before` (no-op if unchanged).
    pub(crate) fn moved(&mut self, id: DrawingId, before: Vec<Anchor>) {
        let Some(e) = self.entry(id) else {
            return;
        };
        let after = e.tool.anchors().to_vec();
        if after != before {
            self.record(Command::Move { id, before, after });
            self.emit(ChartEvent::DrawingModified(id));
        }
        self.dirty = true;
    }

    fn replace(&mut self, id: DrawingId, data: &DrawingData) -> Result<()> {
        let z = self.position(id)?;
        let tool = self.build(data)?;
        self.entries[z].tool = tool;
        self.dirty = true;
        Ok(())
    }

    fn restyle(&mut self, id: DrawingId, edit: impl FnOnce(&mut DrawingData)) -> Result<()> {
        let before = self.data(id).ok_or(Error::UnknownId)?;
        let mut after = before.clone();
        edit(&mut after);
        if after == before {
            return Ok(());
        }
        self.replace(id, &after)?;
        self.record(Command::Restyle { id, before, after });
        self.emit(ChartEvent::DrawingModified(id));
        Ok(())
    }

    /// Changes a drawing's style (undoable).
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn set_style(&mut self, id: DrawingId, style: DrawingStyle) -> Result<()> {
        self.restyle(id, |d| d.style = style)
    }

    /// Sets a tool-specific parameter (undoable), e.g. `"text"` of a text or
    /// `"levels"` of a Fibonacci retracement.
    ///
    /// # Errors
    ///
    /// - [`Error::UnknownId`] for an unknown drawing;
    /// - [`Error::InvalidArgument`] if the tool refuses the value; the
    ///   drawing is unchanged.
    pub fn set_param(&mut self, id: DrawingId, key: &str, value: Param) -> Result<()> {
        self.restyle(id, |d| d.set(key, value))
    }

    /// Shows or hides a drawing.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn set_visible(&mut self, id: DrawingId, visible: bool) -> Result<()> {
        let e = self.entry_mut(id).ok_or(Error::UnknownId)?;
        let changed = e.visible != visible;
        e.visible = visible;
        if !visible && self.selected == Some(id) {
            self.select(None)?;
        }
        if changed {
            self.log(LocalChange::Visible(id));
        }
        self.dirty = true;
        Ok(())
    }

    /// Whether a drawing is visible.
    pub fn is_visible(&self, id: DrawingId) -> Option<bool> {
        self.entry(id).map(|e| e.visible)
    }

    /// Locks a drawing: it can be selected and deleted, but not dragged.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn set_locked(&mut self, id: DrawingId, locked: bool) -> Result<()> {
        let e = self.entry_mut(id).ok_or(Error::UnknownId)?;
        let changed = e.locked != locked;
        e.locked = locked;
        if changed {
            self.log(LocalChange::Locked(id));
        }
        Ok(())
    }

    /// Whether a drawing is locked.
    pub fn is_locked(&self, id: DrawingId) -> Option<bool> {
        self.entry(id).map(|e| e.locked)
    }

    /// Moves a drawing above all others.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn bring_to_front(&mut self, id: DrawingId) -> Result<()> {
        let z = self.position(id)?;
        let e = self.entries.remove(z);
        self.entries.push(e);
        self.dirty = true;
        Ok(())
    }

    /// Moves a drawing below all others.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn send_to_back(&mut self, id: DrawingId) -> Result<()> {
        let z = self.position(id)?;
        let e = self.entries.remove(z);
        self.entries.insert(0, e);
        self.dirty = true;
        Ok(())
    }

    /// Selects a drawing (`None` clears the selection).
    ///
    /// # Errors
    ///
    /// [`Error::UnknownId`] for an unknown drawing.
    pub fn select(&mut self, id: Option<DrawingId>) -> Result<()> {
        if let Some(id) = id {
            self.position(id)?;
        }
        if self.selected != id {
            self.selected = id;
            self.events.push(ChartEvent::DrawingSelected(id));
            self.dirty = true;
        }
        Ok(())
    }

    /// The selected drawing.
    pub fn selected(&self) -> Option<DrawingId> {
        self.selected
    }

    /// Arms a tool: the next click on a plot starts creating a drawing of
    /// `kind`. Escape cancels.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArgument`] for an unregistered kind.
    pub fn start_tool(&mut self, kind: &str) -> Result<()> {
        let (kind, _) = self.factory(kind)?;
        self.cancel_creation();
        self.armed = Some(kind);
        Ok(())
    }

    /// Disarms the tool and drops a drawing being created.
    pub fn cancel_tool(&mut self) {
        self.cancel_creation();
        self.armed = None;
    }

    /// The armed tool, or the kind being created.
    pub fn active_tool(&self) -> Option<&'static str> {
        self.armed.or_else(|| {
            let c = self.creating?;
            Some(self.entry(c.id)?.tool.kind())
        })
    }

    /// Starts creating a drawing of the armed kind at `at` on `pane`; it is
    /// finished at once if it needs a single click.
    pub(crate) fn begin_creation(&mut self, pane: PaneId, at: Anchor, down: crate::geom::PointPx) {
        let Some(kind) = self.armed.take() else {
            return;
        };
        let Ok((_, factory)) = self.factory(kind) else {
            return;
        };
        let Ok(mut tool) = factory(&DrawingData::new(kind, Vec::new())) else {
            return;
        };
        let count = tool.anchor_count();
        // Fixed: all anchors, the second follows the pointer; Open: the
        // first and the one following the pointer; Freehand: the first.
        let n = match count {
            AnchorCount::Fixed(n) => n.max(1),
            AnchorCount::Open { .. } => 2,
            AnchorCount::Freehand => 1,
        };
        let anchors = tool.anchors_mut();
        anchors.clear();
        anchors.resize(n, at);
        let id = DrawingId(self.next_id);
        self.next_id += 1;
        let z = self.entries.len();
        self.insert(id, pane, z, tool);
        self.creating = Some(Creating {
            id,
            pane,
            step: 1,
            down: Some(down),
            count,
            last: down,
        });
        if count == AnchorCount::Fixed(n) && n == 1 {
            self.finish_creation();
        }
    }

    /// The creation click of the current step at `at`.
    pub(crate) fn creation_click(&mut self, at: Anchor, down: Option<crate::geom::PointPx>) {
        let Some(c) = self.creating else {
            return;
        };
        let Some(e) = self.entry_mut(c.id) else {
            self.creating = None;
            return;
        };
        let anchors = e.tool.anchors_mut();
        if let AnchorCount::Open { .. } = c.count {
            if let Some(a) = anchors.get_mut(c.step) {
                *a = at;
            }
            if anchors.len() >= c.count.max() {
                self.finish_open();
                return;
            }
            anchors.push(at);
            self.creating = Some(Creating {
                step: c.step + 1,
                down,
                ..c
            });
            self.dirty = true;
            return;
        }
        if c.count == AnchorCount::Freehand {
            return;
        }
        let n = c.count.min();
        if let Some(a) = anchors.get_mut(c.step) {
            *a = at;
        }
        if c.step + 1 >= n {
            self.finish_creation();
        } else {
            if let Some(a) = anchors.get_mut(c.step + 1) {
                *a = at;
            }
            self.creating = Some(Creating {
                step: c.step + 1,
                down,
                ..c
            });
        }
        self.dirty = true;
    }

    /// Freehand: adds `at` (at screen point `p`) while the button is down,
    /// once the pointer moved `step` px from the last anchor.
    pub(crate) fn freehand_move(&mut self, at: Anchor, p: crate::geom::PointPx, step: f32) {
        let Some(c) = self.creating.filter(|c| c.count == AnchorCount::Freehand) else {
            return;
        };
        if c.down.is_none() || (p.x - c.last.x).hypot(p.y - c.last.y) < step {
            return;
        }
        let Some(e) = self.entry_mut(c.id) else {
            return;
        };
        let anchors = e.tool.anchors_mut();
        if anchors.len() < MAX_ANCHORS {
            anchors.push(at);
            self.creating = Some(Creating { last: p, ..c });
            self.dirty = true;
        }
    }

    /// Freehand: the button was released; finishes the drawing (or drops it
    /// with fewer than 2 anchors).
    pub(crate) fn freehand_release(&mut self) {
        let Some(c) = self.creating.filter(|c| c.count == AnchorCount::Freehand) else {
            return;
        };
        let n = self.entry(c.id).map_or(0, |e| e.tool.anchors().len());
        if n >= c.count.min() {
            self.finish_creation();
        } else {
            self.cancel_creation();
        }
    }

    /// Whether the drawing being created is `Open` or `Freehand`.
    pub(crate) fn creating_count(&self) -> Option<AnchorCount> {
        self.creating.map(|c| c.count)
    }

    /// Open: finishes the drawing (double click, Enter), without the anchor
    /// following the pointer and repeated last anchors (the second click of
    /// a double click). Returns `false`, changing nothing, with fewer anchors
    /// than the minimum.
    pub(crate) fn finish_open(&mut self) -> bool {
        let Some(c) = self
            .creating
            .filter(|c| matches!(c.count, AnchorCount::Open { .. }))
        else {
            return false;
        };
        let Some(e) = self.entry_mut(c.id) else {
            return false;
        };
        let anchors = e.tool.anchors_mut();
        let follower = (anchors.len() > c.step).then(|| anchors.pop()).flatten();
        let mut keep = anchors.len();
        while keep >= 2 && anchors[keep - 1] == anchors[keep - 2] {
            keep -= 1;
        }
        if keep < c.count.min() {
            if let Some(a) = follower {
                anchors.push(a);
            }
            return false;
        }
        anchors.truncate(keep);
        self.finish_creation();
        true
    }

    /// Escape while creating: an `Open` drawing with enough anchors is
    /// finished, anything else is dropped.
    pub(crate) fn escape_creation(&mut self) {
        if !self.finish_open() {
            self.cancel_tool();
        }
    }

    /// Moves the anchor that follows the pointer.
    pub(crate) fn creation_move(&mut self, at: Anchor) {
        let Some(c) = self.creating.filter(|c| c.count != AnchorCount::Freehand) else {
            return;
        };
        if let Some(a) = self
            .entry_mut(c.id)
            .and_then(|e| e.tool.anchors_mut().get_mut(c.step))
        {
            *a = at;
            self.dirty = true;
        }
    }

    fn finish_creation(&mut self) {
        let Some(c) = self.creating.take() else {
            return;
        };
        let Ok(z) = self.position(c.id) else {
            return;
        };
        self.entries[z].tool.finish();
        let data = self.entries[z].tool.save();
        self.record(Command::Add {
            id: c.id,
            pane: c.pane,
            z,
            data,
        });
        self.emit(ChartEvent::DrawingCreated(c.id));
        let _ = self.select(Some(c.id));
        self.dirty = true;
    }

    fn cancel_creation(&mut self) {
        if let Some(c) = self.creating.take() {
            self.entries.retain(|e| e.id != c.id);
            self.dirty = true;
        }
    }

    /// Whether there is something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Clears the undo/redo history.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Undoes the last change; `false` if there was none.
    pub fn undo(&mut self) -> bool {
        self.cancel_creation();
        let Some(cmd) = self.undo.pop() else {
            return false;
        };
        let inverse = self.apply(cmd, true);
        if let Some(c) = inverse {
            self.redo.push(c);
        }
        true
    }

    /// Redoes the last undone change; `false` if there was none.
    pub fn redo(&mut self) -> bool {
        self.cancel_creation();
        let Some(cmd) = self.redo.pop() else {
            return false;
        };
        if let Some(c) = self.apply(cmd, false) {
            self.undo.push(c);
        }
        true
    }

    /// Applies `cmd` backwards (`undo`) or forwards and returns it for the
    /// other stack; `None` if it no longer applies.
    fn apply(&mut self, cmd: Command, undo: bool) -> Option<Command> {
        match cmd {
            Command::Add { id, pane, z, data } if undo => {
                let pos = self.position(id).ok()?;
                self.entries.remove(pos);
                self.after_removal(id);
                Some(Command::Add { id, pane, z, data })
            }
            Command::Add { id, pane, z, data } => {
                self.restore(id, pane, z, &data, true, false)?;
                Some(Command::Add { id, pane, z, data })
            }
            Command::Remove {
                id,
                pane,
                z,
                data,
                visible,
                locked,
            } => {
                if undo {
                    self.restore(id, pane, z, &data, visible, locked)?;
                } else {
                    let pos = self.position(id).ok()?;
                    self.entries.remove(pos);
                    self.after_removal(id);
                }
                Some(Command::Remove {
                    id,
                    pane,
                    z,
                    data,
                    visible,
                    locked,
                })
            }
            Command::Move { id, before, after } => {
                let target = if undo { &before } else { &after };
                let e = self.entry_mut(id)?;
                let anchors = e.tool.anchors_mut();
                anchors.clear();
                anchors.extend_from_slice(target);
                self.emit(ChartEvent::DrawingModified(id));
                self.dirty = true;
                Some(Command::Move { id, before, after })
            }
            Command::Restyle { id, before, after } => {
                let target = if undo { &before } else { &after };
                self.replace(id, target).ok()?;
                self.emit(ChartEvent::DrawingModified(id));
                Some(Command::Restyle { id, before, after })
            }
        }
    }

    fn restore(
        &mut self,
        id: DrawingId,
        pane: PaneId,
        z: usize,
        data: &DrawingData,
        visible: bool,
        locked: bool,
    ) -> Option<()> {
        if !self.panes.contains(&pane) {
            return None;
        }
        let tool = self.build(data).ok()?;
        self.insert(id, pane, z, tool);
        if let Some(e) = self.entry_mut(id) {
            e.visible = visible;
            e.locked = locked;
        }
        self.emit(ChartEvent::DrawingCreated(id));
        Some(())
    }
}
