//! Single-phase inversion, conservation and reversible saturation transitions.
use particle_sim_cpu::water::{
    MAX_PRESSURE_PA, MAX_TEMPERATURE_K, MIN_PRESSURE_PA, MIN_TEMPERATURE_K, WaterError,
    WaterInventory, WaterPhase, WaterTable, WaterVessel, close_water,
};
use std::sync::OnceLock;

fn table() -> &'static WaterTable {
    static TABLE: OnceLock<WaterTable> = OnceLock::new();
    TABLE.get_or_init(WaterTable::new)
}

fn inventory(phase: WaterPhase, t: f64, p: f64, mass: f64) -> WaterInventory {
    let s = table().sample_single_phase(phase, t, p).unwrap();
    WaterInventory {
        mass_kg: mass,
        internal_energy_j: mass * s.energy_j_kg,
        available_volume_m3: mass * s.volume_m3_kg,
    }
}

fn relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs(),
        "{actual} != {expected}"
    );
}

#[test]
fn published_liquid_control_point_inverts_mass_energy_and_volume() {
    // Independent IF97 Table 5 values, not produced by this implementation.
    let input = WaterInventory {
        mass_kg: 1.0,
        internal_energy_j: 112324.818,
        available_volume_m3: 0.00100215168,
    };
    let state = close_water(table(), input).unwrap();
    relative(state.temperature_k, 300.0, 2e-7);
    relative(state.pressure_pa, 3e6, 5e-5);
    assert_eq!(state.vapor_mass_fraction, 0.0);
    assert!(state.energy_residual_j.abs() < 2e-6);
}

#[test]
fn both_phase_branches_invert_interior_and_boundary_states_at_all_mass_scales() {
    for (phase, t, p) in [
        (WaterPhase::Liquid, MIN_TEMPERATURE_K, MIN_PRESSURE_PA),
        (WaterPhase::Liquid, 275.3, 3e6),
        (WaterPhase::Liquid, 279.7, 2e6),
        (WaterPhase::Liquid, 300.0, MAX_PRESSURE_PA),
        (WaterPhase::Liquid, 400.37, 1e6),
        (WaterPhase::Liquid, MAX_TEMPERATURE_K, 18e6),
        (WaterPhase::Vapor, 350.37, MIN_PRESSURE_PA),
        (WaterPhase::Vapor, 450.73, 1e5),
        (WaterPhase::Vapor, 600.21, 1e6),
        (WaterPhase::Vapor, MAX_TEMPERATURE_K, 10e6),
    ] {
        for mass in [1e-6, 1.0, 1e6] {
            let input = inventory(phase, t, p, mass);
            let state = close_water(table(), input).unwrap();
            relative(state.temperature_k, t, 2e-10);
            // Pressure is more sensitive than energy for compressed liquid.
            relative(state.pressure_pa, p, 2e-5);
            relative(state.liquid_mass_kg + state.vapor_mass_kg, mass, 2e-15);
            relative(
                state.liquid_volume_m3 + state.vapor_volume_m3,
                input.available_volume_m3,
                2e-13,
            );
            assert_eq!(
                state.vapor_mass_fraction,
                if phase == WaterPhase::Vapor { 1.0 } else { 0.0 }
            );
            assert!(
                state.energy_residual_j.abs()
                    <= 1.1e-11 * input.internal_energy_j.abs().max(1000.0 * mass)
            );
            for _ in 0..100 {
                assert_eq!(close_water(table(), input).unwrap(), state);
            }
        }
    }
}

#[test]
fn heated_full_liquid_vessel_has_finite_pressure_and_a_reversible_heat_ledger() {
    let input = inventory(WaterPhase::Liquid, 300.0, 3e6, 1.0);
    let mut vessel = WaterVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let mut ledger = 0.0;
    for _ in 0..100 {
        ledger += vessel.apply_heat(table(), 100.0).unwrap();
    }
    let hot = vessel.equilibrium();
    assert!(hot.pressure_pa > before.pressure_pa);
    assert!(hot.temperature_k > before.temperature_k);
    assert_eq!(hot.vapor_mass_fraction, 0.0);
    assert_eq!(vessel.inventory().mass_kg, input.mass_kg);
    assert_eq!(
        vessel.inventory().available_volume_m3,
        input.available_volume_m3
    );
    assert_eq!(
        vessel.inventory().internal_energy_j - input.internal_energy_j,
        ledger
    );
    for _ in 0..100 {
        ledger += vessel.apply_heat(table(), -100.0).unwrap();
    }
    assert_eq!(ledger, 0.0);
    assert_eq!(vessel.inventory(), input);
    assert_eq!(vessel.equilibrium(), before);
}

#[test]
fn heating_and_cooling_cross_both_saturation_boundaries_without_duplicate_inventories() {
    let sat = table().saturation().sample(450.73).unwrap();
    for (volume, energy, phase) in [
        (
            sat.liquid_volume_m3_kg,
            sat.liquid_energy_j_kg,
            WaterPhase::Liquid,
        ),
        (
            sat.vapor_volume_m3_kg,
            sat.vapor_energy_j_kg,
            WaterPhase::Vapor,
        ),
    ] {
        let input = WaterInventory {
            mass_kg: 1.0,
            internal_energy_j: energy,
            available_volume_m3: volume,
        };
        let mut vessel = WaterVessel::new(table(), input).unwrap();
        let before = vessel.equilibrium();
        // At fixed saturated-liquid volume, heating enters compressed liquid.
        // At fixed saturated-vapor volume, heating enters superheated vapor.
        vessel.apply_heat(table(), 1000.0).unwrap();
        assert_eq!(
            vessel.equilibrium().vapor_mass_fraction,
            if phase == WaterPhase::Vapor { 1.0 } else { 0.0 }
        );
        vessel.apply_heat(table(), -2000.0).unwrap();
        assert!((0.0..1.0).contains(&vessel.equilibrium().vapor_mass_fraction));
        assert!(vessel.equilibrium().vapor_mass_fraction > 0.0);
        vessel.apply_heat(table(), 1000.0).unwrap();
        assert_eq!(vessel.inventory(), input);
        assert_eq!(vessel.equilibrium(), before);
    }
}

#[test]
fn out_of_domain_heat_preserves_single_phase_inventory_and_equilibrium() {
    for phase in [WaterPhase::Liquid, WaterPhase::Vapor] {
        let p = if phase == WaterPhase::Liquid {
            3e6
        } else {
            1e5
        };
        let mut vessel = WaterVessel::new(table(), inventory(phase, 400.0, p, 1.0)).unwrap();
        let input = vessel.inventory();
        let state = vessel.equilibrium();
        for heat in [1e9, -1e9, f64::MAX, f64::NAN, f64::INFINITY] {
            assert!(vessel.apply_heat(table(), heat).is_err());
            assert_eq!(vessel.inventory(), input);
            assert_eq!(vessel.equilibrium(), state);
        }
    }
}

#[test]
fn property_queries_reject_wrong_phases_and_unsupported_domains() {
    for phase in [WaterPhase::Liquid, WaterPhase::Vapor] {
        for p in [MIN_PRESSURE_PA - 1.0, MAX_PRESSURE_PA + 1.0, 0.0] {
            assert_eq!(
                table().sample_single_phase(phase, 450.0, p).unwrap_err(),
                WaterError::PressureOutsideTable
            );
        }
        for t in [MIN_TEMPERATURE_K - 0.01, MAX_TEMPERATURE_K + 0.01] {
            assert_eq!(
                table().sample_single_phase(phase, t, 1e5).unwrap_err(),
                WaterError::TemperatureOutsideTable
            );
        }
        for (t, p) in [(f64::NAN, 1e5), (400.0, f64::INFINITY)] {
            assert_eq!(
                table().sample_single_phase(phase, t, p).unwrap_err(),
                WaterError::NonFiniteInput
            );
        }
    }
    assert_eq!(
        table()
            .sample_single_phase(WaterPhase::Liquid, 500.0, 1e5)
            .unwrap_err(),
        WaterError::UnsupportedEquilibrium
    );
    assert_eq!(
        table()
            .sample_single_phase(WaterPhase::Vapor, 400.0, 1e6)
            .unwrap_err(),
        WaterError::UnsupportedEquilibrium
    );
    assert_eq!(
        table()
            .sample_single_phase(WaterPhase::Vapor, 300.0, 1e5)
            .unwrap_err(),
        WaterError::TemperatureOutsideTable
    );
}

#[test]
fn single_phase_and_saturation_projections_share_their_endpoints() {
    for t in [
        table().saturation().min_temperature_k(),
        373.15,
        450.73,
        MAX_TEMPERATURE_K,
    ] {
        let sat = table().saturation().sample(t).unwrap();
        for (phase, volume, energy) in [
            (
                WaterPhase::Liquid,
                sat.liquid_volume_m3_kg,
                sat.liquid_energy_j_kg,
            ),
            (
                WaterPhase::Vapor,
                sat.vapor_volume_m3_kg,
                sat.vapor_energy_j_kg,
            ),
        ] {
            let s = table()
                .sample_single_phase(phase, t, sat.pressure_pa)
                .unwrap();
            relative(s.volume_m3_kg, volume, 2e-15);
            relative(s.energy_j_kg, energy, 2e-15);
        }
    }
}
