#![forbid(unsafe_code)]

//! [`Clock`]: where "now" comes from, so the motion code never reads a clock
//! itself.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
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
///
/// A reference to a clock, and a `Box`, `Rc` or `Arc` of one (trait objects
/// included), is itself a clock, so `fn drive(clock: impl Clock)` takes a
/// clock by value, by `&` or shared.
pub trait Clock {
    /// The time elapsed since this clock's origin.
    fn now(&self) -> Duration;
}

impl<C: Clock + ?Sized> Clock for &C {
    fn now(&self) -> Duration {
        (**self).now()
    }
}

impl<C: Clock + ?Sized> Clock for Box<C> {
    fn now(&self) -> Duration {
        (**self).now()
    }
}

impl<C: Clock + ?Sized> Clock for Rc<C> {
    fn now(&self) -> Duration {
        (**self).now()
    }
}

impl<C: Clock + ?Sized> Clock for Arc<C> {
    fn now(&self) -> Duration {
        (**self).now()
    }
}

/// A [`Clock`] that moves only when told to, for tests and headless runs.
///
/// It reads through `&self`, so it can be shared (`Rc<ManualClock>`, or `&`)
/// between a driver and the test that advances it. Like the rest of the front
/// layer it is single-threaded (`Cell` inside, so not `Sync`).
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
    use super::*;

    /// Takes any clock by value, as a driver generic over `impl Clock` does.
    fn drive(clock: impl Clock) -> Duration {
        clock.now()
    }

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
    fn a_clock_is_accepted_by_reference_box_rc_and_arc() {
        let clock = ManualClock::new();
        clock.advance(Duration::from_millis(40));
        assert_eq!(drive(&clock), Duration::from_millis(40), "&ManualClock");

        let shared = Rc::new(ManualClock::new());
        shared.advance(Duration::from_millis(7));
        assert_eq!(drive(Rc::clone(&shared)), Duration::from_millis(7), "Rc");
        shared.advance(Duration::from_millis(1));
        assert_eq!(drive(Rc::clone(&shared)), Duration::from_millis(8));

        // `ManualClock` is not `Sync`, so an `Arc` needs a clock that is.
        struct Fixed(Duration);
        impl Clock for Fixed {
            fn now(&self) -> Duration {
                self.0
            }
        }
        let atomic = Arc::new(Fixed(Duration::from_secs(3)));
        assert_eq!(drive(Arc::clone(&atomic)), Duration::from_secs(3), "Arc");

        let boxed = Box::new(ManualClock::new());
        boxed.set(Duration::from_secs(9));
        assert_eq!(drive(boxed), Duration::from_secs(9), "Box");
    }

    #[test]
    fn trait_objects_are_clocks_too() {
        let clock = ManualClock::new();
        clock.set(Duration::from_millis(11));
        let by_ref: &dyn Clock = &clock;
        assert_eq!(drive(by_ref), Duration::from_millis(11), "&dyn Clock");

        let boxed: Box<dyn Clock> = Box::new(ManualClock::new());
        assert_eq!(drive(boxed), Duration::ZERO, "Box<dyn Clock>");

        let shared: Rc<dyn Clock> = Rc::new(ManualClock::new());
        assert_eq!(drive(shared), Duration::ZERO, "Rc<dyn Clock>");
    }

    #[test]
    fn a_shared_clock_is_seen_through_a_trait_object() {
        let clock = Rc::new(ManualClock::new());
        let reader: Rc<dyn Clock> = clock.clone();
        clock.advance(Duration::from_secs(2));
        assert_eq!(reader.now(), Duration::from_secs(2));
    }
}
