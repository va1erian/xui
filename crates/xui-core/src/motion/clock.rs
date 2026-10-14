#![forbid(unsafe_code)]

//! [`Clock`]: where "now" comes from, so the motion code never reads a clock
//! itself.

use std::cell::Cell;
use std::time::Duration;

/// A source of time since an arbitrary origin.
///
/// This crate never reads the system clock: it is portable (a web build has no
/// `std::time::Instant`), and a test or a headless run has to control time. A
/// driver takes a `Clock` (or just a [`Duration`]) from whoever owns real time:
/// the desktop canvas backend provides one over `std::time::Instant`
/// (`xui_canvas::MonotonicClock`), a web renderer would read
/// `performance.now()`, and a test uses a [`ManualClock`].
///
/// Only differences between two readings mean anything, since the origin is
/// arbitrary. A real clock must not go backwards.
pub trait Clock {
    /// The time elapsed since this clock's origin.
    fn now(&self) -> Duration;
}

/// A [`Clock`] that moves only when told to, for tests and headless runs.
///
/// It reads through `&self`, so it can be shared (`Rc<ManualClock>`) between a
/// driver and the test that advances it. Like the rest of the front layer it is
/// single-threaded.
///
/// ```
/// use std::time::Duration;
/// use xui_core::motion::{Clock, ManualClock};
///
/// let clock = ManualClock::new();
/// assert_eq!(clock.now(), Duration::ZERO);
/// clock.advance(Duration::from_millis(250));
/// assert_eq!(clock.now(), Duration::from_millis(250));
/// ```
#[derive(Debug, Default)]
pub struct ManualClock {
    now: Cell<Duration>,
}

impl ManualClock {
    /// A clock reading [`Duration::ZERO`].
    pub const fn new() -> ManualClock {
        ManualClock {
            now: Cell::new(Duration::ZERO),
        }
    }

    /// Sets the clock to `now`. Unlike a real clock this may move backwards,
    /// which a test uses to replay a stretch of time.
    pub fn set(&self, now: Duration) {
        self.now.set(now);
    }

    /// Moves the clock forward by `by`, saturating at [`Duration::MAX`]
    /// instead of overflowing.
    pub fn advance(&self, by: Duration) {
        self.now.set(self.now.get().saturating_add(by));
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.now.get()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    #[test]
    fn a_new_clock_reads_zero() {
        assert_eq!(ManualClock::new().now(), Duration::ZERO);
        assert_eq!(ManualClock::default().now(), Duration::ZERO);
    }

    #[test]
    fn set_replaces_the_reading_and_may_rewind() {
        let clock = ManualClock::new();
        clock.set(Duration::from_millis(900));
        assert_eq!(clock.now(), Duration::from_millis(900));
        clock.set(Duration::from_millis(100));
        assert_eq!(clock.now(), Duration::from_millis(100));
    }

    #[test]
    fn advance_accumulates() {
        let clock = ManualClock::new();
        clock.advance(Duration::from_millis(100));
        clock.advance(Duration::from_millis(150));
        assert_eq!(clock.now(), Duration::from_millis(250));
        clock.advance(Duration::ZERO);
        assert_eq!(clock.now(), Duration::from_millis(250));
    }

    #[test]
    fn advance_saturates_instead_of_overflowing() {
        let clock = ManualClock::new();
        clock.set(Duration::MAX);
        clock.advance(Duration::from_secs(1));
        assert_eq!(clock.now(), Duration::MAX);
    }

    #[test]
    fn a_shared_clock_is_seen_through_a_trait_object() {
        let clock = Rc::new(ManualClock::new());
        let reader: Rc<dyn Clock> = clock.clone();
        clock.advance(Duration::from_secs(2));
        assert_eq!(reader.now(), Duration::from_secs(2));
    }
}
