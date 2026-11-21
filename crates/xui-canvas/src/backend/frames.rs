#![forbid(unsafe_code)]

//! Frame pacing for [`WinitBackend`](super::WinitBackend): the pending
//! [`Backend::request_frame`](xui_core::backend::Backend::request_frame)s and
//! the pure arithmetic that spaces them one display refresh apart.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use xui_core::backend::WindowId;

/// The refresh rate assumed when the monitor's is unknown, in millihertz.
const FALLBACK_MILLIHERTZ: u32 = 60_000;

/// The lowest refresh rate taken at its word, in millihertz. Windows reports a
/// frequency of 0 or 1 for "hardware default" and `winit` passes it on (as
/// whole hertz times 1000), which would pace frames one per second; no real
/// display refreshes below this.
const MIN_PLAUSIBLE_MILLIHERTZ: u32 = 10_000;

/// The time between two refreshes of a display running at `millihertz`, as
/// `winit` reports it: whole hertz times 1000 on Windows, possibly fractional
/// elsewhere (`59940` for 59.94 Hz). `None` and anything under
/// [`MIN_PLAUSIBLE_MILLIHERTZ`] mean the rate is unknown and give the period
/// of 60 Hz.
pub(crate) fn refresh_period(millihertz: Option<u32>) -> Duration {
    let millihertz = millihertz
        .filter(|&rate| rate >= MIN_PLAUSIBLE_MILLIHERTZ)
        .unwrap_or(FALLBACK_MILLIHERTZ);
    Duration::from_nanos(1_000_000_000_000 / u64::from(millihertz))
}

/// When a frame asked for at `now` is due: one `period` after `last` (the
/// previous frame delivered to the same window), but never before `now`.
///
/// `last` is when the previous frame was due, not when it was delivered, so
/// the loop's wake-up latency does not add up frame after frame and lower the
/// rate. A frame more than a period late, or a window that was idle for a
/// while, starts again from `now`.
pub(crate) fn frame_deadline(now: Instant, last: Option<Instant>, period: Duration) -> Instant {
    match last {
        Some(last) => now.max(last + period),
        None => now,
    }
}

/// The per-window frame requests waiting for their deadline, and when each
/// window's last delivered frame was due.
#[derive(Default)]
pub(crate) struct Frames {
    pending: HashMap<u64, Instant>,
    last: HashMap<u64, Instant>,
}

impl Frames {
    /// Notes that `window` wants a frame. A request while one is pending
    /// changes nothing, so asking twice still delivers one.
    pub(crate) fn request(&mut self, window: WindowId, now: Instant, period: Duration) {
        let last = self.last.get(&window.raw()).copied();
        self.pending
            .entry(window.raw())
            .or_insert_with(|| frame_deadline(now, last, period));
    }

    /// Removes and returns the window whose frame is due at `now`, the
    /// earliest first, recording its deadline as its last frame. The caller
    /// delivers after this returns, so the handler's own `request_frame`
    /// schedules the next one.
    pub(crate) fn take_due(&mut self, now: Instant) -> Option<WindowId> {
        let (window, at) = self
            .pending
            .iter()
            .filter(|(_, at)| **at <= now)
            .min_by_key(|(_, at)| **at)
            .map(|(window, at)| (*window, *at))?;
        self.pending.remove(&window);
        self.last.insert(window, at);
        Some(WindowId::from_raw(window))
    }

    /// The earliest pending deadline, for the event loop's wake-up.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.pending.values().copied().min()
    }

    /// Drops everything known about `window`, which closed.
    pub(crate) fn forget(&mut self, window: WindowId) {
        self.pending.remove(&window.raw());
        self.last.remove(&window.raw());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn the_period_follows_the_millihertz_rate() {
        assert_eq!(
            refresh_period(Some(60_000)),
            Duration::from_nanos(16_666_666)
        );
        let fast = refresh_period(Some(240_080));
        assert!(
            fast > 4 * MS && fast < Duration::from_micros(4_200),
            "{fast:?}"
        );
    }

    #[test]
    fn an_unknown_rate_is_sixty_hertz() {
        let sixty = refresh_period(Some(60_000));
        assert_eq!(refresh_period(None), sixty);
        assert_eq!(refresh_period(Some(0)), sixty);
    }

    #[test]
    fn a_hardware_default_rate_is_sixty_hertz() {
        let sixty = refresh_period(Some(60_000));
        assert_eq!(refresh_period(Some(1_000)), sixty, "Windows reports 1 Hz");
        assert_eq!(refresh_period(Some(9_999)), sixty);
        assert_eq!(refresh_period(Some(10_000)), Duration::from_millis(100));
    }

    #[test]
    fn the_first_frame_is_due_at_once() {
        let now = Instant::now();
        assert_eq!(frame_deadline(now, None, 16 * MS), now);
    }

    #[test]
    fn a_frame_waits_out_the_rest_of_the_period() {
        let last = Instant::now();
        let now = last + 5 * MS;
        assert_eq!(frame_deadline(now, Some(last), 16 * MS), last + 16 * MS);
    }

    #[test]
    fn a_deadline_is_never_in_the_past() {
        let last = Instant::now();
        let now = last + 40 * MS;
        assert_eq!(frame_deadline(now, Some(last), 16 * MS), now);
    }

    #[test]
    fn a_slightly_late_frame_does_not_delay_the_next() {
        let period = 16 * MS;
        let mut frames = Frames::default();
        let window = WindowId::from_raw(1);
        let start = Instant::now();
        frames.request(window, start, period);
        let delivered = start + 3 * MS;
        assert_eq!(frames.take_due(delivered), Some(window));
        frames.request(window, delivered, period);
        assert_eq!(frames.next_deadline(), Some(start + period));
    }

    #[test]
    fn a_frame_more_than_a_period_late_starts_again_from_now() {
        let period = 16 * MS;
        let mut frames = Frames::default();
        let window = WindowId::from_raw(1);
        let start = Instant::now();
        frames.request(window, start, period);
        let delivered = start + 40 * MS;
        assert_eq!(frames.take_due(delivered), Some(window));
        frames.request(window, delivered, period);
        assert_eq!(frames.next_deadline(), Some(delivered));
    }

    #[test]
    fn a_second_request_changes_nothing() {
        let mut frames = Frames::default();
        let window = WindowId::from_raw(1);
        let now = Instant::now();
        frames.request(window, now, 16 * MS);
        frames.request(window, now + 100 * MS, 16 * MS);
        assert_eq!(frames.next_deadline(), Some(now));
    }

    #[test]
    fn only_due_windows_are_taken_and_a_closed_one_is_forgotten() {
        let mut frames = Frames::default();
        let (a, b) = (WindowId::from_raw(1), WindowId::from_raw(2));
        let now = Instant::now();
        frames.request(a, now, 16 * MS);
        assert_eq!(frames.take_due(now), Some(a));
        frames.request(a, now, 16 * MS);
        frames.request(b, now, 16 * MS);
        assert_eq!(
            frames.take_due(now),
            Some(b),
            "a is still inside its period"
        );
        assert_eq!(frames.take_due(now), None);
        frames.forget(a);
        assert_eq!(frames.next_deadline(), None);
        frames.request(a, now, 16 * MS);
        assert_eq!(frames.next_deadline(), Some(now), "forgotten history");
    }

    #[test]
    fn the_earliest_due_window_comes_first() {
        let mut frames = Frames::default();
        let (a, b) = (WindowId::from_raw(1), WindowId::from_raw(2));
        let now = Instant::now();
        frames.request(a, now + 2 * MS, 16 * MS);
        frames.request(b, now, 16 * MS);
        let later = now + 5 * MS;
        assert_eq!(frames.take_due(later), Some(b));
        assert_eq!(frames.take_due(later), Some(a));
        assert_eq!(frames.take_due(later), None);
    }
}
