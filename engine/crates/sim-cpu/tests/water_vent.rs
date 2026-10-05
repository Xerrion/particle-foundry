//! Finite gas exchange, donor enthalpy, source ledgers and atomic rejection.
use particle_sim_cpu::water::{
    AIR_GAS_CONSTANT_J_KG_K, AIR_HEAT_CAPACITY_J_KG_K, AIR_REFERENCE_TEMPERATURE_K,
    MAX_VENT_GAS_FRACTION, MixtureEquilibrium, MixtureInventory, MixtureVessel, VentError,
    VentExchange, VentReservoir, VentStep, WaterPhase, WaterTable,
};
use std::sync::OnceLock;

fn table() -> &'static WaterTable {
    static TABLE: OnceLock<WaterTable> = OnceLock::new();
    TABLE.get_or_init(WaterTable::new)
}

fn air_enthalpy(temperature_k: f64) -> f64 {
    AIR_HEAT_CAPACITY_J_KG_K * (temperature_k - AIR_REFERENCE_TEMPERATURE_K)
        + AIR_GAS_CONSTANT_J_KG_K * temperature_k
}

fn gas_inventory(
    temperature_k: f64,
    vapor_pressure_pa: f64,
    air_pressure_pa: f64,
) -> MixtureInventory {
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

fn two_phase_inventory(
    temperature_k: f64,
    vapor_fraction: f64,
    air_pressure_pa: f64,
) -> MixtureInventory {
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

fn dry_air_inventory(temperature_k: f64, pressure_pa: f64, mass_kg: f64) -> MixtureInventory {
    MixtureInventory {
        water_mass_kg: 0.0,
        air_mass_kg: mass_kg,
        internal_energy_j: mass_kg
            * AIR_HEAT_CAPACITY_J_KG_K
            * (temperature_k - AIR_REFERENCE_TEMPERATURE_K),
        available_volume_m3: mass_kg * AIR_GAS_CONSTANT_J_KG_K * temperature_k / pressure_pa,
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
    assert!(
        (state.water_energy_j + state.air_energy_j - input.internal_energy_j).abs()
            <= 1.1e-11 * energy_scale
    );
}

fn ledger(before: MixtureInventory, after: MixtureInventory, exchange: VentExchange) {
    assert_eq!(
        exchange.water_mass_delta_kg,
        after.water_mass_kg - before.water_mass_kg
    );
    assert_eq!(
        exchange.air_mass_delta_kg,
        after.air_mass_kg - before.air_mass_kg
    );
    assert_eq!(
        exchange.internal_energy_delta_j,
        after.internal_energy_j - before.internal_energy_j
    );
    assert_eq!(
        exchange.energy_roundoff_j,
        exchange.internal_energy_delta_j - exchange.source_enthalpy_j
    );
    assert_eq!(after.available_volume_m3, before.available_volume_m3);
    assert!(after.water_mass_kg >= 0.0);
    assert!(after.air_mass_kg >= 0.0);
}

fn zero_exchange(exchange: VentExchange) {
    assert_eq!(exchange.requested_gas_mass_delta_kg, 0.0);
    assert_eq!(exchange.water_mass_delta_kg, 0.0);
    assert_eq!(exchange.air_mass_delta_kg, 0.0);
    assert_eq!(exchange.source_enthalpy_j, 0.0);
    assert_eq!(exchange.internal_energy_delta_j, 0.0);
    assert_eq!(exchange.energy_roundoff_j, 0.0);
    assert_eq!(exchange.water_mass_roundoff_kg, 0.0);
    assert_eq!(exchange.air_mass_roundoff_kg, 0.0);
}

fn reservoir_observation(reservoir: &VentReservoir) -> [f64; 7] {
    [
        reservoir.temperature_k(),
        reservoir.pressure_pa(),
        reservoir.air_partial_pressure_pa(),
        reservoir.vapor_partial_pressure_pa(),
        reservoir.water_mass_fraction(),
        reservoir.air_mass_fraction(),
        reservoir.specific_enthalpy_j_kg(),
    ]
}

#[test]
fn reservoir_gas_fractions_and_enthalpy_follow_component_densities_and_flow_work() {
    let vapor = table()
        .sample_single_phase(WaterPhase::Vapor, 450.0, 100_000.0)
        .unwrap();
    let air_density = 110_000.0 / (AIR_GAS_CONSTANT_J_KG_K * 450.0);
    let vapor_density = 1.0 / vapor.volume_m3_kg;
    let water_fraction = vapor_density / (vapor_density + air_density);
    let vapor_enthalpy = vapor.energy_j_kg + 100_000.0 * vapor.volume_m3_kg;
    let reservoir = VentReservoir::new(table(), 450.0, 110_000.0, 100_000.0).unwrap();
    relative(reservoir.pressure_pa(), 210_000.0, 2e-15);
    relative(reservoir.water_mass_fraction(), water_fraction, 2e-15);
    relative(reservoir.air_mass_fraction(), 1.0 - water_fraction, 2e-15);
    relative(
        reservoir.specific_enthalpy_j_kg(),
        water_fraction * vapor_enthalpy + (1.0 - water_fraction) * air_enthalpy(450.0),
        2e-15,
    );
    assert!(
        reservoir.specific_enthalpy_j_kg()
            > water_fraction * vapor.energy_j_kg
                + (1.0 - water_fraction)
                    * AIR_HEAT_CAPACITY_J_KG_K
                    * (450.0 - AIR_REFERENCE_TEMPERATURE_K)
    );
}

#[test]
fn finite_inflow_uses_reservoir_composition_and_actual_component_enthalpy() {
    let input = gas_inventory(400.0, 100_000.0, 100_000.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let reservoir = VentReservoir::new(table(), 450.0, 110_000.0, 100_000.0).unwrap();
    let reservoir_before = reservoir_observation(&reservoir);
    let vapor = table()
        .sample_single_phase(WaterPhase::Vapor, 450.0, 100_000.0)
        .unwrap();
    let vapor_enthalpy = vapor.energy_j_kg + 100_000.0 * vapor.volume_m3_kg;
    let step = VentStep {
        conductance_kg_s_pa: 1e-7,
        dt_s: 1.0,
    };
    let exchange = vessel.apply_vent(table(), &reservoir, step).unwrap();
    let requested =
        step.conductance_kg_s_pa * (reservoir.pressure_pa() - before.pressure_pa) * step.dt_s;
    assert!(exchange.water_mass_delta_kg > 0.0);
    assert!(exchange.air_mass_delta_kg > 0.0);
    assert_eq!(exchange.requested_gas_mass_delta_kg, requested);
    relative(
        exchange.water_mass_delta_kg,
        requested * reservoir.water_mass_fraction(),
        2e-12,
    );
    relative(
        exchange.air_mass_delta_kg,
        requested * reservoir.air_mass_fraction(),
        2e-12,
    );
    assert_eq!(
        exchange.water_mass_roundoff_kg,
        exchange.water_mass_delta_kg - requested * reservoir.water_mass_fraction()
    );
    assert_eq!(
        exchange.air_mass_roundoff_kg,
        exchange.air_mass_delta_kg - requested * reservoir.air_mass_fraction()
    );
    relative(
        exchange.source_enthalpy_j,
        exchange.water_mass_delta_kg * vapor_enthalpy
            + exchange.air_mass_delta_kg * air_enthalpy(450.0),
        2e-15,
    );
    assert!(vessel.equilibrium().pressure_pa > before.pressure_pa);
    assert!(vessel.equilibrium().pressure_pa < reservoir.pressure_pa());
    assert_eq!(reservoir_observation(&reservoir), reservoir_before);
    ledger(input, vessel.inventory(), exchange);
    conserved(vessel.inventory(), vessel.equilibrium());
}

#[test]
fn finite_outflow_uses_chamber_gas_enthalpy_and_signed_source_ledgers() {
    let input = gas_inventory(400.0, 100_000.0, 100_000.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let reservoir = VentReservoir::new(table(), 350.0, 195_000.0, 0.0).unwrap();
    let vapor = table()
        .sample_single_phase(WaterPhase::Vapor, 400.0, 100_000.0)
        .unwrap();
    let gas_mass = input.water_mass_kg + input.air_mass_kg;
    let step = VentStep {
        conductance_kg_s_pa: 1e-7,
        dt_s: 1.0,
    };
    let exchange = vessel.apply_vent(table(), &reservoir, step).unwrap();
    let requested = step.conductance_kg_s_pa * (reservoir.pressure_pa() - before.pressure_pa);
    assert!(exchange.water_mass_delta_kg < 0.0);
    assert!(exchange.air_mass_delta_kg < 0.0);
    assert!(exchange.source_enthalpy_j < 0.0);
    relative(
        exchange.water_mass_delta_kg,
        requested * input.water_mass_kg / gas_mass,
        2e-8,
    );
    relative(
        exchange.air_mass_delta_kg,
        requested * input.air_mass_kg / gas_mass,
        2e-8,
    );
    relative(
        exchange.source_enthalpy_j,
        exchange.water_mass_delta_kg * (vapor.energy_j_kg + 100_000.0 * vapor.volume_m3_kg)
            + exchange.air_mass_delta_kg * air_enthalpy(400.0),
        2e-8,
    );
    assert!(vessel.equilibrium().pressure_pa < before.pressure_pa);
    assert!(vessel.equilibrium().pressure_pa > reservoir.pressure_pa());
    ledger(input, vessel.inventory(), exchange);
    conserved(vessel.inventory(), vessel.equilibrium());
}

#[test]
fn two_phase_outflow_selects_vapor_and_air_without_extracting_bulk_liquid() {
    let input = two_phase_inventory(400.0, 0.1, 50_000.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let reservoir = VentReservoir::new(table(), 400.0, before.pressure_pa * 0.98, 0.0).unwrap();
    let saturation = table().saturation().sample(400.0).unwrap();
    let gas_mass = 0.1 + input.air_mass_kg;
    let step = VentStep {
        conductance_kg_s_pa: 1e-9,
        dt_s: 1.0,
    };
    let exchange = vessel.apply_vent(table(), &reservoir, step).unwrap();
    let requested = step.conductance_kg_s_pa * (reservoir.pressure_pa() - before.pressure_pa);
    let gas_water_fraction = 0.1 / gas_mass;
    let bulk_water_fraction = 1.0 / (1.0 + input.air_mass_kg);
    relative(
        exchange.water_mass_delta_kg,
        requested * gas_water_fraction,
        2e-8,
    );
    relative(
        exchange.air_mass_delta_kg,
        requested * input.air_mass_kg / gas_mass,
        2e-8,
    );
    assert!((gas_water_fraction - bulk_water_fraction).abs() > 0.1);
    relative(
        exchange.source_enthalpy_j,
        exchange.water_mass_delta_kg * saturation.vapor_enthalpy_j_kg()
            + exchange.air_mass_delta_kg * air_enthalpy(400.0),
        2e-8,
    );
    assert!(vessel.equilibrium().liquid_water_mass_kg > 0.89);
    // Closure can change the phase split after gas removal. Total water is ledgered.
    ledger(input, vessel.inventory(), exchange);
    conserved(vessel.inventory(), vessel.equilibrium());
}

#[test]
fn pure_steam_and_dry_air_keep_the_absent_component_absent() {
    for input in [
        gas_inventory(450.0, 100_000.0, 0.0),
        dry_air_inventory(400.0, 100_000.0, 1.0),
    ] {
        let mut vessel = MixtureVessel::new(table(), input).unwrap();
        let reservoir = if input.air_mass_kg == 0.0 {
            VentReservoir::new(table(), 450.0, 0.0, 105_000.0).unwrap()
        } else {
            VentReservoir::new(table(), 400.0, 105_000.0, 0.0).unwrap()
        };
        let exchange = vessel
            .apply_vent(
                table(),
                &reservoir,
                VentStep {
                    conductance_kg_s_pa: 1e-7,
                    dt_s: 1.0,
                },
            )
            .unwrap();
        if input.air_mass_kg == 0.0 {
            assert_eq!(exchange.air_mass_delta_kg, 0.0);
            assert_eq!(vessel.inventory().air_mass_kg, 0.0);
            assert!(exchange.water_mass_delta_kg > 0.0);
        } else {
            assert_eq!(exchange.water_mass_delta_kg, 0.0);
            assert_eq!(vessel.inventory().water_mass_kg, 0.0);
            assert!(exchange.air_mass_delta_kg > 0.0);
            relative(
                exchange.source_enthalpy_j,
                exchange.air_mass_delta_kg * air_enthalpy(400.0),
                2e-15,
            );
        }
        ledger(input, vessel.inventory(), exchange);
        conserved(vessel.inventory(), vessel.equilibrium());
    }
}

#[test]
fn closed_zero_duration_and_equal_pressure_controls_leave_the_whole_vessel_unchanged() {
    let input = dry_air_inventory(400.0, 100_000.0, 1.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let large_difference = VentReservoir::new(table(), 400.0, 200_000.0, 0.0).unwrap();
    for step in [
        VentStep {
            conductance_kg_s_pa: 0.0,
            dt_s: 1.0,
        },
        VentStep {
            conductance_kg_s_pa: 1.0,
            dt_s: 0.0,
        },
    ] {
        zero_exchange(vessel.apply_vent(table(), &large_difference, step).unwrap());
        assert_eq!(vessel.inventory(), input);
        assert_eq!(vessel.equilibrium(), before);
    }
    let equal_pressure = VentReservoir::new(table(), 450.0, 0.0, before.pressure_pa).unwrap();
    zero_exchange(
        vessel
            .apply_vent(
                table(),
                &equal_pressure,
                VentStep {
                    conductance_kg_s_pa: 1.0,
                    dt_s: 1.0,
                },
            )
            .unwrap(),
    );
    assert_eq!(vessel.inventory(), input);
    assert_eq!(vessel.equilibrium(), before);
}

#[test]
fn repeated_finite_steps_relax_pressure_without_resetting_to_the_reservoir() {
    let input = dry_air_inventory(400.0, 100_000.0, 1.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let reservoir = VentReservoir::new(table(), 400.0, 105_000.0, 0.0).unwrap();
    let reservoir_before = reservoir_observation(&reservoir);
    let initial_pressure = vessel.equilibrium().pressure_pa;
    let mut previous_pressure = initial_pressure;
    let mut mass_ledger = 0.0;
    let mut energy_ledger = 0.0;
    let mut enthalpy_ledger = 0.0;
    let mut roundoff_ledger = 0.0;
    for _ in 0..100 {
        let before = vessel.inventory();
        let exchange = vessel
            .apply_vent(
                table(),
                &reservoir,
                VentStep {
                    conductance_kg_s_pa: 1e-6,
                    dt_s: 0.1,
                },
            )
            .unwrap();
        let pressure = vessel.equilibrium().pressure_pa;
        assert!(pressure > previous_pressure);
        assert!(pressure < reservoir.pressure_pa());
        assert!(exchange.requested_gas_mass_delta_kg <= MAX_VENT_GAS_FRACTION * before.air_mass_kg);
        ledger(before, vessel.inventory(), exchange);
        conserved(vessel.inventory(), vessel.equilibrium());
        mass_ledger += exchange.air_mass_delta_kg;
        energy_ledger += exchange.internal_energy_delta_j;
        enthalpy_ledger += exchange.source_enthalpy_j;
        roundoff_ledger += exchange.energy_roundoff_j;
        previous_pressure = pressure;
    }
    assert!(
        reservoir.pressure_pa() - previous_pressure
            < 0.25 * (reservoir.pressure_pa() - initial_pressure)
    );
    relative(
        vessel.inventory().air_mass_kg - input.air_mass_kg,
        mass_ledger,
        2e-13,
    );
    relative(
        vessel.inventory().internal_energy_j - input.internal_energy_j,
        energy_ledger,
        2e-13,
    );
    relative(energy_ledger, enthalpy_ledger + roundoff_ledger, 2e-13);
    assert_eq!(reservoir_observation(&reservoir), reservoir_before);
}

#[test]
fn dry_air_pressure_relaxation_converges_to_the_analytic_flow_equation_under_subdivision() {
    let input = dry_air_inventory(400.0, 100_000.0, 1.0);
    let reservoir = VentReservoir::new(table(), 400.0, 105_000.0, 0.0).unwrap();
    let conductance = 1e-6;
    let rate = AIR_GAS_CONSTANT_J_KG_K / input.available_volume_m3
        * (air_enthalpy(400.0) / AIR_HEAT_CAPACITY_J_KG_K + AIR_REFERENCE_TEMPERATURE_K)
        * conductance;
    let analytic =
        reservoir.pressure_pa() - (reservoir.pressure_pa() - 100_000.0) * (-rate * 10.0).exp();
    let mut errors = Vec::new();
    for substeps in [10, 20, 40] {
        let mut vessel = MixtureVessel::new(table(), input).unwrap();
        let dt_s = 10.0 / f64::from(substeps);
        for _ in 0..substeps {
            vessel
                .apply_vent(
                    table(),
                    &reservoir,
                    VentStep {
                        conductance_kg_s_pa: conductance,
                        dt_s,
                    },
                )
                .unwrap();
        }
        let error = (vessel.equilibrium().pressure_pa - analytic).abs();
        errors.push(error);
        let discrete = reservoir.pressure_pa()
            - (reservoir.pressure_pa() - 100_000.0) * (1.0 - rate * dt_s).powi(substeps);
        relative(vessel.equilibrium().pressure_pa, discrete, 2e-13);
    }
    assert!(errors[1] < 0.55 * errors[0]);
    assert!(errors[2] < 0.55 * errors[1]);
}

#[test]
fn adiabatic_dry_air_discharge_converges_to_the_mass_temperature_and_pressure_relations() {
    let input = dry_air_inventory(400.0, 200_000.0, 1.0);
    let reservoir = VentReservoir::new(table(), 400.0, 195_000.0, 0.0).unwrap();
    let mut temperature_errors = Vec::new();
    let mut pressure_errors = Vec::new();
    for substeps in [5, 10, 20] {
        let mut vessel = MixtureVessel::new(table(), input).unwrap();
        for _ in 0..substeps {
            let before = vessel.inventory();
            let exchange = vessel
                .apply_vent(
                    table(),
                    &reservoir,
                    VentStep {
                        conductance_kg_s_pa: 1e-6,
                        dt_s: 5.0 / f64::from(substeps),
                    },
                )
                .unwrap();
            assert!(exchange.air_mass_delta_kg < 0.0);
            ledger(before, vessel.inventory(), exchange);
            conserved(vessel.inventory(), vessel.equilibrium());
        }
        let state = vessel.equilibrium();
        let mass_ratio = vessel.inventory().air_mass_kg / input.air_mass_kg;
        assert!(mass_ratio < 1.0);
        assert!(state.temperature_k < 400.0);
        assert!(state.pressure_pa > reservoir.pressure_pa());
        // Donor enthalpy produces the gamma=1.4 adiabatic discharge limit.
        temperature_errors.push((state.temperature_k / 400.0 - mass_ratio.powf(0.4)).abs());
        pressure_errors.push((state.pressure_pa / 200_000.0 - mass_ratio.powf(1.4)).abs());
    }
    assert!(temperature_errors[1] < 0.6 * temperature_errors[0]);
    assert!(temperature_errors[2] < 0.6 * temperature_errors[1]);
    assert!(pressure_errors[1] < 0.6 * pressure_errors[0]);
    assert!(pressure_errors[2] < 0.6 * pressure_errors[1]);
}

#[test]
fn extensive_inventory_and_conductance_scaling_preserve_the_post_vent_state() {
    let input = gas_inventory(400.0, 100_000.0, 100_000.0);
    let reservoir = VentReservoir::new(table(), 450.0, 110_000.0, 100_000.0).unwrap();
    let mut reference = MixtureVessel::new(table(), input).unwrap();
    let reference_exchange = reference
        .apply_vent(
            table(),
            &reservoir,
            VentStep {
                conductance_kg_s_pa: 1e-7,
                dt_s: 1.0,
            },
        )
        .unwrap();
    for scale in [1e-6, 1.0, 1e6] {
        let scaled_input = MixtureInventory {
            water_mass_kg: input.water_mass_kg * scale,
            air_mass_kg: input.air_mass_kg * scale,
            internal_energy_j: input.internal_energy_j * scale,
            available_volume_m3: input.available_volume_m3 * scale,
        };
        let mut vessel = MixtureVessel::new(table(), scaled_input).unwrap();
        let exchange = vessel
            .apply_vent(
                table(),
                &reservoir,
                VentStep {
                    conductance_kg_s_pa: 1e-7 * scale,
                    dt_s: 1.0,
                },
            )
            .unwrap();
        relative(
            vessel.equilibrium().temperature_k,
            reference.equilibrium().temperature_k,
            2e-9,
        );
        relative(
            vessel.equilibrium().pressure_pa,
            reference.equilibrium().pressure_pa,
            2e-8,
        );
        relative(
            exchange.water_mass_delta_kg / scale,
            reference_exchange.water_mass_delta_kg,
            2e-12,
        );
        relative(
            exchange.air_mass_delta_kg / scale,
            reference_exchange.air_mass_delta_kg,
            2e-12,
        );
        relative(
            exchange.source_enthalpy_j / scale,
            reference_exchange.source_enthalpy_j,
            2e-12,
        );
        ledger(scaled_input, vessel.inventory(), exchange);
        conserved(vessel.inventory(), vessel.equilibrium());
    }
}

#[test]
fn invalid_reservoir_properties_reject_without_creating_a_boundary_source() {
    for (temperature_k, air_pressure_pa, vapor_pressure_pa) in [
        (f64::NAN, 100_000.0, 0.0),
        (400.0, f64::INFINITY, 0.0),
        (400.0, 0.0, f64::NEG_INFINITY),
        (400.0, -1.0, 100_000.0),
        (400.0, 100_000.0, -1.0),
        (400.0, 0.0, 0.0),
        (273.14, 100_000.0, 0.0),
        (623.16, 100_000.0, 0.0),
        (400.0, 9999.0, 0.0),
        (400.0, 1_000_001.0, 0.0),
        (400.0, 0.0, 9999.0),
        (350.0, 100_000.0, 100_000.0),
        (600.0, 0.0, 1_000_001.0),
        (400.0, f64::MAX, f64::MAX),
    ] {
        assert!(
            VentReservoir::new(table(), temperature_k, air_pressure_pa, vapor_pressure_pa).is_err()
        );
    }
}

#[test]
fn invalid_negative_overflowed_and_unrepresentable_steps_leave_state_unchanged() {
    let input = dry_air_inventory(400.0, 100_000.0, 1.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let reservoir = VentReservoir::new(table(), 400.0, 105_000.0, 0.0).unwrap();
    for step in [
        VentStep {
            conductance_kg_s_pa: -1.0,
            dt_s: 1.0,
        },
        VentStep {
            conductance_kg_s_pa: 1e-7,
            dt_s: -1.0,
        },
        VentStep {
            conductance_kg_s_pa: f64::NAN,
            dt_s: 1.0,
        },
        VentStep {
            conductance_kg_s_pa: 1e-7,
            dt_s: f64::INFINITY,
        },
        VentStep {
            conductance_kg_s_pa: f64::MAX,
            dt_s: f64::MAX,
        },
        VentStep {
            conductance_kg_s_pa: 1e-300,
            dt_s: 1.0,
        },
    ] {
        assert!(vessel.apply_vent(table(), &reservoir, step).is_err());
        assert_eq!(vessel.inventory(), input);
        assert_eq!(vessel.equilibrium(), before);
    }
}

#[test]
fn oversized_turnover_large_pressure_difference_and_pressure_crossing_reject_atomically() {
    let input = dry_air_inventory(400.0, 100_000.0, 1.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    for (pressure_pa, conductance, expected) in [
        (105_000.0, 4e-6, VentError::StepTooLarge),
        (200_000.0, 1e-8, VentError::UnsupportedFlowDomain),
        (100_100.0, 5e-5, VentError::PressureCrossing),
        (99_900.0, 5e-5, VentError::PressureCrossing),
    ] {
        let reservoir = VentReservoir::new(table(), 400.0, pressure_pa, 0.0).unwrap();
        assert_eq!(
            vessel
                .apply_vent(
                    table(),
                    &reservoir,
                    VentStep {
                        conductance_kg_s_pa: conductance,
                        dt_s: 1.0
                    }
                )
                .unwrap_err(),
            expected
        );
        assert_eq!(vessel.inventory(), input);
        assert_eq!(vessel.equilibrium(), before);
    }
}

#[test]
fn unsupported_candidate_closure_rolls_back_the_entire_vent_transaction() {
    for (temperature_k, reservoir_pressure) in [(623.15, 101_000.0), (273.15, 99_000.0)] {
        let input = dry_air_inventory(temperature_k, 100_000.0, 1.0);
        let mut vessel = MixtureVessel::new(table(), input).unwrap();
        let before = vessel.equilibrium();
        let reservoir =
            VentReservoir::new(table(), temperature_k, reservoir_pressure, 0.0).unwrap();
        assert!(matches!(
            vessel.apply_vent(
                table(),
                &reservoir,
                VentStep {
                    conductance_kg_s_pa: 1e-7,
                    dt_s: 1.0
                }
            ),
            Err(VentError::Closure(_))
        ));
        assert_eq!(vessel.inventory(), input);
        assert_eq!(vessel.equilibrium(), before);
    }
}

#[test]
fn steam_ingress_below_the_supported_vapor_partial_pressure_rolls_back_both_species_and_energy() {
    let input = dry_air_inventory(400.0, 200_000.0, 1.0);
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    let reservoir = VentReservoir::new(table(), 450.0, 0.0, 205_000.0).unwrap();
    assert!(matches!(
        vessel.apply_vent(
            table(),
            &reservoir,
            VentStep {
                conductance_kg_s_pa: 1e-7,
                dt_s: 1.0,
            }
        ),
        Err(VentError::Closure(_))
    ));
    assert_eq!(vessel.inventory(), input);
    assert_eq!(vessel.equilibrium(), before);
}

#[test]
fn a_full_liquid_vessel_has_no_gas_inventory_to_vent() {
    let liquid = table()
        .sample_single_phase(WaterPhase::Liquid, 300.0, 300_000.0)
        .unwrap();
    let input = MixtureInventory {
        water_mass_kg: 1.0,
        air_mass_kg: 0.0,
        internal_energy_j: liquid.energy_j_kg,
        available_volume_m3: liquid.volume_m3_kg,
    };
    let mut vessel = MixtureVessel::new(table(), input).unwrap();
    let before = vessel.equilibrium();
    assert_eq!(before.gas_volume_m3, 0.0);
    let reservoir = VentReservoir::new(table(), 400.0, 305_000.0, 0.0).unwrap();
    assert_eq!(
        vessel
            .apply_vent(
                table(),
                &reservoir,
                VentStep {
                    conductance_kg_s_pa: 1e-9,
                    dt_s: 1.0
                }
            )
            .unwrap_err(),
        VentError::UnsupportedFlowDomain
    );
    assert_eq!(vessel.inventory(), input);
    assert_eq!(vessel.equilibrium(), before);
}
