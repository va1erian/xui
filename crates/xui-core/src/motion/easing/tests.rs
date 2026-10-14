use super::*;

/// Every curve, with exponential curves at several exponents (positive,
/// steep, shallow and negative).
fn curves() -> Vec<Easing> {
    let mut all = vec![
        Easing::Linear,
        Easing::QuadIn,
        Easing::QuadOut,
        Easing::QuadInOut,
        Easing::CubicIn,
        Easing::CubicOut,
        Easing::CubicInOut,
        Easing::expo_in(),
        Easing::expo_out(),
    ];
    for exponent in [
        0.01,
        1.0,
        2.0,
        12.0,
        40.0,
        300.0,
        -3.0,
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        all.push(Easing::ExpoIn { exponent });
        all.push(Easing::ExpoOut { exponent });
    }
    all
}

/// The textbook ease-in, `(e^(k t) - 1) / (e^k - 1)`, in `f64`.
fn closed_form_in(k: f64, t: f64) -> f64 {
    ((k * t).exp() - 1.0) / (k.exp() - 1.0)
}

/// The textbook ease-out, `1 - f(1 - t)`, in `f64`.
fn closed_form_out(k: f64, t: f64) -> f64 {
    1.0 - closed_form_in(k, 1.0 - t)
}

#[test]
fn every_curve_starts_at_exactly_zero_and_ends_at_exactly_one() {
    for easing in curves() {
        assert_eq!(easing.ease(0.0), 0.0, "{easing:?}");
        assert_eq!(easing.ease(1.0), 1.0, "{easing:?}");
    }
}

#[test]
fn every_curve_is_monotonic_and_stays_in_the_unit_range() {
    for easing in curves() {
        let mut previous = 0.0_f32;
        for i in 0..=1000 {
            let value = easing.ease(i as f32 / 1000.0);
            assert!((0.0..=1.0).contains(&value), "{easing:?} at {i}: {value}");
            assert!(
                value >= previous,
                "{easing:?} went backwards at {i}: {previous} -> {value}"
            );
            previous = value;
        }
    }
}

#[test]
fn polynomial_curves_hit_their_known_values() {
    assert_eq!(Easing::Linear.ease(0.25), 0.25);
    assert_eq!(Easing::QuadIn.ease(0.5), 0.25);
    assert_eq!(Easing::QuadOut.ease(0.5), 0.75);
    assert_eq!(Easing::CubicIn.ease(0.5), 0.125);
    assert_eq!(Easing::CubicOut.ease(0.5), 0.875);
    assert_eq!(Easing::QuadInOut.ease(0.5), 0.5);
    assert_eq!(Easing::CubicInOut.ease(0.5), 0.5);
    // Each half of an in-out curve is the matching in or out curve, squeezed.
    assert_eq!(Easing::QuadInOut.ease(0.25), 0.125);
    assert_eq!(Easing::QuadInOut.ease(0.75), 0.875);
    assert_eq!(Easing::CubicInOut.ease(0.25), 0.0625);
    assert_eq!(Easing::CubicInOut.ease(0.75), 0.9375);
}

#[test]
fn exponential_out_with_the_default_exponent_matches_the_closed_form() {
    let easing = Easing::ExpoOut {
        exponent: DEFAULT_EXPONENT,
    };
    for t in [0.1_f32, 0.5, 0.9] {
        let expected = closed_form_out(6.0, f64::from(t));
        let actual = f64::from(easing.ease(t));
        assert!(
            (actual - expected).abs() < 1.0e-6,
            "t = {t}: {actual} vs {expected}"
        );
    }
    // The fast start and the slow settle that give Metro its feel.
    assert!(easing.ease(0.4) > 0.9);
    assert!(easing.ease(0.1) > 0.4);
}

#[test]
fn exponential_in_matches_the_closed_form_for_several_exponents() {
    for k in [0.5_f32, 2.0, 6.0, 20.0] {
        let easing = Easing::ExpoIn { exponent: k };
        for t in [0.05_f32, 0.25, 0.5, 0.75, 0.95] {
            let expected = closed_form_in(f64::from(k), f64::from(t));
            let actual = f64::from(easing.ease(t));
            assert!(
                (actual - expected).abs() < 1.0e-6,
                "k = {k}, t = {t}: {actual} vs {expected}"
            );
        }
    }
}

#[test]
fn exponential_in_and_out_mirror_each_other() {
    for k in [0.5_f32, 2.0, 6.0, 20.0] {
        let ease_in = Easing::ExpoIn { exponent: k };
        let ease_out = Easing::ExpoOut { exponent: k };
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let mirrored = 1.0 - ease_in.ease(1.0 - t);
            assert!(
                (ease_out.ease(t) - mirrored).abs() < 1.0e-6,
                "k = {k}, t = {t}"
            );
        }
    }
}

#[test]
fn a_negative_exponent_swaps_in_and_out() {
    for k in [0.5_f32, 6.0] {
        for i in 0..=100 {
            let t = i as f32 / 100.0;
            let negated = f64::from(Easing::ExpoIn { exponent: -k }.ease(t));
            let expected = closed_form_out(f64::from(k), f64::from(t));
            assert!((negated - expected).abs() < 1.0e-6, "k = {k}, t = {t}");
        }
    }
}

#[test]
fn time_outside_the_unit_interval_clamps() {
    for easing in curves() {
        for below in [-0.001, -1.0, -1.0e30, f32::NEG_INFINITY] {
            assert_eq!(easing.ease(below), 0.0, "{easing:?} at {below}");
        }
        for above in [1.001, 2.0, 1.0e30, f32::INFINITY] {
            assert_eq!(easing.ease(above), 1.0, "{easing:?} at {above}");
        }
        assert!(easing.ease(-0.0).is_sign_positive(), "{easing:?}");
    }
}

#[test]
fn nan_time_is_the_start_and_does_not_panic() {
    for easing in curves() {
        assert_eq!(easing.ease(f32::NAN), 0.0, "{easing:?}");
    }
}

#[test]
fn degenerate_exponents_are_defined() {
    // Zero is the limit of the family and NaN is not rejected: both are linear.
    for exponent in [0.0, -0.0, f32::NAN] {
        for t in [0.1_f32, 0.5, 0.9] {
            assert_eq!(Easing::ExpoIn { exponent }.ease(t), t, "in {exponent}");
            assert_eq!(Easing::ExpoOut { exponent }.ease(t), t, "out {exponent}");
        }
    }
    // The smallest exponents keep their precision instead of becoming 0 / 0.
    for exponent in [1.0e-30, f32::from_bits(1)] {
        let value = Easing::ExpoIn { exponent }.ease(0.3);
        assert!((value - 0.3).abs() < 1.0e-6, "{exponent}: {value}");
    }
    // An infinite exponent is a step, with no NaN on the way.
    assert_eq!(
        Easing::ExpoIn {
            exponent: f32::INFINITY
        }
        .ease(0.999),
        0.0
    );
    assert_eq!(
        Easing::ExpoOut {
            exponent: f32::INFINITY
        }
        .ease(0.001),
        1.0
    );
    // A huge finite one is the same step and must not overflow to NaN.
    assert_eq!(Easing::ExpoIn { exponent: f32::MAX }.ease(0.5), 0.0);
    assert_eq!(Easing::ExpoOut { exponent: f32::MAX }.ease(0.5), 1.0);
}

#[test]
fn the_default_constructors_use_the_toolkit_exponent() {
    assert_eq!(DEFAULT_EXPONENT, 6.0);
    assert_eq!(
        Easing::expo_in(),
        Easing::ExpoIn {
            exponent: DEFAULT_EXPONENT
        }
    );
    assert_eq!(
        Easing::expo_out(),
        Easing::ExpoOut {
            exponent: DEFAULT_EXPONENT
        }
    );
}

#[test]
fn the_default_exponent_hits_its_known_literals() {
    // (e^3 - 1) / (e^6 - 1) = 0.047426, and its mirror 0.952574.
    let ease_in = Easing::expo_in().ease(0.5);
    let ease_out = Easing::expo_out().ease(0.5);
    assert!((ease_in - 0.04743).abs() < 1.0e-5, "{ease_in}");
    assert!((ease_out - 0.95257).abs() < 1.0e-5, "{ease_out}");
}

#[test]
fn negative_infinite_and_huge_negative_exponents_are_steps_without_nan() {
    for exponent in [f32::NEG_INFINITY, -f32::MAX] {
        // A negative exponent swaps in and out, so an ease-in of it jumps at
        // the start and an ease-out of it holds until the end.
        let ease_in = Easing::ExpoIn { exponent };
        let ease_out = Easing::ExpoOut { exponent };
        for t in [1.0e-6, 0.001, 0.5, 0.999, 0.999_999] {
            assert_eq!(ease_in.ease(t), 1.0, "ExpoIn {exponent} at {t}");
            assert_eq!(ease_out.ease(t), 0.0, "ExpoOut {exponent} at {t}");
        }
        assert_eq!(ease_in.ease(0.0), 0.0);
        assert_eq!(ease_in.ease(1.0), 1.0);
        assert_eq!(ease_out.ease(0.0), 0.0);
        assert_eq!(ease_out.ease(1.0), 1.0);
    }
}

/// The `f32` bit patterns of `[0, 1]`, every `stride`-th one, plus the first
/// few dense ones (the subnormals and tiny values) and `1.0` itself. Bit
/// patterns of non-negative floats are ordered like the values.
fn strided_unit_samples(stride: usize) -> impl Iterator<Item = f32> {
    let one = 1.0_f32.to_bits();
    (0..=one)
        .step_by(stride)
        .chain(one - 2..=one)
        .chain(0..2048)
        .map(f32::from_bits)
}

#[test]
fn every_curve_is_monotonic_across_the_f32_bit_patterns_of_the_unit_range() {
    for easing in curves() {
        let mut previous_input = -1.0_f32;
        let mut previous = 0.0_f32;
        // Sorted by input so the sweep compares neighbours; the dense head and
        // the tail were chained after the stride.
        let mut samples: Vec<f32> = strided_unit_samples(10_007).collect();
        samples.sort_by(f32::total_cmp);
        samples.dedup();
        for t in samples {
            let value = easing.ease(t);
            assert!((0.0..=1.0).contains(&value), "{easing:?} at {t:e}: {value}");
            assert!(
                value >= previous,
                "{easing:?} went backwards between {previous_input:e} and {t:e}:                  {previous} -> {value}"
            );
            previous_input = t;
            previous = value;
        }
        assert_eq!(previous, 1.0, "{easing:?} ends at 1.0");
    }
}
