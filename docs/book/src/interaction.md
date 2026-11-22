# Gestures and keyboard

The chart handles input itself; bindings only translate platform events into
`InputEvent`s (`ferrochart-winit` and `ferrochart-wasm` do this for you).

## Gestures

| Input | Effect |
|---|---|
| Drag in the plot | Pan (release while moving: kinetic scrolling) |
| Wheel / trackpad scroll | Zoom time around the pointer; horizontal scroll or Shift+wheel pans |
| Wheel over the price axis, or Alt+wheel | Zoom prices |
| Pinch (trackpad, touch) | Zoom |
| Drag the price axis | Scale prices (turns auto-scale off) |
| Drag the time axis | Change the bar spacing |
| Double-click an axis | Reset it (auto-scale, default spacing) |
| ← → / ↑ ↓ | Pan by 10 % / zoom |
| Home / End | Jump to the first / latest bar |

The crosshair follows the pointer, with price and time labels on the axes. Its
mode is `Normal`, `Magnet` (snaps to the nearest of the bar's open, high,
low and close) or `Hidden`.

## Events

What the chart reports back (crosshair moves, clicks, visible range changes,
history requests, drawing changes) is described in
[Crosshair and events](crosshair-events.md). In Rust, pull the same events
with `Chart::drain_events`; if you don't drain them for a while, newer
crosshair and range events replace older ones instead of piling up.

## Time

Input events carry a timestamp (`time_ms`) and `build_frame` takes the current
time (`now_ms`); both must come from the same clock. Kinetic scrolling uses
them. The bindings use `performance.now()` (web) or a monotonic clock started
with the window (desktop).
