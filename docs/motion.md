# Motion

xui has a small, portable foundation for animation: timing curves, an eased
value over time, a clock abstraction and the system's reduced-motion setting.
It computes values. It does not yet move a widget: see [What does not
exist](#what-does-not-exist).

Everything here lives in `xui_core::motion` and is portable: no platform
dependency, no `unsafe`, no clock read, no thread, no file system. It compiles
for `wasm32` and runs the same in a headless test.

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
  of a desktop driver.

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
| `Win32Backend` | `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)`, inverted: the setting says whether client-area animation is *on*, so reduced motion is "off". A failed query is `false`. |
| `WinitBackend` on Windows | the same query |
| `WinitBackend` on macOS and Linux | `false`. No platform query is wired up there yet; nothing is guessed. |
| `OffscreenBackend` | `false` always, so a headless render is the same on every machine |
| any other `Backend` | `false` unless it overrides the method (the default) |

## What does not exist

- **Per-node opacity and 2D transform** in the compositing path. A widget
  cannot yet be faded or translated as a unit; that is a separate, later step.
- A frame driver or scheduler that samples tweens and requests repaints, region
  repaint, and animated widgets.
- Motion presets (slide, fade, turnstile) and their tokens.

Until those exist, `motion` is the part an application or a later driver builds
on: sample a `Tween` with a `Clock`, honour `prefers_reduced_motion`.
