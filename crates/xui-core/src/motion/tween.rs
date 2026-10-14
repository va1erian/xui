#![forbid(unsafe_code)]

//! [`Tween`]: one value eased from a start to an end over time.

use std::time::Duration;

use super::Easing;

/// A value that moves from `from` to `to` over `duration`, after waiting
/// `delay`, shaped by `easing`.
///
/// A tween is a pure function of the time elapsed since it started: it holds no
/// clock and no state, so a driver samples it with whatever time it has (a
/// [`Clock`](super::Clock), a frame timestamp, a test's hand-written
/// [`Duration`]) and two samplers cannot disagree.
///
/// ```text
/// elapsed:  0 ........ delay ........... delay + duration ......
/// value:    from       from  ~eased~     to                 to
/// ```
///
/// The value stays between `from` and `to` (the order does not matter, and
/// either may be negative): the curves never overshoot and the endpoints are
/// returned as given, not recomputed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    /// The value before the delay ends, and at its start.
    pub from: f32,
    /// The value once the tween is [finished](Tween::is_finished).
    pub to: f32,
    /// How long the move takes, once the delay has passed. Zero is an instant
    /// jump: see [`Tween::sample`].
    pub duration: Duration,
    /// How long the tween holds `from` before it starts to move.
    pub delay: Duration,
    /// The timing curve of the move.
    pub easing: Easing,
}

impl Tween {
    /// A [linear](Easing::Linear) tween from `from` to `to` over `duration`,
    /// starting at once. Chain [`Tween::with_delay`] and
    /// [`Tween::with_easing`].
    pub const fn new(from: f32, to: f32, duration: Duration) -> Tween {
        Tween {
            from,
            to,
            duration,
            delay: Duration::ZERO,
            easing: Easing::Linear,
        }
    }

    /// This tween, holding `from` for `delay` before it starts to move.
    pub const fn with_delay(mut self, delay: Duration) -> Tween {
        self.delay = delay;
        self
    }

    /// This tween with the timing curve `easing`.
    pub const fn with_easing(mut self, easing: Easing) -> Tween {
        self.easing = easing;
        self
    }

    /// The elapsed time at which the tween ends: `delay + duration`, saturating
    /// at [`Duration::MAX`] instead of overflowing.
    ///
    /// When the sum saturates, `total()` is only a lower bound on the end, so
    /// do not compare an elapsed time against it: [`Tween::is_finished`] is
    /// authoritative (it never forms the sum).
    pub fn total(&self) -> Duration {
        self.delay.saturating_add(self.duration)
    }

    /// Whether the tween has reached `to` at `elapsed`: true from
    /// `delay + duration` on, and so, for a zero duration, from the delay on.
    pub fn is_finished(&self, elapsed: Duration) -> bool {
        elapsed
            .checked_sub(self.delay)
            .is_some_and(|running| running >= self.duration)
    }

    /// The value `elapsed` after the tween started.
    ///
    /// - Before the delay has passed it is `from`.
    /// - At `delay + duration` and after it is exactly `to`.
    /// - In between it is `from` moved towards `to` by the easing curve.
    /// - A **zero duration** has no in-between: the value is `from` until the
    ///   delay, and `to` from the delay on.
    /// - If `from` or `to` is not finite (NaN or infinite) there is nothing to
    ///   interpolate between, so the tween holds `from` until it ends and then
    ///   returns `to`, rather than producing NaN from a finite pair.
    pub fn sample(&self, elapsed: Duration) -> f32 {
        let Some(running) = elapsed.checked_sub(self.delay) else {
            return self.from;
        };
        if running >= self.duration {
            return self.to;
        }
        if !(self.from.is_finite() && self.to.is_finite()) {
            return self.from;
        }
        // `running < duration`, so the duration is not zero and `t` is in
        // `[0, 1]`. Both are in `f64` so a long tween keeps its precision.
        let t = running.as_secs_f64() / self.duration.as_secs_f64();
        let eased = f64::from(self.easing.ease(t as f32));
        // A weighted sum rather than `from + (to - from) * eased`: the
        // difference overflows for endpoints of opposite sign near `f32::MAX`,
        // and this form is exact at `eased` of 0 and 1.
        (f64::from(self.from) * (1.0 - eased) + f64::from(self.to) * eased) as f32
    }
}

#[cfg(test)]
mod tests;
