//! Chart and plot events → Python.
//!
//! Each chart or plot has one Python handler (`set_event_handler`, set by
//! the Python `Chart` or `Plot`), called as `handler(name, payload)` with `snake_case` event
//! names and payload keys. In the blocking `show` the window calls it
//! directly (the loop runs on the main thread). The non-blocking window
//! thread queues events instead; they are delivered on the Python main
//! thread by a pending call (`Py_AddPendingCall`, run between bytecodes) or
//! by `ferrochart.process_events()`.

use std::collections::VecDeque;
use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use ferrochart_core::chart::{ChartEvent, ClickTarget, CrosshairPoint};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::chart::Shared;
use crate::shared::{self, Scene};

/// Most queued events; older ones are dropped beyond.
const MAX_QUEUED: usize = 10_000;

/// An event of the non-blocking window thread, delivered to its handler.
type Delivery = Box<dyn FnOnce(Python<'_>) + Send>;

/// Events of the non-blocking window thread, oldest first.
static QUEUE: Mutex<VecDeque<Delivery>> = Mutex::new(VecDeque::new());
/// A pending call is scheduled and has not run yet.
static SCHEDULED: AtomicBool = AtomicBool::new(false);

/// Python name of `e`; `None` for events Python does not get (sync).
pub(crate) fn name(e: &ChartEvent) -> Option<&'static str> {
    Some(match e {
        ChartEvent::Click { .. } => "click",
        ChartEvent::CrosshairMove { .. } => "crosshair_move",
        ChartEvent::VisibleRangeChanged { .. } => "visible_range_changed",
        ChartEvent::NeedMoreHistory { .. } => "need_more_history",
        ChartEvent::DrawingCreated(_) => "drawing_created",
        ChartEvent::DrawingModified(_) => "drawing_modified",
        ChartEvent::DrawingRemoved(_) => "drawing_removed",
        ChartEvent::DrawingSelected(_) => "drawing_selected",
        ChartEvent::Sync(_) | ChartEvent::DrawingSync => return None,
        _ => "warning",
    })
}

fn point<'py>(py: Python<'py>, p: &CrosshairPoint) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("logical_index", p.logical_index)?;
    d.set_item("time", p.time)?;
    d.set_item("pane", p.pane.0)?;
    d.set_item("price", p.price)?;
    match p.bar {
        Some(b) => {
            let bar = PyDict::new(py);
            bar.set_item("open", b.open)?;
            bar.set_item("high", b.high)?;
            bar.set_item("low", b.low)?;
            bar.set_item("close", b.close)?;
            bar.set_item("volume", b.volume)?;
            d.set_item("bar", bar)?;
        }
        None => d.set_item("bar", py.None())?,
    }
    Ok(d)
}

fn target(py: Python<'_>, t: ClickTarget) -> PyResult<Option<Bound<'_, PyDict>>> {
    let d = PyDict::new(py);
    match t {
        ClickTarget::Drawing(id) => {
            d.set_item("type", "drawing")?;
            d.set_item("id", id.0)?;
        }
        ClickTarget::Marker { series, index } => {
            d.set_item("type", "marker")?;
            d.set_item("series", series.0)?;
            d.set_item("index", index)?;
        }
        ClickTarget::Series(series) => {
            d.set_item("type", "series")?;
            d.set_item("series", series.0)?;
        }
        _ => return Ok(None),
    }
    Ok(Some(d))
}

/// The payload of `e` (series as ids; the Python `Chart` turns them into
/// `Series`).
pub(crate) fn payload<'py>(py: Python<'py>, e: &ChartEvent) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    match e {
        ChartEvent::Click {
            pos,
            pane,
            logical_index,
            time,
            price,
            target: t,
        } => {
            d.set_item("x", f64::from(pos.x))?;
            d.set_item("y", f64::from(pos.y))?;
            d.set_item("pane", pane.0)?;
            d.set_item("logical_index", logical_index)?;
            d.set_item("time", time)?;
            d.set_item("price", price)?;
            d.set_item("target", t.map(|t| target(py, t)).transpose()?.flatten())?;
        }
        ChartEvent::CrosshairMove { point: p } => {
            d.set_item("point", p.as_ref().map(|p| point(py, p)).transpose()?)?;
        }
        ChartEvent::VisibleRangeChanged { from, to } => {
            d.set_item("from", from)?;
            d.set_item("to", to)?;
        }
        ChartEvent::NeedMoreHistory { bars_before_start } => {
            d.set_item("bars_before_start", bars_before_start)?;
        }
        ChartEvent::DrawingCreated(id)
        | ChartEvent::DrawingModified(id)
        | ChartEvent::DrawingRemoved(id) => d.set_item("id", id.0)?,
        ChartEvent::DrawingSelected(id) => d.set_item("id", id.map(|i| i.0))?,
        other => d.set_item("kind", format!("{other:?}"))?,
    }
    Ok(d)
}

/// Calls the handler of `shared` for each event (errors are printed, like
/// an exception in a GUI callback, and do not stop the window).
pub(crate) fn deliver(py: Python<'_>, shared: &Shared, events: &[ChartEvent]) {
    let Some(handler) = shared.handler(py) else {
        return;
    };
    for e in events {
        let Some(n) = name(e) else {
            continue;
        };
        let result = payload(py, e).and_then(|p| handler.call1(py, (n, p)).map(drop));
        if let Err(err) = result {
            err.print(py);
        }
    }
}

/// Queues events of the window thread for the Python main thread.
pub(crate) fn enqueue<S: Scene>(shared: &Arc<shared::Shared<S>>, events: Vec<S::Event>) {
    {
        let mut q = QUEUE.lock().unwrap_or_else(PoisonError::into_inner);
        for e in events {
            if q.len() >= MAX_QUEUED {
                q.pop_front();
            }
            let shared = shared.clone();
            q.push_back(Box::new(move |py| {
                S::deliver(py, &shared, std::slice::from_ref(&e));
            }));
        }
        if q.is_empty() {
            return;
        }
    }
    if !SCHEDULED.swap(true, Ordering::AcqRel) {
        // SAFETY: `Py_AddPendingCall` may be called from any thread without
        // the GIL; `run_pending` is a plain function and takes no argument.
        let r = unsafe { pyo3::ffi::Py_AddPendingCall(Some(run_pending), std::ptr::null_mut()) };
        if r != 0 {
            // The interpreter's queue is full: `process_events` or the next
            // event delivers them.
            SCHEDULED.store(false, Ordering::Release);
        }
    }
}

extern "C" fn run_pending(_: *mut c_void) -> c_int {
    SCHEDULED.store(false, Ordering::Release);
    // Runs on the main thread with the GIL; errors are printed.
    Python::attach(process);
    0
}

/// Delivers the queued events (non-blocking windows) to their charts' and
/// plots' handlers; returns how many.
pub(crate) fn process(py: Python<'_>) -> usize {
    let events: Vec<_> = QUEUE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain(..)
        .collect();
    let n = events.len();
    for deliver in events {
        deliver(py);
    }
    n
}

/// Delivers events of windows opened with `show(block=False)` now (they are
/// otherwise delivered between bytecodes of the main thread); returns how
/// many.
#[pyfunction]
pub(crate) fn process_events(py: Python<'_>) -> usize {
    process(py)
}
