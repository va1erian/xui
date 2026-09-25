#![forbid(unsafe_code)]

//! Input vocabulary shared by every backend: virtual keys, modifier state,
//! mouse buttons and hit-test results. These are deliberately free of any
//! platform handle or message type.

mod input;

pub use input::{HitTest, Key, Modifiers, MouseButton};
