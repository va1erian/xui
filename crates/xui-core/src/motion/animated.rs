#![forbid(unsafe_code)]

//! [`Animated`]: a value that eases to its latest target, and [`Motion`]: how
//! long that takes and along which curve.

use std::cell::Cell;
use std::time::Duration;

use super::{Easing, Tween};

/// A duration and a timing curve: how one change is animated.
///
/// The associated constants are xui's motion tokens. Widgets use them rather
/// than their own numbers, so the whole library moves alike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// How long the change takes.
    pub duration: Duration,
    /// The timing curve of the change.
    pub easing: Easing,
}

impl Motion {
    /// A small change of state in place: a press released, a mark drawn.
    pub const QUICK: Motion = Motion::new(Duration::from_millis(100), Easing::expo_out());
    /// Something appearing: a popup unrolling, a tree node opening.
    pub const REVEAL: Motion = Motion::new(Duration::from_millis(140), Easing::expo_out());
    /// Something travelling from one place to another: a tab underline.
    pub const TRAVEL: Motion = Motion::new(Duration::from_millis(180), Easing::CubicInOut);
    /// Something leaving: shorter than its arrival, and accelerating away.
    pub const EXIT: Motion = Motion::new(Duration::from_millis(80), Easing::QuadIn);

    /// A change that takes `duration` along `easing`.
    pub const fn new(duration: Duration, easing: Easing) -> Motion {
        Motion { duration, easing }
    }
}

/// A value that eases to its latest target.
///
/// It holds a [`Tween`] and the time that tween started, and is read with the
/// time of the frame being painted, so it has the same value for every reader
/// of that frame and the same duration at any frame rate. It is interruptible:
/// [`Animated::retarget`] starts from the value it has at that moment, so a
/// change reversed mid-flight turns round where it is instead of jumping.
///
/// A widget shares one with its painter through an `Rc` and starts it with
/// [`Ui::animate`](crate::Ui::animate), which also schedules the frames.
///
/// ```
/// use std::time::Duration;
/// use xui_core::motion::{Animated, Easing, Motion};
///
/// let ms = Duration::from_millis;
/// let linear = Motion::new(ms(100), Easing::Linear);
/// let value = Animated::new(0.0);
/// value.retarget(10.0, ms(1000), linear);
/// assert_eq!(value.value(ms(1050)), 5.0);
/// // Reversed half-way: it turns round from 5, it does not jump.
/// value.retarget(0.0, ms(1050), linear);
/// assert_eq!(value.value(ms(1050)), 5.0);
/// assert_eq!(value.value(ms(1150)), 0.0);
/// ```
#[derive(Debug)]
pub struct Animated {
    tween: Cell<Tween>,
    started: Cell<Duration>,
}

impl Animated {
    /// A value resting at `value`.
    pub const fn new(value: f32) -> Animated {
        Animated {
            tween: Cell::new(Tween::new(value, value, Duration::ZERO)),
            started: Cell::new(Duration::ZERO),
        }
    }

    /// The value at `now`. A `now` before the last [`Animated::retarget`] is
    /// the value it started from.
    pub fn value(&self, now: Duration) -> f32 {
        self.tween
            .get()
            .sample(now.saturating_sub(self.started.get()))
    }

    /// The value this is moving to, or resting at.
    pub fn target(&self) -> f32 {
        self.tween.get().to
    }

    /// Whether the value is still moving at `now`.
    pub fn is_running(&self, now: Duration) -> bool {
        !self
            .tween
            .get()
            .is_finished(now.saturating_sub(self.started.get()))
    }

    /// Starts moving to `to` at `now`, from the value it has at `now`.
    pub fn retarget(&self, to: f32, now: Duration, motion: Motion) {
        let from = self.value(now);
        self.tween
            .set(Tween::new(from, to, motion.duration).with_easing(motion.easing));
        self.started.set(now);
    }

    /// Moves to `to` at once.
    pub fn jump(&self, to: f32) {
        self.tween.set(Tween::new(to, to, Duration::ZERO));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    const LINEAR: Motion = Motion::new(ms(100), Easing::Linear);

    #[test]
    fn a_new_value_rests_where_it_was_put() {
        let value = Animated::new(3.0);
        assert_eq!(value.value(Duration::ZERO), 3.0);
        assert_eq!(value.value(ms(5000)), 3.0);
        assert_eq!(value.target(), 3.0);
        assert!(!value.is_running(Duration::ZERO));
    }

    #[test]
    fn it_reaches_the_target_exactly_and_stops() {
        let value = Animated::new(0.0);
        value.retarget(1.0, ms(200), Motion::REVEAL);
        assert!(value.is_running(ms(200)));
        assert!(value.is_running(ms(339)));
        assert!(!value.is_running(ms(340)));
        assert_eq!(value.value(ms(340)), 1.0);
        assert_eq!(value.value(ms(9000)), 1.0);
    }

    #[test]
    fn a_time_before_the_start_is_the_starting_value() {
        let value = Animated::new(2.0);
        value.retarget(8.0, ms(500), LINEAR);
        assert_eq!(value.value(ms(100)), 2.0);
        assert!(value.is_running(ms(100)));
    }

    #[test]
    fn retargeting_mid_flight_is_continuous() {
        let value = Animated::new(0.0);
        value.retarget(10.0, ms(0), LINEAR);
        let before = value.value(ms(30));
        value.retarget(-4.0, ms(30), Motion::TRAVEL);
        assert_eq!(value.value(ms(30)), before);
        assert_eq!(value.target(), -4.0);
        assert_eq!(value.value(ms(210)), -4.0);
    }

    #[test]
    fn jump_lands_at_once_whatever_the_time() {
        let value = Animated::new(0.0);
        value.retarget(10.0, ms(1000), LINEAR);
        value.jump(7.0);
        assert_eq!(value.value(Duration::ZERO), 7.0);
        assert_eq!(value.value(ms(1050)), 7.0);
        assert!(!value.is_running(ms(1050)));
    }

    #[test]
    fn the_same_elapsed_time_gives_the_same_value_at_any_frame_rate() {
        // A sample depends on the time alone, not on how many frames were
        // sampled before it: 30, 60 and 144 fps agree at 98 ms.
        for step in [33, 16, 7] {
            let value = Animated::new(0.0);
            value.retarget(1.0, ms(0), Motion::TRAVEL);
            let mut now = 0;
            while now < 98 {
                let _ = value.value(ms(now));
                now += step;
            }
            let fresh = Animated::new(0.0);
            fresh.retarget(1.0, ms(0), Motion::TRAVEL);
            assert_eq!(value.value(ms(98)), fresh.value(ms(98)), "{step} ms frames");
        }
    }

    #[test]
    fn the_tokens_are_short() {
        for motion in [Motion::QUICK, Motion::REVEAL, Motion::TRAVEL, Motion::EXIT] {
            assert!(motion.duration <= ms(200));
            assert!(motion.duration >= ms(80));
        }
    }
}
