#![forbid(unsafe_code)]

//! [`Ui::every`]: a repeating message without a timer mapping of its own.

use super::Ui;

impl<M: 'static> Ui<M> {
    /// Raises `msg` every `millis` milliseconds while the window lives, for a
    /// view that refreshes itself. It runs alongside [`Ui::on_timer`] and any
    /// other `every`.
    pub fn every(&self, millis: u32, msg: M)
    where
        M: Clone,
    {
        let timer = self.set_timer(millis);
        self.add_timer_listener(move |fired| (fired == timer).then(|| msg.clone()));
    }
}
