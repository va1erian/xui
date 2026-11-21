# Motion

xui has a small, portable foundation for animation: timing curves, an eased
value over time, a clock abstraction, the system's reduced-motion setting, and
a driver that moves a value frame by frame at the display's refresh rate.
Buttons, check boxes, toggle buttons, number fields, progress bars, tabs,
menus, combo boxes and tree views animate with it; see [What does not
exist](#what-does-not-exist) for the limits.

The value types live in `xui_core::motion` and are portable: no platform
dependency, no `unsafe`, no clock read, no thread, no file system. They use
nothing that is unavailable on `wasm32` (not yet compiled for it in CI), and
they run the same in a headless test. Time and frames come from the backend.

## Easing

`Easing` is a pure map from normalised time `t` in `[0, 1]` to progress in
`[0, 1]`: `fn ease(self, t: f32) -> f32`.

| Curve | `f(t)` |
|---|---|
| `Linear` | `t` |
| `QuadIn`, `QuadOut`, `QuadInOut` | `t²`, `1 - (1 - t)²`, and the two joined at `0.5` |
| `CubicIn`, `CubicOut`, `CubicInOut` | `t³`, `1 - (1 - t)³`, and the two joined at `0.5` |
| `ExpoIn { exponent }` | `(e^(k t) - 1) / (e^k - 1)` for `k = exponent` |
| `ExpoOut { exponent }` | `1 - f(1 - t)` for the ease-in `f` of the same exponent |

The exponential pair is the Windows Phone Toolkit's `ExponentialEase`, whose
transitions use ease-out with an exponent of `6` (`DEFAULT_EXPONENT`,
`Easing::expo_out()`): a fast start that settles slowly, over 90% of the way
at `t = 0.4`.

Guarantees, for every curve: `ease(0.0) == 0.0` and `ease(1.0) == 1.0`
exactly; `t` outside `[0, 1]` is clamped and a NaN `t` is the start; the result
stays in `[0, 1]` (no overshoot) and never decreases. An exponent of zero or
NaN is linear, and a very large one is a step; neither produces NaN.

An exponential is not a cubic Bezier, so the web side will approximate the
exponential curves with a CSS `linear()` function sampled from `ease`, and its
tests carry a tolerance. A native renderer evaluates `ease` directly.

## Tween

A `Tween { from, to, duration, delay, easing }` is a value that moves from
`from` to `to`. It is a pure function of the time elapsed since it started:

```rust
use std::time::Duration;
use xui_core::motion::{Easing, Tween};

let slide = Tween::new(40.0, 0.0, Duration::from_millis(300))
    .with_delay(Duration::from_millis(50))
    .with_easing(Easing::expo_out());

assert_eq!(slide.sample(Duration::from_millis(10)), 40.0); // still delayed
assert!(slide.sample(Duration::from_millis(150)) < 10.0);  // most of the way
assert_eq!(slide.sample(Duration::from_millis(350)), 0.0); // exactly `to`
assert!(slide.is_finished(Duration::from_millis(350)));
```

- Before the delay the value is `from`; from `delay + duration` on it is
  exactly `to`.
- A **zero duration** jumps: `from` until the delay, `to` from the delay on.
- The value never leaves the range between `from` and `to`, and negative or
  descending ranges work.
- If `from` or `to` is NaN or infinite there is nothing to interpolate, so the
  tween holds `from` and then returns `to`.
- Duration arithmetic saturates (`Tween::total`, `ManualClock::advance`); no
  input overflows.

## Clock

`Clock` is `fn now(&self) -> Duration`, the time since an arbitrary origin.
`xui-core` never reads one itself, because a web build has no `Instant` and a
test has to control time:

- `ManualClock` (in `xui_core::motion`) moves only when told to
  (`set`, `advance`). Use it in tests and headless runs.
- `xui_canvas::MonotonicClock` reads `std::time::Instant`. It is the real clock
  of a desktop driver. `Instant::now()` panics on `wasm32-unknown-unknown`, so a
  web build implements `Clock` over `performance.now()` instead.

A reference to a clock, and a `Box`, `Rc` or `Arc` of one, is itself a `Clock`,
so `fn drive(clock: impl Clock)` accepts `&clock` or an `Rc<ManualClock>`.

## Reduced motion

`Backend::prefers_reduced_motion(&self) -> bool` and
`Ui::prefers_reduced_motion()` carry the operating system's animation
preference. **When it is `true`, motion resolves to a plain fade or an instant
change, and nothing loops.** Check it when an animation starts; the value is read
live, so a change made while the app runs applies to the next animation.

```rust
let duration = if ui.prefers_reduced_motion() {
    Duration::ZERO
} else {
    Duration::from_millis(300)
};
```

| Backend | Answer |
|---|---|
| `D2dBackend` | `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)`, inverted: the setting says whether client-area animation is *on*, so reduced motion is "off". A failed query is `false`. |
| `WinitBackend` on Windows | the same query |
| `WinitBackend` on macOS and Linux | `false`. No platform query is wired up there yet; nothing is guessed. |
| `OffscreenBackend` | `false` unless a test scripts it with `set_prefers_reduced_motion`; it never reads the machine's setting, so a headless render is the same everywhere |
| any other `Backend` | `false` unless it overrides the method (the default) |

## Animating a value

An `Animated` is a value that eases to its latest target. A widget shares one
with its painter through an `Rc`, starts it with `Ui::animate`, and the painter
reads it with the time of the frame being painted:

```rust
use std::rc::Rc;
use xui_core::motion::{Animated, Motion};

let press = Rc::new(Animated::new(0.0));

// In the event handler: the state changes at once, the picture follows.
ui.animate(id, &press, 1.0, Motion::QUICK);

// In the painter (`time` is `ui.frame_time_handle()`, captured once):
let amount = press.value(time.get());
let fill = theme.surface.lerp(theme.pressed, amount);
```

- **Interruptible.** Animating a value that is already moving turns it round
  from where it is. There is no jump and no queue.
- **Any frame rate.** A value depends only on the time, so an animation lasts
  the same wall time at 30, 60 or 240 fps; a slow frame lands further along
  the curve.
- **One instant per frame.** `Ui::frame_time` is latched once per frame, so
  every painter of a frame agrees.
- **Idle costs nothing.** Frames are requested one at a time, only while a
  value is moving.
- `Ui::animate_with` also calls a closure with the value on each frame, for
  the few animations that must move a node (a popup unrolling) rather than
  only repaint it.
- Destroying a node drops what was animating on it.

`Motion` is a duration and an `Easing`. Its constants are the motion tokens;
widgets use them instead of their own numbers:

| Token | Duration | Curve | For |
|---|---|---|---|
| `Motion::QUICK` | 100 ms | `ExpoOut` | a change of state in place: a press released, a mark drawn |
| `Motion::REVEAL` | 140 ms | `ExpoOut` | something appearing: a popup, a tree node opening |
| `Motion::TRAVEL` | 180 ms | `CubicInOut` | something moving from one place to another |
| `Motion::EXIT` | 80 ms | `QuadIn` | something leaving |

### When a change is instant

`Ui::animate` lands on the target at once, with a single repaint, when
`Ui::animations()` is `false`:

- the system asks for reduced motion (`Ui::prefers_reduced_motion`);
- the application called `Ui::set_animations(false)`;
- the handle is in design mode;
- the backend has no clock (`Backend::now` is `None`).

A widget therefore needs no second code path, and the last frame of an
animation is always the picture it would have painted without one.

### Frames and time

Two `Backend` methods feed the driver:

| | `Backend::now` | `Backend::request_frame` |
|---|---|---|
| `WinitBackend` | a monotonic clock | one `Event::Frame` per refresh of the window's monitor |
| `D2dBackend` | a monotonic clock | one `Event::Frame` per refresh of the window's monitor |
| `OffscreenBackend`, `HeadlessBackend` | `None` until a test calls `set_now` or `advance` | recorded; the test delivers `Event::Frame` |
| any other `Backend` | `None` (the default) | `false` (the default) |

`request_frame` is one-shot, like a browser's `requestAnimationFrame`. A
backend that returns `false` is ticked with a 16 ms `set_timer` instead, which
the application's `Ui::on_timer` never sees. Because the headless backends have
no clock unless a test gives them one, snapshots and existing tests render
exactly as they would with no animation.

To test an animated widget, script the clock and deliver the frames:

```rust
backend.set_now(Duration::ZERO);
// ... trigger the change ...
backend.advance(Duration::from_millis(50));
backend.inject(window, WidgetId::NONE, Event::Frame);
```

The offscreen backend's `inject(window, Event::Frame)` does the same, and so
does `Stage::inject(Event::Frame)`.

## What does not exist

- **Per-node opacity and 2D transform** in the compositing path. A widget
  cannot be faded or translated as a unit: an animation is drawn by the
  widget's own painter (a colour blend, a clip, a moved rectangle).
- **Smooth scrolling**, and a page transition for `Tabs`.
