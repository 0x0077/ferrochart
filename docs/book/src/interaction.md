# Gestures and keyboard

The chart handles input itself; bindings only translate platform events into
`InputEvent`s (`ferrochart-winit` and `ferrochart-wasm` do this for you).

## Gestures

| Input | Effect |
|---|---|
| Drag in the plot | Pan (release while moving: kinetic scrolling) |
| Wheel / trackpad scroll | Zoom time around the pointer; horizontal scroll or Shift+wheel pans |
| Wheel over the price axis, or Alt+wheel | Zoom prices |
| Pinch (trackpad, two fingers) | Zoom |
| Drag the price axis | Scale prices (turns auto-scale off) |
| Drag the time axis | Change the bar spacing |
| Double-click an axis | Reset it (auto-scale, default spacing) |
| ← → / ↑ ↓ | Pan by 10 % / zoom |
| Home / End | Jump to the first / latest bar |

The crosshair follows the pointer, with price and time labels on the axes. Its
mode is `Normal`, `Magnet` (snaps to the nearest of the bar's open, high,
low and close) or `Hidden`.

## Touch

On a touch screen (phones, tablets, touch laptops) a finger has no hover, so
the gestures differ from the mouse. A pen behaves like a mouse.

| Touch | Effect |
|---|---|
| Drag one finger | Pan (lift while moving: kinetic scrolling) |
| Two fingers | Zoom time around their midpoint by the change of their distance, and pan with the midpoint; lifting one finger keeps panning with the other |
| Hold one finger still for 500 ms | Show the crosshair; it follows the finger without panning and stays after lifting |
| Tap | A click; also hides a crosshair left by a long press |
| Double tap an axis | Reset it, like a double click |

- A finger has to move 8 CSS px before a press becomes a drag (a mouse: 3 px),
  so a tap with a little wobble is still a click.
- Fingers hit from further away: 10 CSS px from a line, 12 px from a drawing's
  handle, 8 px beyond a marker.
- While a drawing is being drawn or dragged, a second finger is ignored.
- With the crosshair mode `Hidden`, holding a finger still does nothing
  special: moving it pans.
- When the system takes a touch over (the page starts scrolling, a call comes
  in), the gesture ends without a click or kinetic scrolling.

In the browser, whether a vertical swipe pans the chart or scrolls the page
is the `touchScroll` option; see [Touch screens](web.md#touch-screens).

## Events

What the chart reports back (crosshair moves, clicks, visible range changes,
history requests, drawing changes) is described in
[Crosshair and events](crosshair-events.md). In Rust, pull the same events
with `Chart::drain_events`; if you don't drain them for a while, newer
crosshair and range events replace older ones instead of piling up.

## Time

Input events carry a timestamp (`time_ms`) and `build_frame` takes the current
time (`now_ms`); both must come from the same clock. Kinetic scrolling uses
them, and so does the long press: a finger held still turns into a crosshair
when the event time or the frame time reaches 500 ms after the press. The
bindings use `performance.now()` (web) or a monotonic clock started with the
window (desktop).
