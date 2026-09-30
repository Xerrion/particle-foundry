//! Isolated E09 water closure, conservation, source and domain regressions.

use particle_sim_cpu::water::{
    MAX_TEMPERATURE_K, MIN_PRESSURE_PA, SaturationTable, WaterError, WaterInventory, WaterVessel,
    close_water,
};

fn inventory(table: &SaturationTable, t: f64, x: f64, mass: f64) -> WaterInventory {
    let s = table.sample(t).unwrap();
    WaterInventory {
        mass_kg: mass,
        internal_energy_j: mass
            * (s.liquid_energy_j_kg + x * (s.vapor_energy_j_kg - s.liquid_energy_j_kg)),
        available_volume_m3: mass
            * (s.liquid_volume_m3_kg + x * (s.vapor_volume_m3_kg - s.liquid_volume_m3_kg)),
    }
}

fn relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs(),
        "{actual} != {expected}"
    );
}

#[test]
fn pure_water_mass_energy_and_actual_volume_close_without_a_phase_cycle() {
    let table = SaturationTable::new();
    // Independent equilibrium constraints: both phases share p_sat and T,
    // while their extensive mass, volume and internal energy sums fit the input.
    for mass in [1e-6, 1.0, 1e6] {
        for t in [
            table.min_temperature_k(),
            350.37,
            450.73,
            600.21,
            MAX_TEMPERATURE_K,
        ] {
            for x in [1e-6, 0.01, 0.5, 0.99] {
                let input = inventory(&table, t, x, mass);
                let state = close_water(&table, input).unwrap();
                relative(state.temperature_k, t, 2e-10);
                assert!((state.vapor_mass_fraction - x).abs() < 2e-10);
                relative(state.liquid_mass_kg + state.vapor_mass_kg, mass, 2e-15);
                relative(
                    state.liquid_volume_m3 + state.vapor_volume_m3,
                    input.available_volume_m3,
                    2e-15,
                );
                assert!(state.energy_residual_j.abs() < input.internal_energy_j.abs() * 1.1e-11);
                assert!(state.iterations < 64);
                for _ in 0..1000 {
                    assert_eq!(close_water(&table, input).unwrap(), state);
                }
            }
        }
    }
}

#[test]
fn heating_a_rigid_vessel_changes_pressure_and_phase_amount_with_a_source_ledger() {
    let table = SaturationTable::new();
    let input = inventory(&table, 450.0, 0.1, 1.0);
    let mut vessel = WaterVessel::new(&table, input).unwrap();
    let before = vessel.equilibrium();
    let mut source_j = 0.0;
    for _ in 0..100 {
        source_j += vessel.apply_heat(&table, 1000.0).unwrap();
    }
    let hot = vessel.equilibrium();
    assert!(hot.temperature_k > before.temperature_k);
    assert!(hot.pressure_pa > before.pressure_pa);
    assert!(hot.vapor_mass_kg > before.vapor_mass_kg);
    assert_eq!(vessel.inventory().mass_kg, input.mass_kg);
    assert_eq!(
        vessel.inventory().available_volume_m3,
        input.available_volume_m3
    );
    relative(
        vessel.inventory().internal_energy_j - input.internal_energy_j,
        source_j,
        1e-13,
    );
    for _ in 0..100 {
        source_j += vessel.apply_heat(&table, -1000.0).unwrap();
    }
    assert_eq!(source_j, 0.0);
    assert_eq!(vessel.inventory(), input);
    assert_eq!(vessel.equilibrium(), before);
}

#[test]
fn available_geometry_volume_controls_pressure_and_partial_evaporation() {
    let table = SaturationTable::new();
    let input = inventory(&table, 450.0, 0.1, 1.0);
    let a = close_water(&table, input).unwrap();
    // Separate equilibria at equal U. This is not moving-wall work or a timestep.
    let larger = WaterInventory {
        available_volume_m3: input.available_volume_m3 * 2.0,
        ..input
    };
    let b = close_water(&table, larger).unwrap();
    assert!(b.pressure_pa < a.pressure_pa);
    assert!(b.temperature_k < a.temperature_k);
    assert!(b.vapor_mass_kg > a.vapor_mass_kg);
    relative(
        b.liquid_volume_m3 + b.vapor_volume_m3,
        larger.available_volume_m3,
        2e-15,
    );
    assert!(b.energy_residual_j.abs() < input.internal_energy_j * 1.1e-11);
}

#[test]
fn saturated_phase_endpoints_remain_bounded() {
    let table = SaturationTable::new();
    for t in [table.min_temperature_k(), 373.15, 500.0, MAX_TEMPERATURE_K] {
        for x in [0.0, 1.0] {
            let input = inventory(&table, t, x, 1.0);
            let state = close_water(&table, input).unwrap();
            assert!((0.0..=1.0).contains(&state.vapor_mass_fraction));
            assert!((state.vapor_mass_fraction - x).abs() < 2e-10);
            assert!(state.energy_residual_j.abs() < input.internal_energy_j * 1.1e-11);
        }
    }
}

#[test]
fn energy_increases_monotonically_at_fixed_volume_over_each_feasible_interval() {
    let table = SaturationTable::new();
    for v in [0.0011, 0.002, 0.005, 0.01, 0.1, 1.0, 10.0] {
        let mut previous_u = None;
        for i in 0..=4000 {
            let t = table.min_temperature_k()
                + (MAX_TEMPERATURE_K - table.min_temperature_k()) * f64::from(i) / 4000.0;
            let s = table.sample(t).unwrap();
            let x = (v - s.liquid_volume_m3_kg) / (s.vapor_volume_m3_kg - s.liquid_volume_m3_kg);
            if !(0.0..=1.0).contains(&x) {
                continue;
            }
            let u = s.liquid_energy_j_kg + x * (s.vapor_energy_j_kg - s.liquid_energy_j_kg);
            if let Some(previous) = previous_u {
                assert!(u > previous);
            }
            previous_u = Some(u);
        }
        assert!(previous_u.is_some());
    }
}

#[test]
fn pressure_depends_on_temperature_and_enthalpy_is_distinct_from_internal_energy() {
    let table = SaturationTable::new();
    let s = table.sample(373.15).unwrap();
    // Published IF97 Table 35 values. Table interpolation has a separate error bound.
    relative(table.sample(500.0).unwrap().pressure_pa, 2638897.76, 5e-6);
    relative(table.sample(600.0).unwrap().pressure_pa, 12344314.6, 5e-6);
    relative(
        table.sample(table.min_temperature_k()).unwrap().pressure_pa,
        MIN_PRESSURE_PA,
        1e-12,
    );
    assert!(s.vapor_enthalpy_j_kg() - s.vapor_energy_j_kg > 160000.0);
    relative(
        s.vapor_enthalpy_j_kg() - s.vapor_energy_j_kg,
        s.pressure_pa * s.vapor_volume_m3_kg,
        2e-15,
    );
    relative(
        s.liquid_enthalpy_j_kg() - s.liquid_energy_j_kg,
        s.pressure_pa * s.liquid_volume_m3_kg,
        5e-13,
    );
}

#[test]
fn accepted_heat_reports_the_representable_energy_change() {
    let table = SaturationTable::new();
    let mut vessel = WaterVessel::new(&table, inventory(&table, 450.0, 0.1, 1.0)).unwrap();
    let before = vessel.inventory();
    assert_eq!(vessel.apply_heat(&table, 1e-20).unwrap(), 0.0);
    assert_eq!(vessel.inventory(), before);
    let accepted = vessel.apply_heat(&table, 0.1).unwrap();
    assert_eq!(
        accepted,
        vessel.inventory().internal_energy_j - before.internal_energy_j
    );
}

#[test]
fn unsupported_heating_cooling_and_nonfinite_inputs_never_change_state() {
    let table = SaturationTable::new();
    let mut vessel = WaterVessel::new(&table, inventory(&table, 450.0, 0.1, 1.0)).unwrap();
    let inventory = vessel.inventory();
    let equilibrium = vessel.equilibrium();
    for q in [
        1e9,
        -1e9,
        f64::MAX,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        assert!(vessel.apply_heat(&table, q).is_err());
        assert_eq!(vessel.inventory(), inventory);
        assert_eq!(vessel.equilibrium(), equilibrium);
    }
}

#[test]
fn invalid_and_unsupported_domains_reject_instead_of_clamping() {
    let table = SaturationTable::new();
    let input = inventory(&table, 450.0, 0.1, 1.0);
    for bad in [
        WaterInventory {
            mass_kg: 0.0,
            ..input
        },
        WaterInventory {
            mass_kg: -1.0,
            ..input
        },
        WaterInventory {
            available_volume_m3: 0.0,
            ..input
        },
        WaterInventory {
            available_volume_m3: -1.0,
            ..input
        },
    ] {
        assert_eq!(
            close_water(&table, bad),
            Err(WaterError::NonPositiveInventory)
        );
    }
    for bad in [
        WaterInventory {
            mass_kg: f64::NAN,
            ..input
        },
        WaterInventory {
            available_volume_m3: f64::INFINITY,
            ..input
        },
        WaterInventory {
            internal_energy_j: f64::NEG_INFINITY,
            ..input
        },
        WaterInventory {
            mass_kg: f64::MIN_POSITIVE,
            ..input
        },
    ] {
        assert_eq!(close_water(&table, bad), Err(WaterError::NonFiniteInput));
    }
    for bad in [
        WaterInventory {
            internal_energy_j: -1.0,
            ..input
        },
        WaterInventory {
            internal_energy_j: 3e6,
            ..input
        },
        WaterInventory {
            available_volume_m3: 0.0005,
            ..input
        },
        WaterInventory {
            available_volume_m3: 20.0,
            ..input
        },
    ] {
        assert_eq!(
            close_water(&table, bad),
            Err(WaterError::UnsupportedEquilibrium)
        );
    }
    for t in [273.15, 273.16, 318.0, 650.0, 723.15, 3273.15] {
        assert_eq!(
            table.sample(t).unwrap_err(),
            WaterError::TemperatureOutsideTable
        );
    }
    for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(table.sample(t).unwrap_err(), WaterError::NonFiniteInput);
    }
}
