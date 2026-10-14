#![forbid(unsafe_code)]

//! [`Easing`]: the curves that shape a [`Tween`](super::Tween).

/// The exponent the Windows Phone Toolkit's transitions give their
/// `ExponentialEase`: a sharp start that settles slowly. See [`Easing::expo_in`]
/// and [`Easing::expo_out`].
pub const DEFAULT_EXPONENT: f32 = 6.0;

/// A timing curve: a pure map from normalised time `t` in `[0, 1]` to normalised
/// progress in `[0, 1]`.
///
/// Every curve starts at exactly `0.0` and ends at exactly `1.0` (see
/// [`Easing::ease`]), never leaves `[0, 1]` (no overshoot, no bounce) and never
/// goes backwards. That is what lets a [`Tween`](super::Tween) promise that it
/// stays between its endpoints.
///
/// *In* curves start slowly and finish fast; *out* curves start fast and settle
/// slowly. Metro motion mostly uses the exponential out curve: "fast in, slow
/// out".
///
/// # On the web
///
/// An exponential is not a cubic Bezier, so the web side cannot hand
/// [`Easing::ExpoIn`] and [`Easing::ExpoOut`] to CSS as a `cubic-bezier()`: it
/// will approximate them with a CSS `linear()` function sampled from
/// [`Easing::ease`], and its fixtures carry a tolerance. A native renderer
/// evaluates [`Easing::ease`] directly and is exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    /// `f(t) = t`: constant speed.
    Linear,
    /// `f(t) = t²`: starts slowly, accelerates.
    QuadIn,
    /// `f(t) = 1 - (1 - t)²`: starts fast, decelerates.
    QuadOut,
    /// [`Easing::QuadIn`] for the first half, [`Easing::QuadOut`] for the
    /// second, meeting at `f(0.5) = 0.5`.
    QuadInOut,
    /// `f(t) = t³`: starts slowly, accelerates harder than [`Easing::QuadIn`].
    CubicIn,
    /// `f(t) = 1 - (1 - t)³`: starts fast, decelerates harder than
    /// [`Easing::QuadOut`].
    CubicOut,
    /// [`Easing::CubicIn`] for the first half, [`Easing::CubicOut`] for the
    /// second, meeting at `f(0.5) = 0.5`.
    CubicInOut,
    /// The Windows Phone Toolkit's `ExponentialEase` as an ease-in:
    /// `f(t) = (e^(k t) - 1) / (e^k - 1)` for the `exponent` `k`.
    ///
    /// The larger `k`, the longer it lingers near zero and the steeper the end.
    /// An exponent of zero (or NaN, which is not rejected) is
    /// [`Easing::Linear`]. A negative exponent is the ease-out of the same
    /// magnitude: `ExpoIn { exponent: -k }` equals `ExpoOut { exponent: k }`.
    /// An infinite exponent is a step: at the end for `ExpoIn`, at the start
    /// for `ExpoOut`.
    ExpoIn {
        /// The steepness `k` of the exponential. [`DEFAULT_EXPONENT`] is `6`.
        exponent: f32,
    },
    /// The Windows Phone Toolkit's `ExponentialEase` as an ease-out, the mirror
    /// of [`Easing::ExpoIn`]: `f(t) = 1 - g(1 - t)` where `g` is the ease-in of
    /// the same `exponent`.
    ///
    /// Fast at the start and slow at the end: with the default exponent of
    /// `6` it is over 90% of the way by `t = 0.4`. This is the curve of Metro's
    /// transitions.
    ExpoOut {
        /// The steepness `k` of the exponential. [`DEFAULT_EXPONENT`] is `6`.
        exponent: f32,
    },
}

impl Easing {
    /// [`Easing::ExpoIn`] with the Toolkit's [`DEFAULT_EXPONENT`].
    pub const fn expo_in() -> Easing {
        Easing::ExpoIn {
            exponent: DEFAULT_EXPONENT,
        }
    }

    /// [`Easing::ExpoOut`] with the Toolkit's [`DEFAULT_EXPONENT`].
    pub const fn expo_out() -> Easing {
        Easing::ExpoOut {
            exponent: DEFAULT_EXPONENT,
        }
    }

    /// Maps normalised time `t` to normalised progress.
    ///
    /// `t` is clamped to `[0, 1]`, so `ease(0.0) == 0.0` and `ease(1.0) ==
    /// 1.0` hold exactly for every curve, and a `t` of NaN is treated as the
    /// start (`0.0`). The result is always within `[0, 1]`.
    pub fn ease(self, t: f32) -> f32 {
        if t.is_nan() || t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let progress = match self {
            Easing::Linear => t,
            Easing::QuadIn => t * t,
            Easing::QuadOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::QuadInOut if t < 0.5 => 2.0 * t * t,
            Easing::QuadInOut => 1.0 - 2.0 * (1.0 - t) * (1.0 - t),
            Easing::CubicIn => t * t * t,
            Easing::CubicOut => 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t),
            Easing::CubicInOut if t < 0.5 => 4.0 * t * t * t,
            Easing::CubicInOut => 1.0 - 4.0 * (1.0 - t) * (1.0 - t) * (1.0 - t),
            Easing::ExpoIn { exponent } => exponential(f64::from(exponent), f64::from(t)),
            // `1 - g(1 - t)` for `g` with exponent `k` is `g` with exponent
            // `-k`, which is the form that does not cancel near `t = 0`.
            Easing::ExpoOut { exponent } => exponential(-f64::from(exponent), f64::from(t)),
        };
        // Rounding must never push a curve outside the range a tween relies on.
        progress.clamp(0.0, 1.0)
    }
}

/// `(e^(k t) - 1) / (e^k - 1)` for `t` strictly inside `(0, 1)`, as an `f32`.
///
/// Evaluated with `exp_m1` so a small `k` keeps its precision, and rewritten
/// for `k > 0` as `e^(k (t - 1)) * (e^(-k t) - 1) / (e^(-k) - 1)` so that no
/// exponent is ever positive: the textbook form overflows to `inf / inf` for
/// `k` above about 88 (`f32`) or 709 (`f64`).
fn exponential(k: f64, t: f64) -> f32 {
    // The closed form is `0 / 0` at an exponent of zero, and its limit there
    // is `t`. A subnormal `f32` exponent needs no special case: `exp_m1`
    // returns its argument unchanged that close to zero, so the ratio is `t`.
    if k == 0.0 || k.is_nan() {
        return t as f32;
    }
    let progress = if k > 0.0 {
        (k * (t - 1.0)).exp() * (-k * t).exp_m1() / (-k).exp_m1()
    } else {
        (k * t).exp_m1() / k.exp_m1()
    };
    progress as f32
}

#[cfg(test)]
mod tests;
