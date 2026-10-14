//! The user's animation preference.
//!
//! `SPI_GETCLIENTAREAANIMATION` (`WinUser.h`) writes a `BOOL` that is `TRUE`
//! while client-area animation is on, which is what the "Animation effects"
//! switch in Settings (Accessibility, Visual effects) controls. xui asks the
//! opposite question ("should motion be reduced?"), so the answer is the
//! negation.

use core::ffi::c_void;

use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::BOOL;

/// Whether the system asks apps to reduce motion: client-area animation is
/// switched off. A failed query reports `false` (no preference), never a guess.
pub(crate) fn reduced_motion() -> bool {
    // Starts `TRUE`, so a call that fails to write leaves "animation on".
    let mut animation_enabled = BOOL::from(true);
    // SAFETY: `animation_enabled` is a valid, writable, correctly-sized `BOOL`
    // that outlives the call; `SPI_GETCLIENTAREAANIMATION` only writes a `BOOL`
    // through `pvParam`, and `uiParam` is unused (0) for this action.
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animation_enabled as *mut BOOL as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    reduced_from_animation(result.is_ok(), animation_enabled.as_bool())
}

/// The reduced-motion answer for a query result: `animation_enabled` is what
/// `SPI_GETCLIENTAREAANIMATION` reported, and only a successful query that
/// reported animation off asks for reduced motion. Pure, so the inverted sense
/// and the failure rule are unit-tested rather than depending on the machine.
fn reduced_from_animation(query_succeeded: bool, animation_enabled: bool) -> bool {
    query_succeeded && !animation_enabled
}

#[cfg(test)]
mod tests {
    use super::{reduced_from_animation, reduced_motion};

    #[test]
    fn animation_off_means_reduced_motion() {
        assert!(reduced_from_animation(true, false));
    }

    #[test]
    fn animation_on_means_no_preference() {
        assert!(!reduced_from_animation(true, true));
    }

    #[test]
    fn a_failed_query_is_no_preference_in_either_case() {
        assert!(!reduced_from_animation(false, false));
        assert!(!reduced_from_animation(false, true));
    }

    #[test]
    fn the_live_query_answers_and_is_stable() {
        // The value is the machine's, so only that the call is sound and
        // repeatable is asserted.
        assert_eq!(reduced_motion(), reduced_motion());
    }
}
