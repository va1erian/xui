#![forbid(unsafe_code)]

//! Frame pacing: when each window's next [`Event::Frame`] is due, and the
//! hooks that let the message loop deliver them.

use std::collections::HashMap;
use std::time::Duration;

use xui_core::backend::{Event, WidgetId, WindowId};

use super::Shared;
use crate::sys::frame_timer::FrameTimer;
use crate::sys::window::{self as sys, Frames};

/// The rate assumed when Windows does not report one.
const DEFAULT_HZ: u32 = 60;

/// The time between two refreshes of a display running at `hz`. Windows
/// reports 0 and 1 for "the hardware default", which says nothing, so those
/// count as 60 Hz.
pub(super) fn period(hz: u32) -> Duration {
    let hz = if hz > 1 { hz } else { DEFAULT_HZ };
    Duration::from_secs_f64(1.0 / f64::from(hz))
}

/// When a frame asked for at `now` is due: one `period` after the previous
/// frame, and never in the past.
pub(super) fn deadline(now: Duration, last: Option<Duration>, period: Duration) -> Duration {
    last.map_or(now, |last| (last + period).max(now))
}

/// The time to count a frame due at `due` as delivered at, which paces the
/// next one. A frame delivered a little late keeps its slot, so the frames
/// advance by whole periods instead of drifting; one a whole period or more
/// late (the loop was busy or idle) restarts from `now`.
pub(super) fn slot(due: Duration, now: Duration, period: Duration) -> Duration {
    if now.saturating_sub(due) < period {
        due
    } else {
        now
    }
}

/// The frames asked for and the last one delivered, by window.
#[derive(Default)]
pub(super) struct Pacer {
    /// Each pending frame's due time and the period it was asked at.
    pending: HashMap<u64, (Duration, Duration)>,
    last: HashMap<u64, Duration>,
}

impl Pacer {
    /// Records a frame request, with the refresh `period` of the window's
    /// display. Asking again while one is pending changes nothing.
    fn request(&mut self, window: u64, now: Duration, period: Duration) {
        let last = self.last.get(&window).copied();
        self.pending
            .entry(window)
            .or_insert_with(|| (deadline(now, last, period), period));
    }

    /// How long until the earliest pending frame, zero when it is due.
    fn until_next(&self, now: Duration) -> Option<Duration> {
        let (due, _) = self.pending.values().min_by_key(|(due, _)| *due)?;
        Some(due.saturating_sub(now))
    }

    /// Removes one frame due at `now`, if any, and records its time, so the
    /// next request is paced from it. Called before the frame is delivered,
    /// since the handler asks again.
    fn take_due(&mut self, now: Duration) -> Option<u64> {
        let (&window, &(due, period)) = self
            .pending
            .iter()
            .filter(|(_, (due, _))| *due <= now)
            .min_by_key(|(_, (due, _))| *due)?;
        self.pending.remove(&window);
        self.last.insert(window, slot(due, now, period));
        Some(window)
    }

    /// Forgets a closed window.
    fn forget(&mut self, window: u64) {
        self.pending.remove(&window);
        self.last.remove(&window);
    }
}

impl Shared {
    /// The backend's clock: time since it was created.
    pub(super) fn elapsed(&self) -> Duration {
        self.clock.elapsed()
    }

    /// The refresh period of the display `window` is on, read at each request
    /// so a window dragged to another monitor follows it.
    fn period_of(&self, window: u64) -> Duration {
        let hwnd = self.windows.borrow().get(&window).map(|state| state.hwnd);
        period(hwnd.map_or(0, sys::refresh_hz))
    }

    /// Asks for one frame on `window`; `false` when frames cannot be paced.
    pub(super) fn request_frame(&self, window: WindowId) -> bool {
        if self.frame_timer.is_none() || !self.windows.borrow().contains_key(&window.raw()) {
            return false;
        }
        let period = self.period_of(window.raw());
        self.pacer
            .borrow_mut()
            .request(window.raw(), self.elapsed(), period);
        true
    }

    /// Forgets the frames of a closed window.
    pub(super) fn forget_frames(&self, window: WindowId) {
        self.pacer.borrow_mut().forget(window.raw());
    }
}

impl Frames for Shared {
    fn timer(&self) -> Option<&FrameTimer> {
        self.frame_timer.as_ref()
    }

    fn until_next(&self) -> Option<Duration> {
        self.pacer.borrow().until_next(self.elapsed())
    }

    fn deliver_due(&self) {
        // One reading of the clock for the whole call: a handler that takes
        // longer than a period and asks again is not delivered twice, which
        // would run frames back to back without pumping messages. The pacer
        // is not borrowed while a frame runs: the handler asks for the next
        // one, and may close windows.
        let now = self.elapsed();
        loop {
            let due = self.pacer.borrow_mut().take_due(now);
            let Some(window) = due else {
                break;
            };
            self.deliver(WindowId::from_raw(window), WidgetId::NONE, &Event::Frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn period_follows_the_refresh_rate() {
        assert_eq!(period(240), Duration::from_secs_f64(1.0 / 240.0));
        assert_eq!(period(60), Duration::from_secs_f64(1.0 / 60.0));
    }

    #[test]
    fn the_hardware_default_counts_as_60_hz() {
        for hz in [0, 1] {
            assert_eq!(period(hz), period(60));
        }
    }

    #[test]
    fn the_first_frame_is_due_at_once() {
        assert_eq!(deadline(ms(5), None, ms(16)), ms(5));
    }

    #[test]
    fn a_frame_waits_for_the_period_after_the_last() {
        assert_eq!(deadline(ms(20), Some(ms(10)), ms(16)), ms(26));
    }

    #[test]
    fn a_deadline_is_never_in_the_past() {
        assert_eq!(deadline(ms(100), Some(ms(10)), ms(16)), ms(100));
    }

    #[test]
    fn a_slightly_late_frame_keeps_its_slot() {
        assert_eq!(slot(ms(26), ms(29), ms(16)), ms(26));
        let next = deadline(ms(29), Some(slot(ms(26), ms(29), ms(16))), ms(16));
        assert_eq!(next, ms(42));
    }

    #[test]
    fn a_frame_a_period_late_restarts_from_now() {
        assert_eq!(slot(ms(26), ms(60), ms(16)), ms(60));
    }

    #[test]
    fn asking_twice_keeps_the_first_deadline() {
        let mut pacer = Pacer::default();
        pacer.request(1, ms(10), ms(16));
        pacer.request(1, ms(12), ms(16));
        assert_eq!(pacer.until_next(ms(10)), Some(Duration::ZERO));
        assert_eq!(pacer.take_due(ms(10)), Some(1));
        assert_eq!(pacer.take_due(ms(10)), None);
    }

    #[test]
    fn a_delivered_frame_paces_the_next_request() {
        let mut pacer = Pacer::default();
        pacer.request(1, Duration::ZERO, ms(16));
        assert_eq!(pacer.take_due(ms(2)), Some(1));
        pacer.request(1, ms(2), ms(16));
        assert_eq!(pacer.until_next(ms(2)), Some(ms(14)));
        assert_eq!(pacer.take_due(ms(10)), None);
    }

    #[test]
    fn a_forgotten_window_has_no_frame() {
        let mut pacer = Pacer::default();
        pacer.request(1, Duration::ZERO, ms(16));
        pacer.forget(1);
        assert_eq!(pacer.until_next(Duration::ZERO), None);
    }
}
