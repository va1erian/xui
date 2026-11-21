#![forbid(unsafe_code)]

//! Scripting time on the offscreen backend: its clock and its frame requests,
//! so a test steps an animation by hand. A render that scripts nothing has no
//! clock, so every animation is an instant change and a snapshot is the same
//! on every run.

use std::time::Duration;

use super::OffscreenBackend;

impl OffscreenBackend {
    /// Scripts the answer [`Backend::now`] gives. Until this or
    /// [`OffscreenBackend::advance`] is called the backend has no clock, so
    /// every animation is an instant change.
    ///
    /// [`Backend::now`]: xui_core::backend::Backend::now
    pub fn set_now(&self, now: Duration) {
        self.now.set(Some(now));
    }

    /// Moves the scripted clock forward by `by`, starting it at zero if it
    /// was not set.
    pub fn advance(&self, by: Duration) {
        self.now
            .set(Some(self.now.get().unwrap_or_default().saturating_add(by)));
    }

    /// How many frames were asked for through [`Backend::request_frame`]. A
    /// test delivers each as an [`Event::Frame`] itself.
    ///
    /// [`Backend::request_frame`]: xui_core::backend::Backend::request_frame
    /// [`Event::Frame`]: xui_core::backend::Event::Frame
    pub fn frame_requests(&self) -> u32 {
        self.frame_requests.get()
    }
}
