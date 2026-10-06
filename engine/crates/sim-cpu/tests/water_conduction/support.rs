use particle_sim_cpu::water::{
    AIR_GAS_CONSTANT_J_KG_K, AIR_HEAT_CAPACITY_J_KG_K, AIR_REFERENCE_TEMPERATURE_K,
    ConductionExchange, MixtureEquilibrium, MixtureInventory, MixtureVessel, WaterPhase,
    WaterTable,
};
use std::sync::OnceLock;

pub fn table() -> &'static WaterTable {
    static TABLE: OnceLock<WaterTable> = OnceLock::new();
    TABLE.get_or_init(WaterTable::new)
}

pub fn air(temperature: f64, mass: f64) -> MixtureVessel {
    MixtureVessel::new(
        table(),
        MixtureInventory {
            water_mass_kg: 0.0,
            air_mass_kg: mass,
            internal_energy_j: mass
                * AIR_HEAT_CAPACITY_J_KG_K
                * (temperature - AIR_REFERENCE_TEMPERATURE_K),
            available_volume_m3: mass * AIR_GAS_CONSTANT_J_KG_K * temperature / 100_000.0,
        },
    )
    .unwrap()
}

pub fn mixed(temperature: f64, vapor_fraction: f64, air_pressure: f64) -> MixtureVessel {
    let saturation = table().saturation().sample(temperature).unwrap();
    let liquid = table()
        .sample_single_phase(
            WaterPhase::Liquid,
            temperature,
            saturation.pressure_pa + air_pressure,
        )
        .unwrap();
    let gas_volume = vapor_fraction * saturation.vapor_volume_m3_kg;
    let air_mass = air_pressure * gas_volume / (AIR_GAS_CONSTANT_J_KG_K * temperature);
    MixtureVessel::new(
        table(),
        MixtureInventory {
            water_mass_kg: 1.0,
            air_mass_kg: air_mass,
            internal_energy_j: (1.0 - vapor_fraction) * liquid.energy_j_kg
                + vapor_fraction * saturation.vapor_energy_j_kg
                + air_mass * AIR_HEAT_CAPACITY_J_KG_K * (temperature - AIR_REFERENCE_TEMPERATURE_K),
            available_volume_m3: (1.0 - vapor_fraction) * liquid.volume_m3_kg + gas_volume,
        },
    )
    .unwrap()
}

pub fn state(vessel: &MixtureVessel) -> (MixtureInventory, MixtureEquilibrium) {
    (vessel.inventory(), vessel.equilibrium())
}

pub fn relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1e-300),
        "{actual} != {expected}"
    );
}

pub fn ledger(
    before: [MixtureInventory; 2],
    vessels: [&MixtureVessel; 2],
    exchange: ConductionExchange,
) {
    let after = vessels.map(MixtureVessel::inventory);
    assert_eq!(
        exchange.first_energy_delta_j,
        after[0].internal_energy_j - before[0].internal_energy_j
    );
    assert_eq!(
        exchange.second_energy_delta_j,
        after[1].internal_energy_j - before[1].internal_energy_j
    );
    assert_eq!(
        exchange.energy_roundoff_j,
        exchange.first_energy_delta_j + exchange.second_energy_delta_j
    );
    let heat = exchange.requested_heat_into_first_j;
    relative(exchange.first_energy_delta_j, heat, 1e-10);
    relative(exchange.second_energy_delta_j, -heat, 1e-10);
    assert!(exchange.energy_roundoff_j.abs() <= 1e-10 * heat.abs());
    for index in 0..2 {
        assert_eq!(after[index].water_mass_kg, before[index].water_mass_kg);
        assert_eq!(after[index].air_mass_kg, before[index].air_mass_kg);
        assert_eq!(
            after[index].available_volume_m3,
            before[index].available_volume_m3
        );
        let equilibrium = vessels[index].equilibrium();
        let energy_scale = equilibrium.water_energy_j.abs()
            + equilibrium.air_energy_j.abs()
            + after[index].water_mass_kg * 2_500_000.0;
        assert!(equilibrium.energy_residual_j.abs() <= 1.1e-11 * energy_scale);
        relative(
            equilibrium.liquid_volume_m3 + equilibrium.gas_volume_m3,
            after[index].available_volume_m3,
            2e-12,
        );
    }
}
