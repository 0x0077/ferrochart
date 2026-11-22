//! The `Plot` object: options, public API, layout and frame building.
//!

use ferrochart_core::drawing::{FrameBuilder, Label};
use ferrochart_core::error::{Error, Result};
use ferrochart_core::frame::{
    DrawCmd, FontId, FontStyleRef, Frame, Invalidation, LayerId, LineDash, Reveal, StrokeStyle,
    TextAlign,
};
use ferrochart_core::geom::{PointPx, Px, RectPx, SizeCss, SizePx};
use ferrochart_core::text::{StackStr, TextMeasurer, text_width};
use ferrochart_core::theme::Theme;

use crate::axis::{
    Axis, AxisFormat, AxisId, AxisKind, AxisOptions, AxisRange, TickLabel, number_format,
};
use crate::color::palette;
use crate::draw::{self, Ctx, Geometry, Scratch};
use crate::interact::{Interaction, PlotEvent};
use crate::mark::{
    LabelId, LabelOptions, MarkData, MarkEntry, MarkId, MarkKind, MarkOptions, MarkPoint, RuleId,
    RuleOptions, validate, validate_categories,
};

/// The [`Geometry`] of a plot from disjoint field borrows (a `&self`
/// method would borrow the whole plot while the frame is written).
macro_rules! geometry {
    ($p:expr) => {
        Geometry {
            x: &$p.x,
            left: &$p.left,
            right: &$p.right,
            plot: $p.layout.plot,
            dpr: $p.dpr,
            theme: &$p.theme,
        }
    };
}
pub(crate) use geometry;

/// Options of a [`Plot`].
#[derive(Clone, Debug, PartialEq)]
pub struct PlotOptions {
    /// Colors and sizes.
    pub theme: Theme,
    /// Extra fonts (TTF/OTF) for the host's text engine, as for charts.
    pub fonts: Vec<Vec<u8>>,
    /// The horizontal axis.
    pub x: AxisOptions,
    /// The left axis (hidden by default).
    pub left: AxisOptions,
    /// The right axis.
    pub right: AxisOptions,
    /// The hovered element is highlighted.
    pub hover_highlight: bool,
    /// Crosshair lines through the hovered element.
    pub crosshair: bool,
}

impl Default for PlotOptions {
    fn default() -> Self {
        Self {
            theme: Theme::light(),
            fonts: Vec::new(),
            x: AxisOptions::linear(),
            left: AxisOptions::linear().hidden(),
            right: AxisOptions::linear(),
            hover_highlight: true,
            crosshair: false,
        }
    }
}

/// Rectangles of the last layout, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlotLayout {
    /// The plot area.
    pub plot: RectPx,
    /// The x axis below the plot.
    pub x_axis: RectPx,
    /// The left axis.
    pub left_axis: RectPx,
    /// The right axis.
    pub right_axis: RectPx,
}

/// A chart on numeric, logarithmic, category or date axes.
///
/// ```
/// use ferrochart_core::geom::SizeCss;
/// use ferrochart_core::testing::MonoMeasurer;
/// use ferrochart_plot::{MarkData, MarkKind, MarkOptions, Plot, PlotOptions};
///
/// let mut plot = Plot::new(PlotOptions::default());
/// plot.resize(SizeCss { w: 600.0, h: 400.0 }, 2.0);
/// let smile = plot.add_mark(MarkKind::Line, MarkOptions::default())?;
/// let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
/// plot.set_data(smile, MarkData::xy(strikes, vec![0.31, 0.26, 0.24, 0.25, 0.29]))?;
/// plot.build_frame(0.0, &MonoMeasurer);
/// assert!(!plot.frame().layers[1].cmds.is_empty());
/// # Ok::<(), ferrochart_core::Error>(())
/// ```
pub struct Plot {
    pub(crate) theme: Theme,
    pub(crate) hover_highlight: bool,
    pub(crate) crosshair: bool,
    pub(crate) x: Axis,
    pub(crate) left: Axis,
    pub(crate) right: Axis,
    pub(crate) marks: Vec<MarkEntry>,
    next_mark: u32,
    rules: Vec<(RuleId, RuleOptions)>,
    next_rule: u32,
    labels: Vec<(LabelId, LabelOptions)>,
    next_label: u32,
    pub(crate) size: SizeCss,
    pub(crate) dpr: f32,
    frame: Frame,
    revisions: [u64; 3],
    pub(crate) invalidation: Invalidation,
    pub(crate) layout: PlotLayout,
    pub(crate) scratch: Scratch,
    pub(crate) events: Vec<PlotEvent>,
    pub(crate) input: Interaction,
    /// Scale of marks colored by value without their own scale.
    /// Stack membership or stacked data changed: stack extents need refreshing.
    stacks_dirty: bool,
    /// Gradual display; `None` draws everything.
    reveal: Option<Reveal>,
}

impl core::fmt::Debug for Plot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Plot")
            .field("marks", &self.marks.len())
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl Plot {
    /// A plot with `options` and no marks.
    pub fn new(options: PlotOptions) -> Self {
        let mut frame = Frame::default();
        frame.reserve();
        Self {
            theme: options.theme,
            hover_highlight: options.hover_highlight,
            crosshair: options.crosshair,
            x: Axis::new(options.x, true),
            left: Axis::new(options.left, false),
            right: Axis::new(options.right, false),
            marks: Vec::new(),
            next_mark: 1,
            rules: Vec::new(),
            next_rule: 1,
            labels: Vec::new(),
            next_label: 1,
            size: SizeCss::default(),
            dpr: 1.0,
            frame,
            revisions: [0; 3],
            invalidation: Invalidation::Full,
            layout: PlotLayout::default(),
            scratch: Scratch::default(),
            events: Vec::with_capacity(16),
            input: Interaction::default(),
            stacks_dirty: false,
            reveal: None,
        }
    }

    /// Sets the size in CSS pixels and the device pixel ratio.
    pub fn resize(&mut self, size: SizeCss, dpr: f32) {
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        let size = SizeCss {
            w: if size.w.is_finite() {
                size.w.max(0.0)
            } else {
                0.0
            },
            h: if size.h.is_finite() {
                size.h.max(0.0)
            } else {
                0.0
            },
        };
        #[allow(clippy::float_cmp)] // exact: unchanged values
        if size != self.size || dpr != self.dpr {
            self.size = size;
            self.dpr = dpr;
            self.invalidation = Invalidation::Full;
        }
    }

    /// Replaces the theme; marks without a color take their palette color
    /// from it.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        for (k, m) in self.marks.iter_mut().enumerate() {
            m.auto_color = palette(&self.theme, k);
        }
        self.invalidation = Invalidation::Full;
    }

    /// The theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Gradual display: marks, rules and labels are clipped to
    /// the left `round(progress × width)` of the plot area; axes and grid
    /// are whole. Rule labels and the hover wait until progress reaches 1,
    /// and an optional pen line is drawn at the cut. The host advances the
    /// progress each frame; `None` (the default) draws everything.
    ///
    /// ```
    /// use ferrochart_core::frame::Reveal;
    /// use ferrochart_plot::{Plot, PlotOptions};
    ///
    /// let mut plot = Plot::new(PlotOptions::default());
    /// plot.set_reveal(Some(Reveal { progress: 0.25, pen: None }));
    /// assert_eq!(plot.reveal().map(|r| r.progress), Some(0.25));
    /// plot.set_reveal(None);
    /// ```
    pub fn set_reveal(&mut self, reveal: Option<Reveal>) {
        if reveal != self.reveal {
            self.reveal = reveal;
            self.invalidation = self.invalidation.max(Invalidation::Light);
        }
    }

    /// The gradual display set by [`Plot::set_reveal`].
    pub fn reveal(&self) -> Option<Reveal> {
        self.reveal
    }

    /// The reveal is in progress (not finished).
    fn revealing(&self) -> bool {
        self.reveal.is_some_and(|r| !r.is_done())
    }

    pub(crate) fn axis_ref(&self, id: AxisId) -> &Axis {
        match id {
            AxisId::X => &self.x,
            AxisId::Left => &self.left,
            AxisId::Right => &self.right,
        }
    }

    pub(crate) fn axis_mut(&mut self, id: AxisId) -> &mut Axis {
        match id {
            AxisId::X => &mut self.x,
            AxisId::Left => &mut self.left,
            AxisId::Right => &mut self.right,
        }
    }

    /// The options of axis `id`.
    pub fn axis(&self, id: AxisId) -> &AxisOptions {
        &self.axis_ref(id).opts
    }

    /// Replaces the options of axis `id`; the view goes back to the
    /// configured range.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a fixed range that is not finite and increasing,
    /// or not positive on a log axis.
    pub fn set_axis(&mut self, id: AxisId, options: AxisOptions) -> Result<()> {
        if let AxisRange::Fixed(a, b) = options.range {
            if !(a.is_finite() && b.is_finite() && (a < b || b < a)) {
                return Err(Error::InvalidArgument(
                    "a fixed range needs two finite, different values",
                ));
            }
            if options.kind == AxisKind::Log && (a <= 0.0 || b <= 0.0) {
                return Err(Error::InvalidArgument("a log axis needs a positive range"));
            }
        }
        self.axis_mut(id).set_options(options);
        self.stacks_dirty = true;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Sets the category labels of axis `id` (category `k` is `labels[k]`).
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for more than 1 000 000 categories.
    pub fn set_categories(&mut self, id: AxisId, labels: &[&str]) -> Result<()> {
        if labels.len() > 1_000_000 {
            return Err(Error::InvalidArgument("too many categories"));
        }
        self.axis_mut(id).set_categories(labels);
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// The category labels of axis `id`.
    pub fn categories(&self, id: AxisId) -> &[String] {
        &self.axis_ref(id).categories
    }

    fn check_y_axis(id: AxisId) -> Result<()> {
        if id == AxisId::X {
            return Err(Error::InvalidArgument("y_axis must be Left or Right"));
        }
        Ok(())
    }

    /// Adds a mark of `kind`.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` if `options.y_axis` is `X`; `OutOfMemory`.
    pub fn add_mark(&mut self, kind: MarkKind, options: MarkOptions) -> Result<MarkId> {
        Self::check_y_axis(options.y_axis)?;
        let id = MarkId(self.next_mark);
        self.next_mark = self.next_mark.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.marks.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        let color = palette(&self.theme, self.marks.len());
        self.marks.push(MarkEntry::new(id, kind, options, color));
        self.stacks_dirty = true;
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    fn entry(&self, id: MarkId) -> Result<&MarkEntry> {
        self.marks
            .iter()
            .find(|m| m.id == id)
            .ok_or(Error::UnknownId)
    }

    fn entry_mut(&mut self, id: MarkId) -> Result<&mut MarkEntry> {
        self.marks
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or(Error::UnknownId)
    }

    /// The kind of mark `id`.
    pub fn mark_kind(&self, id: MarkId) -> Option<MarkKind> {
        self.entry(id).ok().map(|m| m.kind)
    }

    /// The options of mark `id`.
    pub fn mark_options(&self, id: MarkId) -> Option<&MarkOptions> {
        self.entry(id).ok().map(|m| &m.opts)
    }

    /// The data of mark `id`.
    pub fn mark_data(&self, id: MarkId) -> Option<&MarkData> {
        self.entry(id).ok().map(|m| &m.data)
    }

    /// Replaces the options of mark `id`.
    ///
    /// # Errors
    ///
    /// `UnknownId`; `InvalidArgument` if `options.y_axis` is `X`.
    pub fn set_mark_options(&mut self, id: MarkId, options: MarkOptions) -> Result<()> {
        Self::check_y_axis(options.y_axis)?;
        let m = self.entry_mut(id)?;
        m.opts = options;
        m.refresh_full();
        self.stacks_dirty = true;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    fn check_data(&self, m: &MarkEntry, d: &MarkData) -> Result<()> {
        validate(m.kind, d)?;
        let cat_x = self.x.opts.kind == AxisKind::Category;
        let cat_y = self.axis_ref(m.opts.y_axis).opts.kind == AxisKind::Category;
        if cat_x && !(m.horizontal()) {
            validate_categories(&d.x)?;
        }
        if cat_y && (m.horizontal() || m.kind == MarkKind::Cells) {
            validate_categories(&d.y)?;
        }
        if m.kind == MarkKind::Waterfall {
            let cats = if m.horizontal() { &d.y } else { &d.x };
            validate_categories(cats)?;
        }
        Ok(())
    }

    /// Replaces the data of mark `id`.
    ///
    /// # Errors
    ///
    /// `UnknownId`; `InvalidArgument` for columns of different lengths,
    /// non-finite x, or category columns that are not whole numbers from 0.
    /// The mark keeps its data on error.
    pub fn set_data(&mut self, id: MarkId, data: MarkData) -> Result<()> {
        self.check_data(self.entry(id)?, &data)?;
        let m = self.entry_mut(id)?;
        m.data = data;
        m.refresh_full();
        self.stacks_dirty = true;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Appends `data` to mark `id`. Columns present in the mark must be
    /// present in `data` and the other way round.
    ///
    /// # Errors
    ///
    /// As [`set_data`](Self::set_data), and `InvalidArgument` when the
    /// optional columns do not match the mark's.
    pub fn append(&mut self, id: MarkId, data: MarkData) -> Result<()> {
        self.check_data(self.entry(id)?, &data)?;
        let m = self.entry_mut(id)?;
        let d = &mut m.data;
        let same = d.y0.is_some() == data.y0.is_some()
            && d.value.is_some() == data.value.is_some()
            && d.size.is_some() == data.size.is_some()
            && d.color.is_some() == data.color.is_some()
            && d.step.is_some() == data.step.is_some()
            && d.label.is_some() == data.label.is_some();
        if !same && !d.is_empty() {
            return Err(Error::InvalidArgument(
                "appended data must have the mark's columns",
            ));
        }
        if d.is_empty() {
            *d = data;
        } else {
            fn ext<T: Clone>(a: &mut Option<Vec<T>>, b: Option<Vec<T>>) {
                if let (Some(a), Some(b)) = (a.as_mut(), b) {
                    a.extend(b);
                }
            }
            d.x.extend_from_slice(&data.x);
            d.y.extend_from_slice(&data.y);
            ext(&mut d.y0, data.y0);
            ext(&mut d.value, data.value);
            ext(&mut d.size, data.size);
            ext(&mut d.color, data.color);
            ext(&mut d.step, data.step);
            ext(&mut d.label, data.label);
        }
        m.refresh_full();
        self.stacks_dirty = true;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Replaces the last point of mark `id` (a live value, e.g. a depth
    /// update). Optional columns the mark has take the point's fields.
    ///
    /// No allocation, O(1): extents combine those of the other points
    /// with the new one; the cell size
    /// and the color-scale domain keep their values from the last
    /// `set_data`/`append`.
    ///
    /// # Errors
    ///
    /// `UnknownId`; `InvalidArgument` if the mark has no data or `x` is not
    /// finite.
    pub fn update_last(&mut self, id: MarkId, point: MarkPoint) -> Result<()> {
        let m = self.entry_mut(id)?;
        let n = m.data.len();
        if n == 0 {
            return Err(Error::InvalidArgument("the mark has no data"));
        }
        if !point.x.is_finite() {
            return Err(Error::InvalidArgument("x must be finite"));
        }
        let i = n - 1;
        let d = &mut m.data;
        d.x[i] = point.x;
        d.y[i] = point.y;
        if let Some(c) = d.y0.as_mut() {
            c[i] = point.y0;
        }
        if let Some(c) = d.value.as_mut() {
            c[i] = point.value;
        }
        if let Some(c) = d.size.as_mut() {
            c[i] = point.size;
        }
        if let Some(c) = d.color.as_mut() {
            c[i] = point.color;
        }
        m.refresh_last();
        let stacked = m.opts.stack.is_some();
        self.stacks_dirty |= stacked;
        self.invalidation = self.invalidation.max(Invalidation::Data);
        Ok(())
    }

    /// Removes mark `id`.
    ///
    /// # Errors
    ///
    /// `UnknownId`.
    pub fn remove_mark(&mut self, id: MarkId) -> Result<()> {
        let k = self
            .marks
            .iter()
            .position(|m| m.id == id)
            .ok_or(Error::UnknownId)?;
        self.marks.remove(k);
        self.stacks_dirty = true;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Ids of all marks, in drawing order.
    pub fn marks(&self) -> impl Iterator<Item = MarkId> + '_ {
        self.marks.iter().map(|m| m.id)
    }

    /// Adds a rule.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a non-finite value; `OutOfMemory`.
    pub fn add_rule(&mut self, rule: RuleOptions) -> Result<RuleId> {
        check_rule(&rule)?;
        let id = RuleId(self.next_rule);
        self.next_rule = self.next_rule.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.rules.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        self.rules.push((id, rule));
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    /// Replaces rule `id`.
    ///
    /// # Errors
    ///
    /// `UnknownId`; `InvalidArgument` as for [`add_rule`](Self::add_rule).
    pub fn update_rule(&mut self, id: RuleId, rule: RuleOptions) -> Result<()> {
        check_rule(&rule)?;
        let r = self
            .rules
            .iter_mut()
            .find(|r| r.0 == id)
            .ok_or(Error::UnknownId)?;
        r.1 = rule;
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Removes rule `id`.
    ///
    /// # Errors
    ///
    /// `UnknownId`.
    pub fn remove_rule(&mut self, id: RuleId) -> Result<()> {
        let k = self
            .rules
            .iter()
            .position(|r| r.0 == id)
            .ok_or(Error::UnknownId)?;
        self.rules.remove(k);
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Adds a label.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for non-finite coordinates or `y_axis == X`;
    /// `OutOfMemory`.
    pub fn add_label(&mut self, label: LabelOptions) -> Result<LabelId> {
        Self::check_y_axis(label.y_axis)?;
        if !(label.x.is_finite() && label.y.is_finite()) {
            return Err(Error::InvalidArgument("label coordinates must be finite"));
        }
        let id = LabelId(self.next_label);
        self.next_label = self.next_label.checked_add(1).ok_or(Error::OutOfMemory)?;
        self.labels.try_reserve(1).map_err(|_| Error::OutOfMemory)?;
        self.labels.push((id, label));
        self.invalidation = Invalidation::Full;
        Ok(id)
    }

    /// Removes label `id`.
    ///
    /// # Errors
    ///
    /// `UnknownId`.
    pub fn remove_label(&mut self, id: LabelId) -> Result<()> {
        let k = self
            .labels
            .iter()
            .position(|l| l.0 == id)
            .ok_or(Error::UnknownId)?;
        self.labels.remove(k);
        self.invalidation = Invalidation::Full;
        Ok(())
    }

    /// Every axis back to its configured range (automatic ranges follow the
    /// data again).
    pub fn fit(&mut self) {
        for id in [AxisId::X, AxisId::Left, AxisId::Right] {
            self.axis_mut(id).reset_range();
        }
        self.input.kinetic = None;
        self.view_changed();
    }

    /// Shows values `lo..hi` on axis `id` (category units on a category
    /// axis); the axis stops following the data until [`fit`](Self::fit).
    /// No `ViewChanged` event: that reports changes made by the user (a host
    /// copying a chart's range every frame would otherwise hear its own
    /// updates).
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for non-finite or equal values, or a non-positive
    /// range on a log axis.
    pub fn set_view(&mut self, id: AxisId, lo: f64, hi: f64) -> Result<()> {
        if !(lo.is_finite() && hi.is_finite() && (lo < hi || hi < lo)) {
            return Err(Error::InvalidArgument(
                "the view needs two finite, different values",
            ));
        }
        let a = self.axis_mut(id);
        if a.opts.kind == AxisKind::Log && (lo <= 0.0 || hi <= 0.0) {
            return Err(Error::InvalidArgument("a log axis needs a positive range"));
        }
        a.view = (lo.min(hi), lo.max(hi));
        a.auto = false;
        self.invalidation = self.invalidation.max(Invalidation::Light);
        Ok(())
    }

    /// The current range of axis `id`, as of the last frame for automatic
    /// ranges.
    pub fn view(&self, id: AxisId) -> (f64, f64) {
        self.axis_ref(id).view
    }

    /// Pixel position of value `v` on axis `id` in the last layout (x for
    /// the x axis, y otherwise); `None` before the first frame or for values
    /// without a position (≤ 0 on a log axis).
    pub fn value_to_px(&self, id: AxisId, v: f64) -> Option<Px> {
        let a = self.axis_ref(id);
        if a.len <= 0.0 {
            return None;
        }
        let p = a.to_px(v);
        p.is_finite().then_some(Px(p as f32))
    }

    /// Value at pixel position `p` on axis `id` in the last layout.
    pub fn px_to_value(&self, id: AxisId, p: Px) -> Option<f64> {
        let a = self.axis_ref(id);
        if a.len <= 0.0 {
            return None;
        }
        let v = a.value_at(f64::from(p.0));
        v.is_finite().then_some(v)
    }

    /// The last built layout.
    pub fn layout(&self) -> PlotLayout {
        self.layout
    }

    /// What needs redrawing (kinetic panning keeps asking for frames).
    pub fn needs_redraw(&self) -> Invalidation {
        if self.input.kinetic.is_some() {
            self.invalidation.max(Invalidation::Light)
        } else {
            self.invalidation
        }
    }

    /// The frame of the last [`build_frame`](Self::build_frame).
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// Events since the last call.
    pub fn drain_events(&mut self) -> impl Iterator<Item = PlotEvent> + '_ {
        self.events.drain(..)
    }

    /// Extents of the running sums of stacked bars and areas (positive and
    /// negative values apart for bars), by data index. No allocation once
    /// the scratch sums are as long as the longest stacked mark.
    fn refresh_stacks(&mut self) {
        if !self.stacks_dirty {
            return;
        }
        self.stacks_dirty = false;
        for m in &mut self.marks {
            m.stack_ext = None;
        }
        for k in 0..self.marks.len() {
            let (kind, Some(stack)) = (self.marks[k].kind, self.marks[k].opts.stack) else {
                continue;
            };
            let member =
                |m: &MarkEntry| m.kind == kind && m.opts.stack == Some(stack) && m.opts.visible;
            if !matches!(kind, MarkKind::Bars | MarkKind::Area)
                || !member(&self.marks[k])
                || self.marks[..k].iter().any(member)
            {
                continue;
            }
            let s = &mut self.scratch;
            s.pos.clear();
            s.neg.clear();
            let (mut lo, mut hi) = (0.0_f64, 0.0_f64);
            for m in self.marks[k..].iter().filter(|m| member(m)) {
                let vals = if m.horizontal() { &m.data.x } else { &m.data.y };
                if s.pos.len() < vals.len() {
                    s.pos.resize(vals.len(), 0.0);
                    s.neg.resize(vals.len(), 0.0);
                }
                for (i, &v) in vals.iter().enumerate() {
                    if !v.is_finite() {
                        continue;
                    }
                    if v >= 0.0 || kind == MarkKind::Area {
                        s.pos[i] += v;
                        hi = hi.max(s.pos[i]);
                        lo = lo.min(s.pos[i]);
                    } else {
                        s.neg[i] += v;
                        lo = lo.min(s.neg[i]);
                    }
                }
            }
            for m in self.marks[k..].iter_mut().filter(|m| member(m)) {
                m.stack_ext = Some((lo, hi));
            }
        }
    }

    /// Automatic ranges from the marks.
    fn apply_auto_ranges(&mut self) {
        let mut ext = [None, None, None];
        let mut cats = [0_usize; 3];
        let mut anchored = [false; 3];
        for m in self.marks.iter().filter(|m| m.opts.visible) {
            let y = if m.opts.y_axis == AxisId::Left { 1 } else { 2 };
            let (mut ex, mut ey) = (m.ext_x, m.ext_y);
            if let Some(st) = m.stack_ext {
                if m.horizontal() {
                    ex = Some(st);
                } else {
                    ey = Some(st);
                }
            }
            ext[0] = crate::mark::merge(ext[0], ex);
            ext[y] = crate::mark::merge(ext[y], ey);
            cats[0] = cats[0].max(m.cat_x);
            cats[y] = cats[y].max(m.cat_y);
            if m.anchored() {
                anchored[if m.horizontal() { 0 } else { y }] = true;
            }
        }
        for (k, id) in [AxisId::X, AxisId::Left, AxisId::Right]
            .into_iter()
            .enumerate()
        {
            let a = self.axis_mut(id);
            let n = a.category_count(cats[k]);
            a.apply_auto(ext[k], n, anchored[k]);
        }
    }

    /// Builds the frame. `now_ms` drives kinetic panning (the clock of the
    /// input events' `time_ms`). Every layer is rebuilt; only the revisions
    /// of layers whose content changed are bumped (`Cursor` bumps only the
    /// overlay). The hovered element is hit-tested again in the new layout.
    pub fn build_frame(&mut self, now_ms: f64, text: &dyn TextMeasurer) {
        self.step_kinetic(now_ms);
        self.refresh_stacks();
        let inv = self.needs_redraw();
        if inv >= Invalidation::Light {
            self.revisions[LayerId::Background as usize] += 1;
            self.revisions[LayerId::Main as usize] += 1;
        }
        if inv >= Invalidation::Cursor {
            self.revisions[LayerId::Overlay as usize] += 1;
        }
        let size = SizePx {
            w: (self.size.w * self.dpr).round(),
            h: (self.size.h * self.dpr).round(),
        };
        self.frame.begin(size, self.theme.background, inv);
        for (layer, rev) in self.frame.layers.iter_mut().zip(self.revisions) {
            layer.revision = rev;
        }
        self.invalidation = Invalidation::None;
        if size.w <= 0.0 || size.h <= 0.0 {
            return;
        }
        let font = FontStyleRef {
            font: FontId(0),
            size: self.theme.font_size.to_px(self.dpr).0,
        };
        let line_h = text.line_height(font);
        let pad = self.theme.axis_padding.to_px(self.dpr).0.round();
        self.apply_auto_ranges();
        self.layout_axes(text, font, line_h, pad, size);
        self.emit_grid();
        // Axes first: rule labels drawn with the marks cover tick labels.
        self.emit_axes(text, font, line_h, pad);
        self.emit_marks(text, font, line_h, pad);
        // Geometry moved (pan, zoom, data): what is under the pointer may
        // have changed. Reported now; drawn in this frame's overlay.
        let before = self.invalidation;
        self.update_hover();
        self.invalidation = before;
        self.emit_overlay(text, font, line_h, pad);
        if let Some(r) = self.reveal {
            r.push_pen(
                &mut self.frame,
                LayerId::Overlay,
                self.layout.plot,
                self.dpr,
            );
        }
    }

    /// The hovered element: crosshair lines through it and a highlight.
    fn emit_overlay(&mut self, text: &dyn TextMeasurer, font: FontStyleRef, line_h: f32, pad: f32) {
        let Some((hit, shape)) = self.input.hover.filter(|_| !self.revealing()) else {
            return;
        };
        let Some(m) = self.marks.iter().find(|m| m.id == hit.mark) else {
            return;
        };
        let plot = self.layout.plot;
        let ctx = Ctx {
            geo: geometry!(self),
            font,
            line_h,
            pad,
            text,
        };
        self.frame
            .push_cmd(LayerId::Overlay, DrawCmd::PushClip(plot));
        let mut b = FrameBuilder::new(&mut self.frame, LayerId::Overlay);
        if self.crosshair {
            let stroke = StrokeStyle {
                width: Px(1.0),
                color: self.theme.crosshair,
                dash: LineDash::Dashed,
            };
            let c = shape.center();
            let (x, y) = (c.x.floor() + 0.5, c.y.floor() + 0.5);
            b.line(
                PointPx { x, y: plot.y },
                PointPx {
                    x,
                    y: plot.bottom(),
                },
                stroke,
            );
            b.line(
                PointPx { x: plot.x, y },
                PointPx { x: plot.right(), y },
                stroke,
            );
        }
        if self.hover_highlight {
            draw::highlight(&ctx, m, hit.index, shape, &mut b);
        }
        self.frame.push_cmd(LayerId::Overlay, DrawCmd::PopClip);
    }

    fn layout_axes(
        &mut self,
        text: &dyn TextMeasurer,
        font: FontStyleRef,
        line_h: f32,
        pad: f32,
        size: SizePx,
    ) {
        let (w, h) = (f64::from(size.w), f64::from(size.h));
        let (line, pad) = (f64::from(line_h), f64::from(pad));
        let xv = self.x.opts.visible;
        let x_h = if xv {
            pad + line
                + pad
                + if self.x.opts.title.is_some() {
                    line + pad
                } else {
                    0.0
                }
        } else {
            pad
        };
        let titled = (self.left.opts.visible && self.left.opts.title.is_some())
            || (self.right.opts.visible && self.right.opts.title.is_some());
        let top = (pad + if titled { line + pad } else { 0.0 })
            .max(line * 0.5 + 2.0)
            .round();
        let plot_h = (h - top - x_h).max(0.0).round();
        let mut widths = [pad, pad];
        let ncat = [
            self.category_count(AxisId::X),
            self.category_count(AxisId::Left),
            self.category_count(AxisId::Right),
        ];
        let dpr = self.dpr;
        for (k, id) in [AxisId::Left, AxisId::Right].into_iter().enumerate() {
            let a = self.axis_mut(id);
            a.start = top;
            a.len = plot_h;
            a.limit_category_span(ncat[k + 1], dpr);
            a.layout_ticks(line * 2.5);
            if a.opts.visible {
                let label_w = if a.opts.kind == AxisKind::Category {
                    a.category_label_widths(text, font)
                } else {
                    a.widest_tick(text, font)
                };
                widths[k] = (f64::from(label_w) + 2.0 * pad).ceil().min(w * 0.4);
            }
        }
        let plot_x = widths[0];
        let plot_w = (w - widths[0] - widths[1]).max(0.0);
        self.x.start = plot_x;
        self.x.len = plot_w;
        // Tick spacing on x: the widest label of the range ends plus a gap.
        let spacing = match (self.x.opts.kind, self.x.opts.format) {
            (_, AxisFormat::Date { .. }) => text_width(text, "00:00:00", font) + 3.0 * line_h,
            (AxisKind::Category, _) => 0.0,
            (_, fmt) => {
                let (lo, hi) = self.x.view;
                let nf = number_format(fmt, lo, hi);
                let mut wmax: f32 = 0.0;
                for v in [lo, hi, lo.midpoint(hi)] {
                    let mut s = StackStr::<64>::new();
                    nf.write(&mut s, v);
                    wmax = wmax.max(text_width(text, s.as_str(), font));
                }
                wmax + 2.5 * line_h
            }
        };
        self.x.limit_category_span(ncat[0], dpr);
        self.x.layout_ticks(f64::from(spacing).max(line * 2.5));
        if self.x.opts.kind == AxisKind::Category {
            let maxw = f64::from(self.x.category_label_widths(text, font));
            let slot = self.x.slot_px();
            self.x.category_step = if slot > 0.0 {
                ((maxw + 2.0 * pad) / slot).ceil().max(1.0) as usize
            } else {
                1
            };
        }
        for id in [AxisId::Left, AxisId::Right] {
            let a = self.axis_mut(id);
            if a.opts.kind == AxisKind::Category {
                let slot = a.slot_px();
                a.category_step = if slot > 0.0 {
                    (line * 1.2 / slot).ceil().max(1.0) as usize
                } else {
                    1
                };
            }
        }
        let plot = RectPx {
            x: plot_x as f32,
            y: top as f32,
            w: plot_w as f32,
            h: plot_h as f32,
        };
        self.layout = PlotLayout {
            plot,
            x_axis: RectPx {
                x: plot.x,
                y: plot.bottom(),
                w: plot.w,
                h: (h - top - plot_h) as f32,
            },
            left_axis: RectPx {
                x: 0.0,
                y: plot.y,
                w: plot.x,
                h: plot.h,
            },
            right_axis: RectPx {
                x: plot.right(),
                y: plot.y,
                w: (w as f32 - plot.right()).max(0.0),
                h: plot.h,
            },
        };
    }

    /// The y axis whose ticks give the horizontal grid lines.
    pub(crate) fn grid_axis(&self) -> AxisId {
        if self.right.opts.visible || !self.left.opts.visible {
            AxisId::Right
        } else {
            AxisId::Left
        }
    }

    fn emit_grid(&mut self) {
        let plot = self.layout.plot;
        let stroke = StrokeStyle {
            width: Px(1.0),
            color: self.theme.grid,
            dash: LineDash::Solid,
        };
        let gy = self.grid_axis();
        let (xa, ya) = (
            &self.x,
            if gy == AxisId::Left {
                &self.left
            } else {
                &self.right
            },
        );
        let b = &mut FrameBuilder::new(&mut self.frame, LayerId::Background);
        if xa.opts.grid {
            for_each_grid_value(xa, |v| {
                let x = xa.to_px(v) as f32;
                if x.is_finite() && x >= plot.x && x <= plot.right() {
                    let x = x.floor() + 0.5;
                    b.line(
                        PointPx { x, y: plot.y },
                        PointPx {
                            x,
                            y: plot.bottom(),
                        },
                        stroke,
                    );
                }
            });
        }
        if ya.opts.grid {
            for_each_grid_value(ya, |v| {
                let y = ya.to_px(v) as f32;
                if y.is_finite() && y >= plot.y && y <= plot.bottom() {
                    let y = y.floor() + 0.5;
                    b.line(
                        PointPx { x: plot.x, y },
                        PointPx { x: plot.right(), y },
                        stroke,
                    );
                }
            });
        }
    }

    fn emit_marks(&mut self, text: &dyn TextMeasurer, font: FontStyleRef, line_h: f32, pad: f32) {
        let plot = self.layout.plot;
        let ctx = Ctx {
            geo: geometry!(self),
            font,
            line_h,
            pad,
            text,
        };
        let clip = self.reveal.map_or(plot, |r| r.clip(plot));
        self.frame.push_cmd(LayerId::Main, DrawCmd::PushClip(clip));
        {
            // Rule bands under the marks.
            let mut b = FrameBuilder::new(&mut self.frame, LayerId::Main);
            for (_, r) in self.rules.iter().filter(|r| r.1.to.is_some()) {
                draw::rule(&ctx, r, &mut b);
            }
        }
        draw::marks(&ctx, &self.marks, &mut self.scratch, &mut self.frame);
        let mut b = FrameBuilder::new(&mut self.frame, LayerId::Main);
        for (_, r) in self.rules.iter().filter(|r| r.1.to.is_none()) {
            draw::rule(&ctx, r, &mut b);
        }
        for (_, l) in &self.labels {
            draw::label(&ctx, l, &mut b);
        }
        self.frame.push_cmd(LayerId::Main, DrawCmd::PopClip);
        if self.revealing() {
            return;
        }
        // Rule labels on their axes, outside the clip.
        let mut b = FrameBuilder::new(&mut self.frame, LayerId::Main);
        for (_, r) in &self.rules {
            draw::rule_label(&ctx, r, &mut b);
        }
    }

    fn emit_axes(&mut self, text: &dyn TextMeasurer, font: FontStyleRef, line_h: f32, pad: f32) {
        let plot = self.layout.plot;
        let size = self.frame.size_px;
        let axis_stroke = StrokeStyle {
            width: Px(1.0),
            color: self.theme.axis_line,
            dash: LineDash::Solid,
        };
        let label = |align| Label {
            font,
            color: self.theme.text,
            align,
            bg: None,
        };
        let mut b = FrameBuilder::new(&mut self.frame, LayerId::Main);
        // x axis.
        if self.x.opts.visible {
            let y = plot.bottom().floor() + 0.5;
            b.line(
                PointPx { x: plot.x, y },
                PointPx { x: plot.right(), y },
                axis_stroke,
            );
            let ly = plot.bottom() + pad + line_h * 0.5;
            if self.x.opts.kind == AxisKind::Category {
                let n = self.x.category_count(0);
                let step = self.x.category_step.max(1);
                let (lo, hi) = self.x.view;
                let k0 = lo.floor().max(0.0) as usize;
                let k1 = (hi.ceil().max(0.0) as usize).min(n);
                for k in (k0..k1).filter(|k| k % step == 0) {
                    let Some(name) = self.x.categories.get(k) else {
                        continue;
                    };
                    let x = self.x.to_px(k as f64 + 0.5) as f32;
                    let half = text_width(text, name, font) * 0.5;
                    if x < plot.x || x > plot.right() || x - half < 0.0 || x + half > size.w {
                        continue;
                    }
                    b.text(PointPx { x, y: ly }, name, label(TextAlign::Center));
                }
            } else {
                for &t in &self.x.ticks {
                    let x = self.x.to_px(t.value) as f32;
                    if !(x >= plot.x - 0.5 && x <= plot.right() + 0.5) {
                        continue;
                    }
                    let mut s = StackStr::<64>::new();
                    self.x.write_tick(t, &mut s);
                    let half = text_width(text, s.as_str(), font) * 0.5;
                    if x - half < 0.0
                        || x + half > size.w
                        || rule_label_near(
                            &self.rules,
                            &self.x,
                            AxisId::X,
                            x,
                            half + pad,
                            text,
                            font,
                        )
                    {
                        continue;
                    }
                    b.text(PointPx { x, y: ly }, s.as_str(), label(TextAlign::Center));
                }
            }
            if let Some(title) = &self.x.opts.title {
                let y = ly + line_h + pad;
                b.text(
                    PointPx {
                        x: plot.x + plot.w * 0.5,
                        y,
                    },
                    title,
                    label(TextAlign::Center),
                );
            }
        }
        // y axes.
        for id in [AxisId::Left, AxisId::Right] {
            let a = if id == AxisId::Left {
                &self.left
            } else {
                &self.right
            };
            if !a.opts.visible {
                continue;
            }
            let (ex, tx, align) = if id == AxisId::Left {
                (plot.x.floor() - 0.5, plot.x - pad, TextAlign::Right)
            } else {
                (
                    plot.right().floor() + 0.5,
                    plot.right() + pad,
                    TextAlign::Left,
                )
            };
            b.line(
                PointPx { x: ex, y: plot.y },
                PointPx {
                    x: ex,
                    y: plot.bottom(),
                },
                axis_stroke,
            );
            let inside = |y: f32| {
                y >= plot.y + line_h * 0.5 - 1.0 && y <= plot.bottom() - line_h * 0.5 + 1.0
            };
            if a.opts.kind == AxisKind::Category {
                let n = a.category_count(0);
                let step = a.category_step.max(1);
                let (lo, hi) = a.view;
                let k0 = lo.floor().max(0.0) as usize;
                let k1 = (hi.ceil().max(0.0) as usize).min(n);
                for k in (k0..k1).filter(|k| k % step == 0) {
                    let Some(name) = a.categories.get(k) else {
                        continue;
                    };
                    let y = a.to_px(k as f64 + 0.5) as f32;
                    if y >= plot.y && y <= plot.bottom() {
                        b.text(PointPx { x: tx, y }, name, label(align));
                    }
                }
            } else {
                for &t in &a.ticks {
                    let y = a.to_px(t.value) as f32;
                    if !inside(y)
                        || rule_label_near(&self.rules, a, id, y, line_h * 0.5 + pad, text, font)
                    {
                        continue;
                    }
                    let mut s = StackStr::<64>::new();
                    a.write_tick(t, &mut s);
                    b.text(PointPx { x: tx, y }, s.as_str(), label(align));
                }
            }
            if let Some(title) = &a.opts.title {
                let y = plot.y - pad - line_h * 0.5;
                let (x, al) = if id == AxisId::Left {
                    (plot.x, TextAlign::Left)
                } else {
                    (plot.right(), TextAlign::Right)
                };
                b.text(PointPx { x, y }, title, label(al));
            }
        }
    }
}

/// Calls `f` with the grid values of `a`: ticks, or the slot boundaries in
/// view on a category axis.
fn for_each_grid_value(a: &Axis, mut f: impl FnMut(f64)) {
    if a.opts.kind == AxisKind::Category {
        let (lo, hi) = a.view;
        let k0 = lo.ceil().max(0.0) as usize;
        let k1 = hi.floor().max(0.0) as usize;
        for k in k0..=k1 {
            f(k as f64);
        }
    } else {
        for &TickLabel { value, .. } in &a.ticks {
            f(value);
        }
    }
}

/// Whether the label of a rule on axis `id` comes within `reach` px of a
/// tick label at `p` (so the tick label is left out, as on chart axes). Along x,
/// `reach` is the tick label's half width plus a gap and the rule label's
/// half width is added; along y it is half a line plus a gap and the rule
/// label's half height is added.
fn rule_label_near(
    rules: &[(RuleId, RuleOptions)],
    a: &Axis,
    id: AxisId,
    p: f32,
    reach: f32,
    text: &dyn TextMeasurer,
    font: FontStyleRef,
) -> bool {
    rules.iter().any(|(_, r)| {
        let Some(t) = r.label.as_deref().filter(|t| !t.is_empty() && r.axis == id) else {
            return false;
        };
        let q = a.to_px(r.value) as f32;
        let own = if id == AxisId::X {
            text_width(text, t, font) * 0.5
        } else {
            text.line_height(font) * 0.5
        };
        (q - p).abs() < reach + own
    })
}

fn check_rule(r: &RuleOptions) -> Result<()> {
    if !r.value.is_finite() || r.to.is_some_and(|t| !t.is_finite()) {
        return Err(Error::InvalidArgument("rule values must be finite"));
    }
    Ok(())
}
