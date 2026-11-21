# Plan: optional widget animations

Status: steps 0 and 1 (the pacing probe and the driver) are implemented; no
widget animates yet. `docs/motion.md` documents what exists.

Goal: short, informative animations on the portable widgets, on every backend,
at any frame rate, built on `xui_core::motion` (`Easing`, `Tween`, `Clock`,
`prefers_reduced_motion`) with as little new code as possible.

## Principles

- **Time-sampled, never frame-counted.** A `Tween` is already a pure function
  of elapsed time. A painter samples it at the frame's timestamp, so an
  animation lasts the same wall time at 30, 60 or 144 fps; a slow or dropped
  frame just lands further along the curve. Nothing queues up.
- **Painted by the widget, not composited.** There is no per-node opacity or
  transform, and this plan does not add one. Every effect is drawn in the
  widget's own painter with what `Canvas` already has: `Color::lerp`,
  `push_clip`, `save`/`set_translation`, and node bounds through
  `apply_moves`. A backend only has to run painters and deliver a timer, which
  all of them already do, so the animations are identical everywhere.
- **Input is never delayed.** The message is emitted and the state changes at
  once; the animation only shows how the picture gets there. Hit-testing uses
  the final state.
- **Quick.** 80 to 200 ms, `ExpoOut` for things that appear (most of the move
  is in the first frames, so it reads well even at 30 fps), `CubicInOut` for
  things that travel. No overshoot, no bounce, no loop: the existing curves
  guarantee it.
- **Optional.** Animations resolve instantly when the OS asks for reduced
  motion, when the app calls `Ui::set_animations(false)`, in design mode, and
  on a backend with no clock.
- **Idle costs nothing.** Frames are requested one at a time, only while
  something animates. No allocation per frame.
- **Every refresh rate.** Frames are paced to the monitor the window is on,
  not to a fixed 60 Hz.

## 1. The driver (the only new infrastructure)

About 200 lines in `xui-core`, plus a few per backend.

### `motion/animated.rs`

```rust
/// A value that eases to its latest target. Interruptible: retargeting
/// mid-flight starts from the value it has now.
pub struct Animated { tween: Cell<Tween>, started: Cell<Duration> }

impl Animated {
    pub const fn new(value: f32) -> Animated;
    pub fn value(&self, now: Duration) -> f32;   // tween.sample(now - started)
    pub fn target(&self) -> f32;
    pub fn is_running(&self, now: Duration) -> bool;
    pub fn retarget(&self, to: f32, now: Duration, motion: Motion);
    pub fn jump(&self, to: f32);
}

/// A duration and a curve. The presets are the motion tokens.
pub struct Motion { pub duration: Duration, pub easing: Easing }
impl Motion {
    pub const QUICK: Motion;   // 100 ms, ExpoOut   (press, check, hover)
    pub const REVEAL: Motion;  // 140 ms, ExpoOut   (menu, tree node, number)
    pub const TRAVEL: Motion;  // 180 ms, CubicInOut (tab underline, progress)
    pub const EXIT: Motion;    //  80 ms, QuadIn    (closing)
}
```

Pure, no clock read, unit-tested with hand-written durations like `Tween`.

### `Backend` contract: two defaulted methods

```rust
/// Monotonic time, or `None` when the backend has no clock.
fn now(&self) -> Option<Duration> { None }
/// Asks for one `Event::Frame` on `window`, delivered once, paced to the
/// refresh of the monitor the window is on. `false`: not supported, and the
/// driver falls back to a 16 ms `set_timer`.
fn request_frame(&self, window: WindowId) -> bool { false }
```

| Backend | `now` | `request_frame` |
|---|---|---|
| `WinitBackend`, GL host | `MonotonicClock` | yes, paced to the monitor (see "Frame pacing") |
| `D2dBackend` | `Instant`-based | yes, paced to the monitor |
| `OffscreenBackend`, `HeadlessBackend` | `None` until a test calls `set_now`/`advance` | yes: the test delivers `Event::Frame` itself |
| any other | `None` (default) | `false` (default): 16 ms timer |

`request_frame` is one-shot, like `requestAnimationFrame`: the driver asks
again from each frame while something still animates, so the loop stops by
itself and a backend never ticks an idle window.

`None` means "animate instantly". So every existing headless test, golden and
`scripts/snapshots` render stays pixel-identical, and a new platform (web,
macOS-native) gets animations by implementing one method over its own clock.

### `Ui` API (`app/ui/motion.rs`)

```rust
pub fn animate(&self, id: WidgetId, value: &Animated, to: f32, motion: Motion);
pub fn animate_with(&self, id, value: &Rc<Animated>, to, motion, step: impl Fn(f32) + 'static);
pub fn frame_time(&self) -> Duration;     // what painters sample with
pub fn set_animations(&self, on: bool);   // default on
pub fn animations(&self) -> bool;         // on && !reduced motion && !design mode && clock
```

`animate`:

1. If `!animations()`: `value.jump(to)`, `invalidate(id)`, done. One code path
   for reduced motion, the app switch and clockless backends.
2. Otherwise `value.retarget(to, now, motion)`, record `(id, end, step)` in the
   core's active list (a retained `Vec`; an entry for the same id and value is
   replaced), and call `request_frame` if no frame is pending.

On `Event::Frame` (or the fallback timer's tick), in the core:

1. Latch `frame_time = backend.now()` once, so every painter in the frame
   samples the same instant.
2. For each entry: call `step(value)` if there is one, then `invalidate(id)`.
3. Drop entries whose end has passed (after one final frame at exactly `to`).
4. If entries remain, `request_frame` again; otherwise stop (and kill the
   fallback timer if that is what was ticking).

`Control::drop` removes its entries, so a widget destroyed mid-animation leaves
nothing behind. `step` exists only for the two cases that must move nodes
(menu popup, tab page); everything else is painter-only.

### Frame pacing

The target is one frame per refresh of the monitor the window is on, whatever
its rate (60, 160, 240.08 Hz), following the window when it moves between
monitors of different rates. Below the refresh rate (slow machine, software
rendering of a big window) there are fewer samples of the same curve over the
same duration.

What exists today, read from the code:

- `xui-d2d` pumps with a plain `GetMessageW` loop (`sys/pump.rs`), ticks with
  `SetTimer`/`WM_TIMER`, and paints in `WM_PAINT` into an
  `ID2D1HwndRenderTarget` created with `D2D1_PRESENT_OPTIONS_RETAIN_CONTENTS`
  only (`sys/gfx/target.rs`). `SetTimer` cannot tick faster than
  `USER_TIMER_MINIMUM` (10 ms, `WinUser.h`) and is usually coarser, so it is
  the wrong source for a frame tick: about 64 fps at best.
- `xui-canvas` drives its timers from `about_to_wait` with
  `ControlFlow::WaitUntil` (`backend/app/mod.rs`). winit 0.30.13 backs that
  on Windows with a high-resolution waitable timer
  (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`, its `event_loop.rs`), so
  sub-millisecond deadlines are honoured. The software path presents with a
  GDI `BitBlt` (softbuffer), which does not wait for vblank; the GL path sets
  swap interval 1.

#### Winit (all platforms)

`request_frame` records a deadline, `last_frame + refresh_period`, next to the
timers `App::timers` already schedules, and delivers `Event::Frame` when it is
due. The period comes from `window.current_monitor()` and
`MonitorHandle::refresh_rate_millihertz()` (240 080 for a 240.08 Hz panel),
read at each request so a window dragged to another monitor follows it; 60 Hz
if the monitor reports nothing. No new dependency, no `unsafe`, about 30
lines. On Windows this gives the full refresh rate through the existing
high-resolution wait.

#### Direct2D

Two parts, each in `src/sys/`:

1. **A frame tick as precise as winit's** (`sys/frame_timer.rs`, new): a
   waitable timer created with `CreateWaitableTimerExW` and
   `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` (`synchapi.h`; Windows 10 1803 and
   later, falling back to the `SetTimer` path when creation fails). The two
   loops in `sys/pump.rs` wait with `MsgWaitForMultipleObjectsEx` on that
   handle while a frame is requested and stay on `GetMessageW` otherwise, so
   an idle app is exactly as it is now. The period is the refresh rate of the
   window's monitor (`MonitorFromWindow`, then `EnumDisplaySettingsExW`
   `dmDisplayFrequency`), re-read per request. That value is whole hertz, so
   240.08 Hz is paced at 240: one repeated frame every 12 s of continuous
   animation, which no 200 ms animation can show.
2. **A present that does not hold the frame back:**
   `D2D1_PRESENT_OPTIONS_IMMEDIATELY` alongside `RETAIN_CONTENTS`. The
   desktop compositor still shows whole frames, so there is no tearing in a
   window, and the pacing is entirely the timer's. The measurements below
   are why.

`crates/xui-d2d/examples/frame_pacing.rs` measures this: it repaints a small
window on every monitor under each combination (present option x tick source)
and prints the frame intervals. It stays as the regression check for pacing.
One run, 240 Hz primary with 160 Hz and 60 Hz secondaries, frames per second
by median interval:

| Present | Tick | on 240 Hz (primary) | on 160 Hz | on 60 Hz |
|---|---|---|---|---|
| vsync (today) | free-run | 240.5 | 240.2 | 240.6 |
| vsync (today) | high-res timer | 240.8 | 160.5 | 60.2 |
| vsync (today) | `SetTimer` (today) | 63.8 | 63.8 | 64.5 |
| immediate | high-res timer | 242.8 | 160.2 | 60.0 |
| immediate | `SetTimer` | 64.3 | 64.1 | 64.3 |

- `SetTimer` gives about 64 fps everywhere, with frames up to 31 ms apart.
- The high-resolution timer hits each monitor's own rate, with no late frame
  and a worst interval within 0.8 ms of the period.
- The default present blocks on the **primary** monitor's vblank whichever
  monitor the window is on (240 fps free-running on the 60 Hz panel). With
  the fastest monitor as primary that costs nothing. With a slower primary it
  should hold a window on the 240 Hz panel to the primary's rate; that
  arrangement was not run.
- `IMMEDIATELY` with the timer reaches the same rates and does not depend on
  which monitor is primary, so that is the choice. The probe measures
  intervals, not what the eye sees: the timer is not phase-locked to vblank,
  so an occasional repeated frame is possible and has to be judged on screen
  once a real animation exists.

Not covered: the modal move/size loop and native menus run Windows' own
message loop, not `sys/pump.rs`, so an animation in flight while the user
drags the window border falls back to the `SetTimer` rate for that time.

## 2. The animations

Each is a few lines in the widget's event handler plus a few in its painter.
The last frame is always exactly the picture painted today.

| Widget | Trigger | What is drawn | Motion | Tells the user |
|---|---|---|---|---|
| **Button** | release / Space / Enter | press is instant; on activation the face eases from `pressed` back to its resting fill (`Color::lerp`) | QUICK | the click registered. Keyboard activation has no feedback at all today |
| **Menu open** | first popup of a menu, submenu | popup node unrolls from its anchor edge: `step` grows the node's height via `apply_moves`, rows stay anchored at the top | REVEAL | where the menu came from |
| **Menu close** | pick, Esc, click away | the same in reverse; the pick's message is already emitted | EXIT | it closed on purpose, not vanished |
| **Tree node** | expand / collapse | children are clipped to a growing or shrinking band under the node; rows below slide with its edge; the chevron rotates with the same value | REVEAL | which rows belong to the node, and where the rest went |
| **Tabs** | select | the accent underline slides and resizes from the old tab to the new; the new page starts offset a few dip in the direction of travel and settles | TRAVEL / REVEAL | direction: which side the new page is on |
| **NumberField** | stepper, Up/Down | old text rolls out and new rolls in, up for an increment, down for a decrement, clipped to the text rect | REVEAL | the direction of the change, not just a different number |
| **Checkbox, ToggleButton** | toggle | mark draws in; a switch thumb slides | QUICK | new state vs. old |
| **ProgressBar** | `set_value` | fill eases to the new value instead of jumping | TRAVEL | progress is continuous; coarse updates stop looking like stalls |
| **ComboBox list** | open / close | same unroll as the menu, shared helper | REVEAL / EXIT | same |

Deliberately not animated: `Slider` thumb and scrollbar drags (direct
manipulation must track the pointer 1:1), text carets and selection, hover
highlights in lists and trees (they fire constantly; easing them smears),
window and dialog open (the OS owns those), tooltips (need node opacity).
Smooth wheel/keyboard scrolling fits the brief but touches every scrolling
widget's offset model; it is a separate plan.

### Notes per widget

- **Menu.** Only the first popup of a session animates; sliding along the bar
  from title to title swaps instantly, as now (`open_bar` already avoids
  churn there). Closing needs the popup's `Level`/`Open` to outlive the state
  truncation in `close_levels_from` for 80 ms: move them to a `closing` list
  the painter falls back to, and cancel it if the pooled node is reopened.
  The first step sets the node's start height *before* `set_visible`, because
  D2D paints a popup synchronously inside `set_visible` (#130).
- **Tree.** One animation at a time (`Animated` + the node id + the row count,
  capped to what fits the viewport). A collapsing node's rows must still be
  painted while they shrink although `expanded` is already false: the painter
  treats that subtree as visible until the animation ends. The scrollbar goes
  to its final range at once. `draw_chevron` takes an angle instead of a bool.
- **Tabs.** The underline is painter-only in the strip. The page nudge moves
  the selected page's children with one `apply_moves` per frame and depends on
  children being clipped to the container on every backend; verify that
  first, and ship the underline alone if it does not hold.
- **NumberField.** Keeps the previous text in an existing-style `RefCell`
  (written once per step, not per frame). The caret is hidden during the
  roll. Typing and `set_value` do not animate. The file is at 413 lines:
  move its tests to `numberfield/tests.rs` first.

## 3. Tests

- `Animated`: retarget mid-flight is continuous, `jump`, end is exactly `to`.
- Driver, on `HeadlessBackend` with a scripted clock: the timer starts with
  the first animation and is killed after the last; `set_animations(false)`,
  reduced motion and a clockless backend all land on the final value with one
  invalidate; dropping a widget mid-animation clears its entry; two widgets
  animating share one timer.
- Per widget, on `OffscreenBackend` with a scripted clock: a snapshot at the
  start, one mid-way and one at the end, and **the end snapshot equals the
  render with animations off**. A mid-way snapshot differs from both.
- Frame-rate independence: stepping the clock in 33 ms, 16 ms and 7 ms
  increments reaches the same value at the same elapsed time.
- Gallery: an "Animations" switch on the `widgets` gallery page that drives
  `Ui::set_animations`, so each one can be seen and compared.

## 4. Order of work

Each step is its own PR and leaves `main` shippable.

0. Pacing probe: `xui-d2d/examples/frame_pacing.rs` (written and run; see
   "Frame pacing").
1. Driver: `Animated`, `Motion`, `Event::Frame`, the two `Backend` methods on
   all four backends (winit and Direct2D paced to the monitor),
   `Ui::animate`/`frame_time`/`set_animations`, tests, `docs/motion.md`
   ("What does not exist" loses the driver and the tokens).
2. Button, Checkbox, ToggleButton (painter-only, proves the driver).
3. NumberField roll, ProgressBar.
4. Tabs underline, then the page nudge if clipping holds.
5. Menu and ComboBox popup unroll (needs `animate_with`).
6. Tree expand/collapse.

Steps 2 to 6 touch disjoint files once step 1 is in, so they can run in
parallel.

## Open decisions

- **Tabs:** underline plus a short page nudge (recommended), or a full-width
  page slide. The full slide keeps the outgoing page alive and moving for the
  whole animation, which is more code and more repaint for little more
  information.
- **Default:** on, with the OS setting and `set_animations` to turn it off
  (recommended), or off until an app opts in.
