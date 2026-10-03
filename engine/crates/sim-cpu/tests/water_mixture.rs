//! Conserved air/water inventories, partial pressure and reversible phase changes.
use particle_sim_cpu::water::{
    AIR_GAS_CONSTANT_J_KG_K, AIR_HEAT_CAPACITY_J_KG_K, AIR_REFERENCE_TEMPERATURE_K,
    MAX_MIXTURE_PRESSURE_PA, MIN_PRESSURE_PA, MixtureEquilibrium, MixtureInventory, MixtureVessel,
    WaterError, WaterInventory, WaterPhase, WaterTable, close_mixture, close_water,
};
use std::sync::OnceLock;

fn table() -> &'static WaterTable {
    static TABLE: OnceLock<WaterTable> = OnceLock::new();
    TABLE.get_or_init(WaterTable::new)
}

fn two_phase(temperature_k: f64, vapor_fraction: f64, air_pressure_pa: f64) -> MixtureInventory {
    let saturation = table().saturation().sample(temperature_k).unwrap();
    let liquid = table()
        .sample_single_phase(
            WaterPhase::Liquid,
            temperature_k,
            saturation.pressure_pa + air_pressure_pa,
        )
        .unwrap();
    let gas_volume_m3 = vapor_fraction * saturation.vapor_volume_m3_kg;
    let air_mass_kg = air_pressure_pa * gas_volume_m3 / (AIR_GAS_CONSTANT_J_KG_K * temperature_k);
    MixtureInventory {
        water_mass_kg: 1.0,
        air_mass_kg,
        internal_energy_j: (1.0 - vapor_fraction) * liquid.energy_j_kg
            + vapor_fraction * saturation.vapor_energy_j_kg
            + air_mass_kg
                * AIR_HEAT_CAPACITY_J_KG_K
                * (temperature_k - AIR_REFERENCE_TEMPERATURE_K),
        available_volume_m3: (1.0 - vapor_fraction) * liquid.volume_m3_kg + gas_volume_m3,
    }
}

fn all_vapor(temperature_k: f64, vapor_pressure_pa: f64, air_pressure_pa: f64) -> MixtureInventory {
    let vapor = table()
        .sample_single_phase(WaterPhase::Vapor, temperature_k, vapor_pressure_pa)
        .unwrap();
    let air_mass_kg =
        air_pressure_pa * vapor.volume_m3_kg / (AIR_GAS_CONSTANT_J_KG_K * temperature_k);
    MixtureInventory {
        water_mass_kg: 1.0,
        air_mass_kg,
        internal_energy_j: vapor.energy_j_kg
            + air_mass_kg
                * AIR_HEAT_CAPACITY_J_KG_K
                * (temperature_k - AIR_REFERENCE_TEMPERATURE_K),
        available_volume_m3: vapor.volume_m3_kg,
    }
}

fn scaled(input: MixtureInventory, scale: f64) -> MixtureInventory {
    MixtureInventory {
        water_mass_kg: input.water_mass_kg * scale,
        air_mass_kg: input.air_mass_kg * scale,
        internal_energy_j: input.internal_energy_j * scale,
        available_volume_m3: input.available_volume_m3 * scale,
    }
}

fn relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs(),
        "{actual} != {expected}"
    );
}

fn conserved(input: MixtureInventory, state: MixtureEquilibrium) {
    relative(
        state.liquid_water_mass_kg + state.vapor_water_mass_kg,
        input.water_mass_kg,
        2e-15,
    );
    relative(
        state.liquid_volume_m3 + state.gas_volume_m3,
        input.available_volume_m3,
        2e-12,
    );
    let energy_scale = input
        .internal_energy_j
        .abs()
        .max(1000.0 * (input.water_mass_kg + input.air_mass_kg));
    assert!(state.energy_residual_j.abs() <= 1.1e-11 * energy_scale);
    assert!(state.volume_residual_m3.abs() <= 2e-12 * input.available_volume_m3);
    assert!(
        (state.water_energy_j + state.air_energy_j - input.internal_energy_j).abs()
            <= 1.1e-11 * energy_scale
    );
    relative(
        state.pressure_pa,
        state.vapor_partial_pressure_pa + state.air_partial_pressure_pa,
        2e-15,
    );
    if input.air_mass_kg > 0.0 {
        relative(
            state.air_partial_pressure_pa,
            input.air_mass_kg * AIR_GAS_CONSTANT_J_KG_K * state.temperature_k / state.gas_volume_m3,
            2e-12,
        );
    }
}

#[test]
fn saturated_water_uses_vapor_partial_pressure_and_liquid_uses_total_pressure() {
    let input = two_phase(400.0, 0.1, 50_000.0);
    let state = close_mixture(table(), input).unwrap();
    let saturation = table().saturation().sample(400.0).unwrap();
    let liquid = table()
        .sample_single_phase(WaterPhase::Liquid, 400.0, saturation.pressure_pa + 50_000.0)
        .unwrap();
    relative(state.temperature_k, 400.0, 2e-9);
    relative(
        state.vapor_partial_pressure_pa,
        saturation.pressure_pa,
        2e-8,
    );
    relative(state.air_partial_pressure_pa, 50_000.0, 2e-8);
    relative(state.pressure_pa, saturation.pressure_pa + 50_000.0, 2e-8);
    relative(state.liquid_water_mass_kg, 0.9, 2e-8);
    relative(state.vapor_water_mass_kg, 0.1, 2e-8);
    relative(state.liquid_volume_m3, 0.9 * liquid.volume_m3_kg, 2e-8);
    relative(
        state.water_energy_j,
        0.9 * liquid.energy_j_kg + 0.1 * saturation.vapor_energy_j_kg,
        2e-8,
    );
    assert!(state.pressure_pa > state.vapor_partial_pressure_pa + 49_999.0);
    conserved(input, state);
}

#[test]
fn liquid_displaces_the_volume_shared_by_steam_and_air() {
    let input = two_phase(400.0, 1e-4, 200_000.0);
    let state = close_mixture(table(), input).unwrap();
    let saturation = table().saturation().sample(400.0).unwrap();
    relative(state.temperature_k, 400.0, 2e-9);
    relative(
        state.gas_volume_m3,
        1e-4 * saturation.vapor_volume_m3_kg,
        2e-8,
    );
    relative(state.air_partial_pressure_pa, 200_000.0, 2e-8);
    assert!(state.liquid_volume_m3 > 10.0 * state.gas_volume_m3);
    let pressure_if_liquid_volume_were_used =
        input.air_mass_kg * AIR_GAS_CONSTANT_J_KG_K * state.temperature_k
            / input.available_volume_m3;
    assert!(state.air_partial_pressure_pa > 10.0 * pressure_if_liquid_volume_were_used);
    conserved(input, state);
}

#[test]
fn superheated_steam_and_air_close_below_the_total_pressure_bound() {
    let input = all_vapor(600.0, 400_000.0, 200_000.0);
    let state = close_mixture(table(), input).unwrap();
    relative(state.temperature_k, 600.0, 2e-9);
    relative(state.vapor_partial_pressure_pa, 400_000.0, 2e-8);
    relative(state.air_partial_pressure_pa, 200_000.0, 2e-8);
    relative(state.pressure_pa, 600_000.0, 2e-8);
    assert_eq!(state.liquid_water_mass_kg, 0.0);
    assert_eq!(state.liquid_volume_m3, 0.0);
    assert_eq!(state.vapor_water_mass_kg, input.water_mass_kg);
    assert!(
        state.vapor_partial_pressure_pa < table().saturation().sample(600.0).unwrap().pressure_pa
    );
    assert!(state.pressure_pa < MAX_MIXTURE_PRESSURE_PA);
    conserved(input, state);
}

#[test]
fn repeated_closure_preserves_intensive_state_at_all_inventory_scales() {
    for input in [
        two_phase(330.0, 0.02, 30_000.0),
        two_phase(400.0, 0.2, 200_000.0),
        two_phase(450.0, 0.7, 20_000.0),
        all_vapor(600.0, 400_000.0, 200_000.0),
    ] {
        let reference = close_mixture(table(), input).unwrap();
        for scale in [1e-6, 1.0, 1e6] {
            let scaled_input = scaled(input, scale);
            let state = close_mixture(table(), scaled_input).unwrap();
            relative(state.temperature_k, reference.temperature_k, 2e-9);
            relative(state.pressure_pa, reference.pressure_pa, 2e-8);
            assert!(
                (state.liquid_water_mass_kg - reference.liquid_water_mass_kg * scale).abs()
                    <= 2e-8 * scaled_input.water_mass_kg
            );
            assert!(
                (state.vapor_water_mass_kg - reference.vapor_water_mass_kg * scale).abs()
                    <= 2e-8 * scaled_input.water_mass_kg
            );
            conserved(scaled_input, state);
            for _ in 0..20 {
                assert_eq!(close_mixture(table(), scaled_input).unwrap(), state);
            }
        }
    }
}

#[test]
fn heating_and_cooling_cross_zero_liquid_without_changing_either_mass_or_geometry() {
    let input = two_phase(400.0, 1.0, 50_000.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    assert!(before.liquid_water_mass_kg <= 2e-8);
    let mut ledger = vessel.apply_heat(table(), 10_000.0).unwrap();
    let hot = vessel.equilibrium();
    assert!(hot.temperature_k > 400.0);
    assert_eq!(hot.liquid_water_mass_kg, 0.0);
    ledger += vessel.apply_heat(table(), -20_000.0).unwrap();
    let cold = vessel.equilibrium();
    assert!(cold.temperature_k < 400.0);
    assert!(cold.liquid_water_mass_kg > 0.0);
    assert!(cold.vapor_water_mass_kg > 0.0);
    assert_eq!(vessel.inventory().water_mass_kg, input.water_mass_kg);
    assert_eq!(vessel.inventory().air_mass_kg, input.air_mass_kg);
    assert_eq!(
        vessel.inventory().available_volume_m3,
        input.available_volume_m3
    );
    assert_eq!(
        vessel.inventory().internal_energy_j - input.internal_energy_j,
        ledger
    );
    conserved(vessel.inventory(), cold);
    ledger += vessel.apply_heat(table(), 10_000.0).unwrap();
    assert_eq!(ledger, 0.0);
    assert_eq!(vessel.inventory(), input);
    assert_eq!(vessel.equilibrium(), before);
}

#[test]
fn rejected_heat_preserves_both_conserved_inventory_and_equilibrium() {
    for input in [
        two_phase(400.0, 0.1, 50_000.0),
        all_vapor(600.0, 400_000.0, 200_000.0),
    ] {
        let mut vessel = MixtureVessel::new(table(), input).unwrap();
        let state = vessel.equilibrium();
        for heat in [
            1e9,
            -1e9,
            f64::MAX,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert!(vessel.apply_heat(table(), heat).is_err());
            assert_eq!(vessel.inventory(), input);
            assert_eq!(vessel.equilibrium(), state);
        }
    }
}

#[test]
fn heat_ledger_reports_the_actual_representable_energy_change() {
    let input = two_phase(400.0, 0.1, 50_000.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    assert_eq!(vessel.apply_heat(table(), 1e-20).unwrap(), 0.0);
    assert_eq!(vessel.inventory(), input);
    let heat = vessel.apply_heat(table(), 0.1).unwrap();
    assert_eq!(
        heat,
        vessel.inventory().internal_energy_j - input.internal_energy_j
    );
}

#[test]
fn invalid_empty_negative_and_nonfinite_inventories_reject() {
    let input = two_phase(400.0, 0.1, 50_000.0);
    for invalid in [
        MixtureInventory {
            water_mass_kg: -1.0,
            ..input
        },
        MixtureInventory {
            air_mass_kg: -1.0,
            ..input
        },
        MixtureInventory {
            water_mass_kg: 0.0,
            air_mass_kg: 0.0,
            ..input
        },
        MixtureInventory {
            available_volume_m3: 0.0,
            ..input
        },
        MixtureInventory {
            available_volume_m3: -1.0,
            ..input
        },
    ] {
        assert_eq!(
            close_mixture(table(), invalid).unwrap_err(),
            WaterError::NonPositiveInventory
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for invalid in [
            MixtureInventory {
                water_mass_kg: value,
                ..input
            },
            MixtureInventory {
                air_mass_kg: value,
                ..input
            },
            MixtureInventory {
                internal_energy_j: value,
                ..input
            },
            MixtureInventory {
                available_volume_m3: value,
                ..input
            },
        ] {
            assert_eq!(
                close_mixture(table(), invalid).unwrap_err(),
                WaterError::NonFiniteInput
            );
        }
    }
    assert!(
        close_mixture(
            table(),
            MixtureInventory {
                internal_energy_j: -1.0,
                ..input
            }
        )
        .is_err()
    );
}

#[test]
fn dilute_steam_rejects_below_the_vapor_partial_pressure_floor() {
    let input = all_vapor(600.0, MIN_PRESSURE_PA * 1.01, 50_000.0);
    let dilute = MixtureInventory {
        available_volume_m3: input.available_volume_m3 * 10.0,
        ..input
    };
    assert!(close_mixture(table(), input).is_ok());
    assert_eq!(
        close_mixture(table(), dilute).unwrap_err(),
        WaterError::UnsupportedEquilibrium
    );
}

#[test]
fn mixed_water_and_air_rejects_above_the_total_pressure_bound() {
    let input = two_phase(400.0, 0.1, MAX_MIXTURE_PRESSURE_PA);
    assert!(close_mixture(table(), input).is_err());
}

#[test]
fn mixed_phase_and_vapor_states_close_at_the_property_domain_boundaries() {
    let minimum_temperature_k = table().saturation().min_temperature_k();
    let saturation = table().saturation().sample(400.0).unwrap();
    for (temperature_k, input) in [
        (
            minimum_temperature_k,
            two_phase(minimum_temperature_k, 0.1, 100.0),
        ),
        (
            400.0,
            two_phase(400.0, 0.1, MAX_MIXTURE_PRESSURE_PA - saturation.pressure_pa),
        ),
        (600.0, all_vapor(600.0, MIN_PRESSURE_PA, 50_000.0)),
        (623.15, all_vapor(623.15, 400_000.0, 200_000.0)),
    ] {
        let state = close_mixture(table(), input).unwrap();
        relative(state.temperature_k, temperature_k, 2e-9);
        assert!(state.pressure_pa <= MAX_MIXTURE_PRESSURE_PA);
        assert!(state.vapor_partial_pressure_pa >= MIN_PRESSURE_PA);
        conserved(input, state);
    }
}

#[test]
fn vapor_floor_and_total_pressure_endpoints_close_across_inventory_scales() {
    let minimum_temperature_k = table().saturation().min_temperature_k();
    for (temperature_k, air_partial_pressure_pa, scales) in [
        (minimum_temperature_k, 990_000.0, &[1e-6, 1.0, 3.0, 1e6][..]),
        (623.15, 1e-8, &[3.0, 1e6][..]),
    ] {
        let input = all_vapor(temperature_k, MIN_PRESSURE_PA, air_partial_pressure_pa);
        for &scale in scales {
            let scaled_input = scaled(input, scale);
            let state = close_mixture(table(), scaled_input).unwrap_or_else(|error| {
                panic!(
                    "{temperature_k} K, {air_partial_pressure_pa} Pa air, scale {scale}: {error}"
                )
            });
            relative(state.temperature_k, temperature_k, 2e-9);
            relative(
                state.pressure_pa,
                MIN_PRESSURE_PA + air_partial_pressure_pa,
                8.0 * f64::EPSILON,
            );
            assert!(state.pressure_pa <= MAX_MIXTURE_PRESSURE_PA * (1.0 + 8.0 * f64::EPSILON));
            assert_eq!(state.liquid_water_mass_kg, 0.0);
            assert_eq!(state.liquid_volume_m3, 0.0);
            conserved(scaled_input, state);
            let vessel = MixtureVessel::new(table(), scaled_input).unwrap();
            assert_eq!(vessel.inventory(), scaled_input);
            assert_eq!(vessel.equilibrium(), state);
        }
    }
}

#[test]
fn small_vapor_fraction_closes_at_the_total_pressure_guard() {
    for temperature_k in [330.0, 400.0, 450.0] {
        let saturation = table().saturation().sample(temperature_k).unwrap();
        let input = two_phase(
            temperature_k,
            1e-4,
            MAX_MIXTURE_PRESSURE_PA - saturation.pressure_pa,
        );
        let state = close_mixture(table(), input).unwrap();
        relative(state.temperature_k, temperature_k, 2e-9);
        relative(state.pressure_pa, MAX_MIXTURE_PRESSURE_PA, 2e-8);
        relative(state.vapor_water_mass_kg, 1e-4, 2e-8);
        assert!(state.pressure_pa <= MAX_MIXTURE_PRESSURE_PA);
        conserved(input, state);
    }
}

#[test]
fn tiny_positive_vapor_fractions_preserve_their_shared_gas_volume() {
    for vapor_fraction in [1e-8, 1e-7, 1e-6, 1e-5, 1e-4] {
        let input = two_phase(330.0, vapor_fraction, 100.0);
        let state = close_mixture(table(), input).unwrap();
        relative(state.temperature_k, 330.0, 2e-9);
        relative(state.air_partial_pressure_pa, 100.0, 2e-8);
        relative(state.vapor_water_mass_kg, vapor_fraction, 2e-8);
        conserved(input, state);
    }
}

#[test]
fn representable_tiny_gas_and_air_inventories_close_without_iteration_failure() {
    for temperature_k in [330.0, 400.0, 450.0] {
        for vapor_fraction in [1e-12, 1e-8, 0.1] {
            for air_partial_pressure_pa in [1e-8, 100.0] {
                let input = two_phase(temperature_k, vapor_fraction, air_partial_pressure_pa);
                let state = close_mixture(table(), input).unwrap();
                relative(state.temperature_k, temperature_k, 2e-9);
                assert!(state.vapor_water_mass_kg > 0.0);
                assert!(state.gas_volume_m3 > 0.0);
                assert!(state.air_partial_pressure_pa > 0.0);
                // Conserved U/V tolerances limit relative accuracy of tiny phases.
                conserved(input, state);
                let vessel = MixtureVessel::new(table(), input).unwrap();
                assert_eq!(vessel.inventory(), input);
                assert_eq!(vessel.equilibrium(), state);
            }
        }
    }
}

#[test]
fn dry_air_matches_its_analytic_energy_and_pressure_limits() {
    for temperature_k in [AIR_REFERENCE_TEMPERATURE_K, 400.0, 600.0, 623.15] {
        let input = MixtureInventory {
            water_mass_kg: 0.0,
            air_mass_kg: 2.0,
            internal_energy_j: 2.0
                * AIR_HEAT_CAPACITY_J_KG_K
                * (temperature_k - AIR_REFERENCE_TEMPERATURE_K),
            available_volume_m3: 2.0 * AIR_GAS_CONSTANT_J_KG_K * temperature_k / 100_000.0,
        };
        let state = close_mixture(table(), input).unwrap();
        relative(state.temperature_k, temperature_k, 2e-15);
        relative(state.pressure_pa, 100_000.0, 2e-15);
        assert_eq!(state.air_partial_pressure_pa, state.pressure_pa);
        assert_eq!(state.vapor_partial_pressure_pa, 0.0);
        assert_eq!(state.liquid_water_mass_kg, 0.0);
        assert_eq!(state.vapor_water_mass_kg, 0.0);
        assert_eq!(state.liquid_volume_m3, 0.0);
        assert_eq!(state.gas_volume_m3, input.available_volume_m3);
        assert_eq!(state.water_energy_j, 0.0);
        assert_eq!(state.air_energy_j, input.internal_energy_j);
        conserved(input, state);
    }
}

#[test]
fn zero_air_delegates_to_the_wider_pure_water_domain_and_phase_observations() {
    for (phase, temperature_k, pressure_pa) in [
        (WaterPhase::Liquid, 300.0, 3e6),
        (WaterPhase::Vapor, 600.0, 8e6),
    ] {
        let properties = table()
            .sample_single_phase(phase, temperature_k, pressure_pa)
            .unwrap();
        let water = WaterInventory {
            mass_kg: 1.0,
            internal_energy_j: properties.energy_j_kg,
            available_volume_m3: properties.volume_m3_kg,
        };
        let input = MixtureInventory {
            water_mass_kg: water.mass_kg,
            air_mass_kg: 0.0,
            internal_energy_j: water.internal_energy_j,
            available_volume_m3: water.available_volume_m3,
        };
        let pure = close_water(table(), water).unwrap();
        let state = close_mixture(table(), input).unwrap();
        assert_eq!(state.temperature_k, pure.temperature_k);
        assert_eq!(state.pressure_pa, pure.pressure_pa);
        assert!(state.pressure_pa > MAX_MIXTURE_PRESSURE_PA);
        assert_eq!(state.air_partial_pressure_pa, 0.0);
        assert_eq!(state.air_energy_j, 0.0);
        assert_eq!(state.liquid_water_mass_kg, pure.liquid_mass_kg);
        assert_eq!(state.vapor_water_mass_kg, pure.vapor_mass_kg);
        assert_eq!(state.liquid_volume_m3, pure.liquid_volume_m3);
        assert_eq!(state.gas_volume_m3, pure.vapor_volume_m3);
        assert_eq!(state.energy_residual_j, pure.energy_residual_j);
        relative(
            state.water_energy_j,
            water.internal_energy_j + pure.energy_residual_j,
            2e-15,
        );
        assert_eq!(
            state.vapor_partial_pressure_pa,
            if phase == WaterPhase::Liquid {
                0.0
            } else {
                pure.pressure_pa
            }
        );
        // Pure liquid has no gas pressure. Its mechanical pressure remains finite.
        if phase == WaterPhase::Vapor {
            conserved(input, state);
        } else {
            assert_eq!(state.gas_volume_m3, 0.0);
            assert_eq!(state.vapor_partial_pressure_pa, 0.0);
        }
    }
}

#[test]
fn zero_air_preserves_the_pure_water_saturation_closure() {
    let input = two_phase(450.0, 0.25, 0.0);
    let pure = close_water(
        table(),
        WaterInventory {
            mass_kg: input.water_mass_kg,
            internal_energy_j: input.internal_energy_j,
            available_volume_m3: input.available_volume_m3,
        },
    )
    .unwrap();
    let state = close_mixture(table(), input).unwrap();
    assert_eq!(state.temperature_k, pure.temperature_k);
    assert_eq!(state.pressure_pa, pure.pressure_pa);
    assert_eq!(state.vapor_partial_pressure_pa, pure.pressure_pa);
    assert_eq!(state.air_partial_pressure_pa, 0.0);
    assert_eq!(state.air_energy_j, 0.0);
    assert_eq!(state.liquid_water_mass_kg, pure.liquid_mass_kg);
    assert_eq!(state.vapor_water_mass_kg, pure.vapor_mass_kg);
    assert_eq!(state.energy_residual_j, pure.energy_residual_j);
    conserved(input, state);
}

#[test]
fn dry_air_rejects_temperatures_and_pressures_outside_its_domain() {
    for (temperature_k, pressure_pa) in [
        (273.14, 100_000.0),
        (623.16, 100_000.0),
        (400.0, MIN_PRESSURE_PA - 1.0),
        (400.0, MAX_MIXTURE_PRESSURE_PA + 1.0),
    ] {
        let input = MixtureInventory {
            water_mass_kg: 0.0,
            air_mass_kg: 1.0,
            internal_energy_j: AIR_HEAT_CAPACITY_J_KG_K
                * (temperature_k - AIR_REFERENCE_TEMPERATURE_K),
            available_volume_m3: AIR_GAS_CONSTANT_J_KG_K * temperature_k / pressure_pa,
        };
        assert!(close_mixture(table(), input).is_err());
    }
}

#[test]
fn dry_air_closes_at_both_total_pressure_boundaries() {
    for temperature_k in [AIR_REFERENCE_TEMPERATURE_K, 400.0, 623.15] {
        for pressure_pa in [MIN_PRESSURE_PA, MAX_MIXTURE_PRESSURE_PA] {
            for air_mass_kg in [1e-6, 1.0, 1e3, 1e6] {
                let input = MixtureInventory {
                    water_mass_kg: 0.0,
                    air_mass_kg,
                    internal_energy_j: air_mass_kg
                        * AIR_HEAT_CAPACITY_J_KG_K
                        * (temperature_k - AIR_REFERENCE_TEMPERATURE_K),
                    available_volume_m3: air_mass_kg * AIR_GAS_CONSTANT_J_KG_K * temperature_k
                        / pressure_pa,
                };
                let state = close_mixture(table(), input).unwrap();
                relative(state.temperature_k, temperature_k, 2e-15);
                relative(state.pressure_pa, pressure_pa, 2e-15);
                conserved(input, state);
                let vessel = MixtureVessel::new(table(), input).unwrap();
                assert_eq!(vessel.inventory(), input);
                assert_eq!(vessel.equilibrium(), state);
            }
        }
    }
}
