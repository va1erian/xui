use super::*;

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// 10 -> 20 over 1000 ms after a 200 ms delay, linear.
fn delayed() -> Tween {
    Tween::new(10.0, 20.0, ms(1000)).with_delay(ms(200))
}

#[test]
fn a_new_tween_is_linear_and_starts_at_once() {
    let tween = Tween::new(1.0, 3.0, ms(100));
    assert_eq!(tween.easing, Easing::Linear);
    assert_eq!(tween.delay, Duration::ZERO);
    assert_eq!(tween.total(), ms(100));
    assert_eq!(tween.sample(Duration::ZERO), 1.0);
    assert_eq!(tween.sample(ms(50)), 2.0);
}

#[test]
fn the_value_is_from_before_the_delay_and_at_it() {
    let tween = delayed();
    assert_eq!(tween.sample(Duration::ZERO), 10.0);
    assert_eq!(tween.sample(ms(199)), 10.0);
    assert_eq!(tween.sample(ms(200)), 10.0);
}

#[test]
fn the_value_moves_between_the_delay_and_the_end() {
    let tween = delayed();
    assert_eq!(tween.sample(ms(700)), 15.0);
    assert_eq!(tween.sample(ms(450)), 12.5);
    assert!(tween.sample(ms(1199)) < 20.0);
}

#[test]
fn the_value_is_exactly_to_at_the_end_and_after() {
    let tween = delayed();
    assert_eq!(tween.total(), ms(1200));
    assert_eq!(tween.sample(ms(1200)), 20.0);
    assert_eq!(tween.sample(ms(5000)), 20.0);
    assert_eq!(tween.sample(Duration::MAX), 20.0);
}

#[test]
fn the_easing_shapes_the_move() {
    let quad = Tween::new(0.0, 100.0, ms(1000)).with_easing(Easing::QuadIn);
    assert_eq!(quad.sample(ms(500)), 25.0);
    let out = Tween::new(0.0, 100.0, ms(1000)).with_easing(Easing::expo_out());
    assert!(out.sample(ms(400)) > 90.0, "fast in, slow out");
    assert_eq!(out.sample(ms(1000)), 100.0);
}

#[test]
fn a_zero_duration_jumps_to_the_target_at_the_delay() {
    let tween = Tween::new(1.0, 5.0, Duration::ZERO).with_delay(ms(100));
    assert_eq!(tween.sample(ms(99)), 1.0);
    assert_eq!(tween.sample(ms(100)), 5.0);
    assert_eq!(tween.sample(ms(500)), 5.0);
    assert!(!tween.is_finished(ms(99)));
    assert!(tween.is_finished(ms(100)));

    let immediate = Tween::new(1.0, 5.0, Duration::ZERO);
    assert_eq!(immediate.sample(Duration::ZERO), 5.0);
    assert!(immediate.is_finished(Duration::ZERO));
}

#[test]
fn negative_and_descending_ranges_interpolate() {
    let down = Tween::new(-10.0, -30.0, ms(1000));
    assert_eq!(down.sample(ms(500)), -20.0);
    assert_eq!(down.sample(ms(1000)), -30.0);

    let across = Tween::new(5.0, -5.0, ms(1000));
    assert_eq!(across.sample(ms(500)), 0.0);
    assert_eq!(across.sample(ms(250)), 2.5);
}

#[test]
fn finished_means_the_value_has_reached_the_target() {
    let tween = delayed();
    assert!(!tween.is_finished(Duration::ZERO));
    assert!(!tween.is_finished(ms(200)));
    assert!(!tween.is_finished(ms(1200) - Duration::from_nanos(1)));
    assert!(tween.is_finished(ms(1200)));
    assert!(tween.is_finished(ms(9000)));
    assert!(tween.is_finished(Duration::MAX));
}

#[test]
fn the_value_stays_between_the_endpoints_and_never_goes_backwards() {
    let easings = [
        Easing::Linear,
        Easing::QuadInOut,
        Easing::CubicOut,
        Easing::expo_in(),
        Easing::expo_out(),
        Easing::ExpoOut { exponent: 60.0 },
    ];
    for easing in easings {
        for (from, to) in [(0.0_f32, 1.0), (-4.0, 9.0), (3.0, -7.0), (0.1, 0.3)] {
            let tween = Tween::new(from, to, ms(777))
                .with_delay(ms(13))
                .with_easing(easing);
            let (low, high) = (from.min(to), from.max(to));
            let mut previous = from;
            for step in 0..=900 {
                let value = tween.sample(ms(step));
                assert!(
                    (low..=high).contains(&value),
                    "{easing:?} {from}->{to} at {step} ms: {value}"
                );
                if to >= from {
                    assert!(value >= previous, "{easing:?} {from}->{to} at {step}");
                } else {
                    assert!(value <= previous, "{easing:?} {from}->{to} at {step}");
                }
                previous = value;
            }
            assert_eq!(previous, to, "{easing:?} ends exactly on its target");
        }
    }
}

#[test]
fn the_last_instant_before_the_end_does_not_overshoot() {
    // Within a nanosecond of the end the progress rounds to 1 in `f32`; the
    // value must still be inside the range and not drift past `to`.
    let tween = Tween::new(0.1, 0.3, ms(1000)).with_easing(Easing::expo_out());
    let almost = tween.sample(ms(1000) - Duration::from_nanos(1));
    assert!((0.1..=0.3).contains(&almost), "{almost}");
    assert_eq!(tween.sample(ms(1000)), 0.3);
}

#[test]
fn extreme_endpoints_do_not_overflow() {
    let tween = Tween::new(-f32::MAX, f32::MAX, ms(1000));
    for step in [0, 1, 250, 500, 750, 999, 1000] {
        let value = tween.sample(ms(step));
        assert!(value.is_finite(), "{step} ms: {value}");
    }
    assert_eq!(tween.sample(ms(500)), 0.0);
}

#[test]
fn non_finite_endpoints_hold_from_then_jump_to() {
    let to_infinity = Tween::new(0.0, f32::INFINITY, ms(100));
    assert_eq!(to_infinity.sample(ms(50)), 0.0);
    assert_eq!(to_infinity.sample(ms(100)), f32::INFINITY);

    let across = Tween::new(f32::NEG_INFINITY, f32::INFINITY, ms(100));
    assert_eq!(across.sample(ms(50)), f32::NEG_INFINITY, "never NaN");

    let nan = Tween::new(f32::NAN, 1.0, ms(100));
    assert!(nan.sample(ms(50)).is_nan(), "from is returned as given");
    assert_eq!(nan.sample(ms(100)), 1.0);
}

#[test]
fn huge_durations_neither_overflow_nor_panic() {
    let tween = Tween::new(0.0, 1.0, Duration::MAX).with_delay(Duration::MAX);
    assert_eq!(tween.total(), Duration::MAX, "saturates");
    assert_eq!(tween.sample(ms(10)), 0.0);
    assert_eq!(tween.sample(Duration::MAX), 0.0, "delay just ended");
    assert!(!tween.is_finished(Duration::MAX - Duration::from_secs(1)));

    let long = Tween::new(0.0, 1.0, Duration::MAX);
    let value = long.sample(Duration::from_secs(60));
    assert!((0.0..1.0e-15).contains(&value), "{value}");
}
