//! The frame tick: a high-resolution waitable timer the message pump waits on
//! next to the message queue, which `WM_TIMER` (about 15 ms granularity)
//! cannot match.

use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, INFINITE, SetWaitableTimer,
    TIMER_ALL_ACCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, QS_ALLINPUT,
};
use windows::core::PCWSTR;

/// A one-shot, high-resolution timer owned by the backend.
pub(crate) struct FrameTimer(HANDLE);

/// What ended a [`FrameTimer::wait`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Woke {
    /// The delay elapsed.
    Timer,
    /// A message is waiting in the queue.
    Message,
}

impl FrameTimer {
    /// Creates the timer, or `None` where Windows has no high-resolution
    /// timers (before Windows 10 1803).
    pub(crate) fn new() -> Option<FrameTimer> {
        // SAFETY: an unnamed timer with default security; the handle is closed
        // in `Drop`.
        unsafe {
            CreateWaitableTimerExW(
                None,
                PCWSTR::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
        }
        .ok()
        .map(FrameTimer)
    }

    /// Arms the timer for `delay` from now, replacing any earlier arming, and
    /// blocks until it fires or a message arrives.
    pub(crate) fn wait(&self, delay: Duration) -> Woke {
        // A negative due time is relative, in 100 ns units; zero would mean an
        // absolute time in 1601.
        let units = ((delay.as_nanos() / 100) as i64).max(1);
        let relative = -units;
        // SAFETY: `self.0` is a live waitable timer and `relative` outlives the
        // call; the wait only reads the one-element handle array.
        unsafe {
            if SetWaitableTimer(self.0, &relative, 0, None, None, false).is_ok() {
                // With one handle, `WAIT_OBJECT_0` is the timer and the next
                // value is input.
                let woke = MsgWaitForMultipleObjectsEx(
                    Some(&[self.0]),
                    INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
                if woke == WAIT_OBJECT_0 {
                    return Woke::Timer;
                }
                if woke.0 == WAIT_OBJECT_0.0 + 1 {
                    return Woke::Message;
                }
            }
        }
        Self::wait_millis(delay)
    }

    /// The fallback when the timer cannot be armed or the wait failed, which
    /// would otherwise spin the loop: time out in whole milliseconds instead.
    fn wait_millis(delay: Duration) -> Woke {
        let millis = (delay.as_millis() as u32).clamp(1, 1000);
        // SAFETY: no handles are waited on; the call only reads the queue state.
        let woke =
            unsafe { MsgWaitForMultipleObjectsEx(None, millis, QS_ALLINPUT, MWMO_INPUTAVAILABLE) };
        // With no handles, `WAIT_OBJECT_0` is input.
        if woke == WAIT_OBJECT_0 {
            Woke::Message
        } else {
            Woke::Timer
        }
    }
}

impl Drop for FrameTimer {
    fn drop(&mut self) {
        // SAFETY: the handle created in `new`, not used afterwards.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
