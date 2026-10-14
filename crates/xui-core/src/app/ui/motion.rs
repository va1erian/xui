#![forbid(unsafe_code)]

//! [`Ui::prefers_reduced_motion`]: the operating system's animation preference,
//! as the backend reports it.

use super::Ui;

impl<M: 'static> Ui<M> {
    /// Whether the user has asked the system to reduce motion.
    ///
    /// Anything that animates must check this when it starts: when it is `true`
    /// motion resolves to a plain fade or an instant change, and nothing loops.
    /// It is read live from the backend, so a change made while the app runs
    /// applies to the next animation. A backend with no way to read the setting
    /// reports `false`; see
    /// [`Backend::prefers_reduced_motion`](crate::backend::Backend::prefers_reduced_motion)
    /// for which do.
    pub fn prefers_reduced_motion(&self) -> bool {
        self.core.backend().prefers_reduced_motion()
    }
}
