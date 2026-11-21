#![forbid(unsafe_code)]

//! Scripting time on the headless backend: its clock, its frame requests and
//! its timers, so a test steps an animation by hand.

use std::time::Duration;

use super::HeadlessBackend;
use crate::backend::TimerId;

impl HeadlessBackend {
    /// Scripts the answer [`Backend::now`] gives. Until this or
    /// [`HeadlessBackend::advance`] is called the backend has no clock, so
    /// every animation is an instant change.
    ///
    /// [`Backend::now`]: super::Backend::now
    pub fn set_now(&self, now: Duration) {
        self.state.borrow_mut().now = Some(now);
    }

    /// Moves the scripted clock forward by `by`, starting it at zero if it
    /// was not set.
    pub fn advance(&self, by: Duration) {
        let mut state = self.state.borrow_mut();
        state.now = Some(state.now.unwrap_or_default().saturating_add(by));
    }

    /// Scripts whether [`Backend::request_frame`] is honoured. `false` is a
    /// backend with no frame pacing, which the front layer ticks with a timer.
    ///
    /// [`Backend::request_frame`]: super::Backend::request_frame
    pub fn set_frame_pacing(&self, on: bool) {
        self.state.borrow_mut().frame_pacing = on;
    }

    /// How many frames were asked for through [`Backend::request_frame`]. A
    /// test delivers each as an [`Event::Frame`] itself.
    ///
    /// [`Backend::request_frame`]: super::Backend::request_frame
    pub fn frame_requests(&self) -> u32 {
        self.state.borrow().frame_requests
    }

    /// The timers started and not yet killed.
    pub fn live_timers(&self) -> Vec<TimerId> {
        self.state.borrow().timers.clone()
    }
}
