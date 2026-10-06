//! Closed finite pairs, explicit heat ledgers, phase transitions and atomic rejection.
#[path = "water_conduction/support.rs"]
mod support;

use particle_sim_cpu::water::{
    AIR_HEAT_CAPACITY_J_KG_K, ConductionError, ConductionExchange, ConductionStep,
    MixtureInventory, MixtureVessel, WaterPhase, conduct_heat,
};
use support::*;

fn step(conductance_w_k: f64, duration_s: f64) -> ConductionStep {
    ConductionStep {
        conductance_w_k,
        duration_s,
    }
}

#[test]
fn dry_air_transfer_matches_two_finite_heat_capacities() {
    for reverse in [false, true] {
        let mut first = air(if reverse { 350.0 } else { 450.0 }, 1.0);
        let mut second = air(if reverse { 450.0 } else { 350.0 }, 2.0);
        let before = [first.inventory(), second.inventory()];
        let temperature = [
            first.equilibrium().temperature_k,
            second.equilibrium().temperature_k,
        ];
        let exchange = conduct_heat(table(), &mut first, &mut second, step(20.0, 0.5)).unwrap();
        let heat = 10.0 * (temperature[1] - temperature[0]);
        relative(exchange.requested_heat_into_first_j, heat, 2e-15);
        relative(
            first.equilibrium().temperature_k,
            temperature[0] + heat / AIR_HEAT_CAPACITY_J_KG_K,
            2e-15,
        );
        relative(
            second.equilibrium().temperature_k,
            temperature[1] - heat / (2.0 * AIR_HEAT_CAPACITY_J_KG_K),
            2e-15,
        );
        ledger(before, [&first, &second], exchange);
    }
}

#[test]
fn dry_air_relaxation_refines_toward_the_analytic_solution() {
    let duration = 2.0;
    let conductance = 50.0;
    let equilibrium = (450.0 + 2.0 * 350.0) / 3.0;
    let decay = (-conductance * (1.0 + 0.5) * duration / AIR_HEAT_CAPACITY_J_KG_K).exp();
    let expected = equilibrium + (450.0 - equilibrium) * decay;
    let mut errors = Vec::new();
    for count in [20, 40, 80] {
        let mut first = air(450.0, 1.0);
        let mut second = air(350.0, 2.0);
        let initial_total =
            first.inventory().internal_energy_j + second.inventory().internal_energy_j;
        for _ in 0..count {
            let before = [first.inventory(), second.inventory()];
            let exchange = conduct_heat(
                table(),
                &mut first,
                &mut second,
                step(conductance, duration / count as f64),
            )
            .unwrap();
            ledger(before, [&first, &second], exchange);
        }
        relative(
            first.inventory().internal_energy_j + second.inventory().internal_energy_j,
            initial_total,
            2e-14,
        );
        errors.push((first.equilibrium().temperature_k - expected).abs());
    }
    assert!(errors[1] < 0.51 * errors[0]);
    assert!(errors[2] < 0.51 * errors[1]);
}

#[test]
fn exact_controls_preserve_both_complete_owners() {
    for control in [step(0.0, 1.0), step(1.0, 0.0)] {
        let mut first = mixed(400.0, 0.5, 100.0);
        let mut second = air(500.0, 1.0);
        let before = [state(&first), state(&second)];
        let exchange = conduct_heat(table(), &mut first, &mut second, control).unwrap();
        assert_eq!(exchange, ConductionExchange::default());
        assert_eq!([state(&first), state(&second)], before);
    }
    let mut first = air(400.0, 1.0);
    let mut second = air(400.0, 2.0);
    let before = [state(&first), state(&second)];
    assert_eq!(
        conduct_heat(table(), &mut first, &mut second, step(1e200, 1e200)).unwrap(),
        ConductionExchange::default()
    );
    assert_eq!([state(&first), state(&second)], before);
}

#[test]
fn invalid_steps_preserve_both_complete_owners() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        for control in [step(invalid, 1.0), step(1.0, invalid)] {
            let mut first = air(400.0, 1.0);
            let mut second = air(500.0, 1.0);
            let before = [state(&first), state(&second)];
            assert_eq!(
                conduct_heat(table(), &mut first, &mut second, control),
                Err(ConductionError::InvalidInput)
            );
            assert_eq!([state(&first), state(&second)], before);
        }
    }
}

#[test]
fn excessive_heat_cannot_reverse_the_temperature_order() {
    let mut first = air(400.0, 1.0);
    let mut second = air(500.0, 1.0);
    let before = [state(&first), state(&second)];
    assert_eq!(
        conduct_heat(
            table(),
            &mut first,
            &mut second,
            step(AIR_HEAT_CAPACITY_J_KG_K, 0.6)
        ),
        Err(ConductionError::TemperatureCrossing)
    );
    assert_eq!([state(&first), state(&second)], before);
}

#[test]
fn one_step_can_reach_equal_temperature_without_crossing() {
    let mut first = air(400.0, 1.0);
    let mut second = air(500.0, 1.0);
    let before = [first.inventory(), second.inventory()];
    let exchange = conduct_heat(
        table(),
        &mut first,
        &mut second,
        step(AIR_HEAT_CAPACITY_J_KG_K / 2.0, 1.0),
    )
    .unwrap();
    relative(first.equilibrium().temperature_k, 450.0, 2e-15);
    relative(second.equilibrium().temperature_k, 450.0, 2e-15);
    ledger(before, [&first, &second], exchange);
}

#[test]
fn rejected_first_or_second_closure_preserves_the_other_owner() {
    for reverse in [false, true] {
        let mut first = air(
            if reverse { 500.0 } else { 300.0 },
            if reverse { 0.01 } else { 1.0 },
        );
        let mut second = air(
            if reverse { 300.0 } else { 500.0 },
            if reverse { 1.0 } else { 0.01 },
        );
        let before = [state(&first), state(&second)];
        let error = conduct_heat(table(), &mut first, &mut second, step(20.0, 1.0)).unwrap_err();
        assert!(matches!(
            (reverse, error),
            (false, ConductionError::SecondClosure(_)) | (true, ConductionError::FirstClosure(_))
        ));
        assert_eq!([state(&first), state(&second)], before);
    }
}

#[test]
fn unrepresentable_transfer_cannot_heat_only_one_vessel() {
    let mut first = air(400.0, 1e20);
    let mut second = air(500.0, 1.0);
    let before = [state(&first), state(&second)];
    assert_eq!(
        conduct_heat(table(), &mut first, &mut second, step(1.0, 1.0)),
        Err(ConductionError::UnrepresentableTransfer)
    );
    assert_eq!([state(&first), state(&second)], before);
}

#[test]
fn active_overflow_and_underflow_reject_without_edits() {
    for control in [
        step(f64::MAX, 1.0),
        step(f64::from_bits(1), f64::from_bits(1)),
    ] {
        let mut first = air(400.0, 1.0);
        let mut second = air(500.0, 1.0);
        let before = [state(&first), state(&second)];
        assert!(conduct_heat(table(), &mut first, &mut second, control).is_err());
        assert_eq!([state(&first), state(&second)], before);
    }
}

#[test]
fn scaled_pairs_preserve_the_same_temperature_and_phase_result() {
    let mut results = Vec::new();
    for scale in [1e-6, 1.0, 1e6] {
        let input = mixed(400.0, 0.5, 100.0).inventory();
        let mut first = MixtureVessel::new(
            table(),
            MixtureInventory {
                water_mass_kg: scale * input.water_mass_kg,
                air_mass_kg: scale * input.air_mass_kg,
                internal_energy_j: scale * input.internal_energy_j,
                available_volume_m3: scale * input.available_volume_m3,
            },
        )
        .unwrap();
        let mut second = air(500.0, scale);
        let before = [first.inventory(), second.inventory()];
        let exchange =
            conduct_heat(table(), &mut first, &mut second, step(scale * 100.0, 0.1)).unwrap();
        ledger(before, [&first, &second], exchange);
        results.push((
            first.equilibrium().temperature_k,
            second.equilibrium().temperature_k,
            first.equilibrium().vapor_water_mass_kg / scale,
        ));
    }
    for result in &results[1..] {
        relative(result.0, results[0].0, 2e-10);
        relative(result.1, results[0].1, 2e-14);
        relative(result.2, results[0].2, 2e-10);
    }
}

#[test]
fn conduction_can_evaporate_and_condense_with_one_water_inventory() {
    let mut water = mixed(400.0, 1.0, 50_000.0);
    water.apply_heat(table(), -10_000.0).unwrap();
    let initial = state(&water);
    assert!(water.equilibrium().liquid_water_mass_kg > 0.0);
    let mut hot = air(500.0, 100.0);
    let conductance =
        20_000.0 / (hot.equilibrium().temperature_k - water.equilibrium().temperature_k);
    let before = [water.inventory(), hot.inventory()];
    let exchange = conduct_heat(table(), &mut water, &mut hot, step(conductance, 1.0)).unwrap();
    ledger(before, [&water, &hot], exchange);
    assert_eq!(water.equilibrium().liquid_water_mass_kg, 0.0);

    let mut cold = air(350.0, 100.0);
    let conductance =
        20_000.0 / (water.equilibrium().temperature_k - cold.equilibrium().temperature_k);
    let before = [water.inventory(), cold.inventory()];
    let exchange = conduct_heat(table(), &mut water, &mut cold, step(conductance, 1.0)).unwrap();
    ledger(before, [&water, &cold], exchange);
    assert!(water.equilibrium().liquid_water_mass_kg > 0.0);
    relative(
        water.inventory().internal_energy_j,
        initial.0.internal_energy_j,
        2e-15,
    );
    relative(
        water.equilibrium().temperature_k,
        initial.1.temperature_k,
        2e-10,
    );
}

#[test]
fn rounded_nonzero_transfers_cannot_hide_a_large_heat_error() {
    let mut first = air(400.0, 1e9);
    let mut second = air(500.0, 1e9);
    let before = [state(&first), state(&second)];
    assert_eq!(
        conduct_heat(table(), &mut first, &mut second, step(1.234567, 1.0)),
        Err(ConductionError::UnrepresentableTransfer)
    );
    assert_eq!([state(&first), state(&second)], before);
}

#[test]
fn pure_compressed_water_keeps_its_wider_pressure_domain() {
    let properties = table()
        .sample_single_phase(WaterPhase::Liquid, 400.0, 5e6)
        .unwrap();
    let mut first = MixtureVessel::new(
        table(),
        MixtureInventory {
            water_mass_kg: 1.0,
            air_mass_kg: 0.0,
            internal_energy_j: properties.energy_j_kg,
            available_volume_m3: properties.volume_m3_kg,
        },
    )
    .unwrap();
    let mut second = air(450.0, 1.0);
    let before = [first.inventory(), second.inventory()];
    let exchange = conduct_heat(table(), &mut first, &mut second, step(0.1, 1.0)).unwrap();
    ledger(before, [&first, &second], exchange);
    assert!(first.equilibrium().pressure_pa > 1e6);
    assert_eq!(first.inventory().air_mass_kg, 0.0);
}
