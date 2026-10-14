use super::*;

#[test]
fn f32_shader_steps_keep_nominal_f64_outer_time() {
    let selection = GpuCflSelection {
        dt_s: f32::from_bits((OUTER_DT_S as f32).to_bits() - 1),
        max_cfl_dt_s: None,
        max_diffusion_dt_s: None,
    };
    let mut accepted_time = 0.0_f64;
    let mut encoded_time = 0.0_f64;
    for tick in 1..=1_000 {
        let (nominal, encoded) =
            select_clock_durations(OUTER_DT_S, selection, accepted_time - encoded_time).unwrap();
        accepted_time += nominal;
        encoded_time += f64::from(encoded);
        assert!((accepted_time - f64::from(tick) * OUTER_DT_S).abs() <= 1e-10);
        assert!((accepted_time - encoded_time).abs() < 1e-9);
    }
    let capped = GpuCflSelection {
        dt_s: 0.001,
        max_cfl_dt_s: Some(0.0011),
        max_diffusion_dt_s: None,
    };
    let (nominal, encoded) = select_clock_durations(OUTER_DT_S, capped, 0.0).unwrap();
    assert_eq!(nominal, f64::from(encoded));
    assert!(f64::from(encoded) <= 0.0011);

    let exact_cap = GpuCflSelection {
        dt_s: f32::from_bits((OUTER_DT_S as f32).to_bits() - 1),
        max_cfl_dt_s: Some(OUTER_DT_S),
        max_diffusion_dt_s: None,
    };
    let (nominal, encoded) = select_clock_durations(OUTER_DT_S, exact_cap, 0.0).unwrap();
    assert_eq!(nominal, f64::from(encoded));
    assert!(nominal > OUTER_DT_S * 0.49);
    assert!(nominal < OUTER_DT_S * 0.51);
    let remaining = OUTER_DT_S - nominal;
    let rounded_final = remaining as f32;
    let final_dt = if f64::from(rounded_final) > remaining {
        f32::from_bits(rounded_final.to_bits() - 1)
    } else {
        rounded_final
    };
    let final_selection = GpuCflSelection {
        dt_s: final_dt,
        ..exact_cap
    };
    let (final_nominal, final_encoded) =
        select_clock_durations(remaining, final_selection, 0.0).unwrap();
    assert_eq!(final_nominal, remaining);
    assert!(f64::from(final_encoded) <= OUTER_DT_S);
    assert_eq!(nominal + final_nominal, OUTER_DT_S);
}

#[test]
fn near_outer_cfl_cap_balances_two_shader_steps() {
    let cap = 0.016_563_048_586_25_f64;
    let rounded = cap as f32;
    let selected_dt = if f64::from(rounded) > cap {
        f32::from_bits(rounded.to_bits() - 1)
    } else {
        rounded
    };
    let first_selection = GpuCflSelection {
        dt_s: selected_dt,
        max_cfl_dt_s: Some(cap),
        max_diffusion_dt_s: None,
    };
    let (first_nominal, first_encoded) =
        select_clock_durations(OUTER_DT_S, first_selection, 0.0).unwrap();
    assert_eq!(first_nominal, f64::from(first_encoded));
    assert!(first_nominal > OUTER_DT_S * 0.49);
    assert!(first_nominal < OUTER_DT_S * 0.51);
    assert!(first_nominal <= cap);

    let remaining = OUTER_DT_S - first_nominal;
    let rounded_final = remaining as f32;
    let final_dt = if f64::from(rounded_final) > remaining {
        f32::from_bits(rounded_final.to_bits() - 1)
    } else {
        rounded_final
    };
    let final_selection = GpuCflSelection {
        dt_s: final_dt,
        max_cfl_dt_s: Some(cap),
        max_diffusion_dt_s: None,
    };
    let (final_nominal, final_encoded) =
        select_clock_durations(remaining, final_selection, 0.0).unwrap();
    assert_eq!(final_nominal, remaining);
    assert!(f64::from(final_encoded) <= cap);
    assert_eq!(first_nominal + final_nominal, OUTER_DT_S);
    assert!((OUTER_DT_S - f64::from(first_encoded) - f64::from(final_encoded)).abs() < 1e-9);
}

#[test]
fn numerical_refinement_halves_duration_and_retains_clock_balance() {
    let initial = GpuCflSelection {
        dt_s: OUTER_DT_S as f32,
        max_cfl_dt_s: None,
        max_diffusion_dt_s: Some(0.1),
    };
    let mut accepted_time = 0.0_f64;
    let mut encoded_time = 0.0_f64;
    for tick in 1..=1_000 {
        let mut selection = initial;
        let mut nominal = OUTER_DT_S;
        let mut encoded = initial.dt_s;
        for _ in 0..MAX_CANDIDATE_REFINEMENTS {
            selection = refined_selection(selection, nominal).unwrap();
            let (next_nominal, next_encoded) =
                select_clock_durations(OUTER_DT_S, selection, accepted_time - encoded_time)
                    .unwrap();
            assert!(next_nominal <= nominal * 0.5);
            assert!(next_encoded <= encoded * 0.5);
            assert_eq!(selection.max_diffusion_dt_s, Some(0.1));
            nominal = next_nominal;
            encoded = next_encoded;
        }
        accepted_time += nominal;
        encoded_time += f64::from(encoded);
        let remaining = OUTER_DT_S - nominal;
        let final_selection = GpuCflSelection {
            dt_s: remaining as f32,
            ..initial
        };
        let (final_nominal, final_encoded) =
            select_clock_durations(remaining, final_selection, accepted_time - encoded_time)
                .unwrap();
        accepted_time += final_nominal;
        encoded_time += f64::from(final_encoded);
        assert!((accepted_time - f64::from(tick) * OUTER_DT_S).abs() <= 1e-10);
        assert!((accepted_time - encoded_time).abs() < 1e-9);
    }
    for invalid_duration in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MIN_POSITIVE] {
        assert!(refined_selection(initial, invalid_duration).is_err());
    }
}

#[test]
fn stage_refinement_excludes_invalid_fields_and_arithmetic() {
    for (slot, status) in [
        (0, transport::STATUS_DONOR_OVERDRAW),
        (0, transport::STATUS_VOLUME_CLOSURE),
        (0, transport::STATUS_CORRECTION_LIMIT),
        (1, momentum::STATUS_DONOR_OVERDRAW),
        (2, density::STATUS_VOLUME_CLOSURE),
    ] {
        assert!(matches!(
            stage_rejection(slot, "fixture", status),
            CandidateRejection::Numerical { .. }
        ));
    }
    for (slot, status) in [
        (0, transport::STATUS_INVALID_FACE),
        (0, transport::STATUS_INVALID_CELL),
        (
            0,
            transport::STATUS_VOLUME_CLOSURE | transport::STATUS_INVALID_CELL,
        ),
        (0, 1 << 31),
        (1, momentum::STATUS_INVALID_LEDGER),
        (1, momentum::STATUS_EMPTY_OPEN_FACE),
        (2, density::STATUS_INVALID_CELL),
        (3, crate::viscosity::STATUS_NONFINITE_RESULT),
        (4, crate::gravity::STATUS_NONFINITE_RESULT),
    ] {
        assert!(matches!(
            stage_rejection(slot, "fixture", status),
            CandidateRejection::Terminal(_)
        ));
    }
}
