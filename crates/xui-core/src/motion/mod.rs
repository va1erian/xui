#![forbid(unsafe_code)]

//! Portable motion: the timing curves ([`Easing`]), an eased value over time
//! ([`Tween`]) and where time comes from ([`Clock`]).
//!
//! This is the backend-independent foundation of animation. It computes values;
//! it does not draw, schedule or own a timer, and it never reads the system
//! clock, so it uses nothing that is unavailable on `wasm32` and runs the same
//! in a headless test. What moves a widget (per-node opacity and transform, a
//! frame driver) builds on it separately. See `docs/motion.md` for what exists
//! and what does not.
//!
//! [`Backend::prefers_reduced_motion`](crate::Backend::prefers_reduced_motion)
//! and [`Ui::prefers_reduced_motion`](crate::Ui::prefers_reduced_motion) carry
//! the operating system's animation preference, which a caller of this module
//! must honour: when it is set, motion resolves to a plain fade or an instant
//! change and nothing loops.
//!
//! ```
//! use std::time::Duration;
//! use xui_core::motion::{Easing, Tween};
//!
//! let slide = Tween::new(40.0, 0.0, Duration::from_millis(300))
//!     .with_delay(Duration::from_millis(50))
//!     .with_easing(Easing::expo_out());
//!
//! assert_eq!(slide.sample(Duration::from_millis(10)), 40.0); // still delayed
//! assert!(slide.sample(Duration::from_millis(150)) < 10.0); // most of the way
//! assert_eq!(slide.sample(Duration::from_millis(350)), 0.0); // exactly `to`
//! assert!(slide.is_finished(Duration::from_millis(350)));
//! ```

mod clock;
mod easing;
mod tween;

pub use clock::{Clock, ManualClock};
pub use easing::{DEFAULT_EXPONENT, Easing};
pub use tween::Tween;
