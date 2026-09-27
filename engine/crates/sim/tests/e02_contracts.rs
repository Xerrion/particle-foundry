//! E02 state, identity, source and scalar ABI contract regression tests.

use std::{
    mem::{align_of, size_of},
    num::NonZeroU32,
};

use particle_sim::{
    Grid, REPRESENTED_DEPTH_M,
    contracts::{
        ActiveComponents, Boundary, ChemicalReference, CommandStamp, CommandStatus,
        ComponentDefinition, ComponentId, ComponentRole, ContractError, ElementId, EnergyReference,
        FormId, LegacyFireUse, LegacyId, MAX_ACTIVE_COMPONENTS, NuclideId, PhysicalModel,
        PropertyDomain, SourceTransaction, SpeciesId, legacy_fire_use,
    },
    gpu_layout::{GpuCellHeader, GpuGridParams, mass_byte_offset},
    snapshot::{NuclearExtension, SNAPSHOT_VERSION, Snapshot},
};

fn component(id: u32, role: ComponentRole) -> ComponentDefinition {
    ComponentDefinition {
        id: ComponentId::new(id).unwrap(),
        species: SpeciesId::new(id).unwrap(),
        form: FormId::new(id).unwrap(),
        isotope_signature: None,
        role,
        property_domain: None,
    }
}

fn sample() -> Snapshot {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let active = ActiveComponents::new(vec![
        component(1, ComponentRole::Carrier),
        component(2, ComponentRole::Liquid),
        component(3, ComponentRole::Fuel),
        component(4, ComponentRole::Oxidizer),
        component(5, ComponentRole::Product),
        component(6, ComponentRole::Particulate),
    ])
    .unwrap();
    Snapshot {
        grid,
        model: PhysicalModel::LowMach,
        catalogue_sha256: [7; 32],
        network: Some((NonZeroU32::new(9).unwrap(), NonZeroU32::new(1).unwrap())),
        energy_reference: EnergyReference::PassiveMarker,
        chemical_reference: ChemicalReference::Disabled,
        boundaries: [
            Boundary::Closed,
            Boundary::Open {
                reservoir: NonZeroU32::new(3).unwrap(),
            },
            Boundary::Closed,
            Boundary::Closed,
        ],
        epoch: 2,
        tick: 7,
        accepted_time_s: 7.0 / 60.0,
        last_applied_sequence: 15,
        random_counter: 99,
        active,
        component_mass_kg: vec![
            0.3, 0.4, 0.2, 0.1, 0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.001, 0.002,
        ],
        energy_j: vec![12.0, 13.0],
        u_m_s: vec![0.0, 0.2, 0.0],
        v_m_s: vec![0.0; 4],
        fixed_wall: vec![0, 1],
        source_mass_kg: vec![0.0; 6],
        boundary_mass_kg: vec![0.0; 24],
        source_energy_j: 5.0,
        boundary_energy_j: [0.0, -0.5, 0.0, 0.0],
        nuclear_extension: None,
    }
}

#[test]
fn legacy_and_scientific_ids_do_not_collapse_to_one_namespace() {
    for value in 0..=255 {
        assert_eq!(LegacyId::new(value).unwrap().get(), value as u8);
    }
    assert_eq!(LegacyId::new(255).unwrap().get(), 255);
    assert!(LegacyId::new(256).is_err());
    assert_eq!(ElementId::new(118).unwrap().get(), 118);
    assert!(ElementId::new(0).is_err());
    assert!(ElementId::new(119).is_err());
    assert!(NuclideId::new(ElementId::new(8).unwrap(), 7, 0).is_err());
    assert_eq!(
        NuclideId::new(ElementId::new(8).unwrap(), 16, 1)
            .unwrap()
            .mass_number,
        16
    );
    assert!(SpeciesId::new(0).is_err());
    assert!(FormId::new(0).is_err());
    assert!(ComponentId::new(0).is_err());
    let fire = LegacyId::new(8).unwrap();
    assert_eq!(
        legacy_fire_use(fire, fire, true),
        Some(LegacyFireUse::IgnitionCommand)
    );
    assert_eq!(
        legacy_fire_use(fire, fire, false),
        Some(LegacyFireUse::VisualOnly)
    );
    assert_eq!(
        legacy_fire_use(LegacyId::new(10).unwrap(), fire, false),
        None
    );
}

#[test]
fn component_reservation_and_product_closure_are_bounded_and_atomic() {
    let initial = ActiveComponents::new(vec![
        component(1, ComponentRole::Carrier),
        component(2, ComponentRole::Liquid),
    ])
    .unwrap();
    assert_eq!(initial.index_of(ComponentId::new(2).unwrap()), Some(1));
    assert_eq!(initial.index_of(ComponentId::new(3).unwrap()), None);
    assert_eq!(
        initial.require(&[ComponentId::new(3).unwrap()]),
        Err(ContractError::MissingComponent(
            ComponentId::new(3).unwrap()
        ))
    );
    let network = initial
        .with_reserved(&[
            component(3, ComponentRole::Fuel),
            component(4, ComponentRole::Oxidizer),
            component(5, ComponentRole::Product),
            component(6, ComponentRole::Particulate),
        ])
        .unwrap();
    network
        .require(&[3, 4, 5, 6].map(|id| ComponentId::new(id).unwrap()))
        .unwrap();
    assert_eq!(initial.definitions().len(), 2);
    assert_eq!(network.definitions().len(), 6);
    assert!(
        network
            .definitions()
            .iter()
            .all(|definition| definition.property_domain.is_none())
    );

    let full = ActiveComponents::new(
        (1..=MAX_ACTIVE_COMPONENTS as u32)
            .map(|id| component(id, ComponentRole::Marker))
            .collect(),
    )
    .unwrap();
    assert_eq!(
        full.with_reserved(&[component(17, ComponentRole::Product)]),
        Err(ContractError::Capacity {
            requested: 17,
            maximum: MAX_ACTIVE_COMPONENTS
        })
    );
    assert_eq!(full.definitions().len(), MAX_ACTIVE_COMPONENTS);
    assert_eq!(
        initial.with_reserved(&[component(1, ComponentRole::Fuel)]),
        Err(ContractError::ConflictingComponent(
            ComponentId::new(1).unwrap()
        ))
    );
    assert!(
        ActiveComponents::new(vec![
            component(1, ComponentRole::Carrier),
            component(1, ComponentRole::Carrier)
        ])
        .is_err()
    );

    let mut bad = component(7, ComponentRole::Marker);
    bad.property_domain = Some(PropertyDomain {
        source: NonZeroU32::new(1).unwrap(),
        version: NonZeroU32::new(1).unwrap(),
        temperature_k: [300.0, f64::NAN],
        pressure_pa: [0.0, 101_325.0],
    });
    assert!(ActiveComponents::new(vec![bad]).is_err());
}

#[test]
fn source_proposal_preflights_products_without_committing_an_inventory() {
    let active = ActiveComponents::new(vec![
        component(3, ComponentRole::Fuel),
        component(4, ComponentRole::Oxidizer),
        component(5, ComponentRole::Product),
    ])
    .unwrap();
    let stamp = CommandStamp {
        epoch: 2,
        sequence: 16,
        intended_tick: 8,
    };
    let mut proposal = SourceTransaction {
        stamp,
        component_delta_kg: vec![
            (ComponentId::new(3).unwrap(), -0.2),
            (ComponentId::new(4).unwrap(), -0.1),
            (ComponentId::new(5).unwrap(), 0.3),
        ],
        energy_delta_j: 10.0,
        volume_delta_m3: 0.0,
    };
    proposal.validate(&active).unwrap();
    assert_eq!(CommandStatus::Queued(stamp), CommandStatus::Queued(stamp));
    proposal.component_delta_kg[2].0 = ComponentId::new(6).unwrap();
    assert_eq!(
        proposal.validate(&active),
        Err(ContractError::MissingComponent(
            ComponentId::new(6).unwrap()
        ))
    );
    proposal.component_delta_kg[2].0 = ComponentId::new(3).unwrap();
    assert_eq!(
        proposal.validate(&active),
        Err(ContractError::ConflictingComponent(
            ComponentId::new(3).unwrap()
        ))
    );
    proposal.component_delta_kg[2].0 = ComponentId::new(5).unwrap();
    proposal.energy_delta_j = f64::NAN;
    assert!(proposal.validate(&active).is_err());
    proposal.energy_delta_j = 10.0;
    proposal.stamp.sequence = 0;
    assert!(proposal.validate(&active).is_err());
}

#[test]
fn snapshot_round_trip_preserves_one_inventory_and_source_ledger() {
    let snapshot = sample();
    let bytes = snapshot.encode().unwrap();
    assert_eq!(&bytes[..4], b"PFSN");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        SNAPSHOT_VERSION
    );
    assert_eq!(Snapshot::decode(&bytes).unwrap(), snapshot);
    for prefix in 0..bytes.len() {
        assert!(
            Snapshot::decode(&bytes[..prefix]).is_err(),
            "accepted truncated prefix {prefix}"
        );
    }
    let mut unsupported_chemistry = bytes.clone();
    let chemical_reference_offset = 5 * 4 + 32 + 2 * 4 + 4;
    unsupported_chemistry[chemical_reference_offset..chemical_reference_offset + 4]
        .copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        Snapshot::decode(&unsupported_chemistry),
        Err(ContractError::Unsupported(_))
    ));
    assert!(
        Snapshot::decode(&bytes)
            .unwrap()
            .nuclear_extension
            .is_none()
    );

    let mut extended = snapshot;
    extended.nuclear_extension = Some(NuclearExtension {
        version: 1,
        payload: vec![3, 4, 5],
    });
    let bytes = extended.encode().unwrap();
    assert_eq!(Snapshot::decode(&bytes).unwrap(), extended);
    let mut unknown = bytes.clone();
    let version_offset = unknown.len() - 11;
    unknown[version_offset..version_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    assert!(matches!(
        Snapshot::decode(&unknown),
        Err(ContractError::Unsupported(_))
    ));
    assert!(Snapshot::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Snapshot::decode(&trailing).is_err());
    let mut unknown_schema = bytes;
    unknown_schema[4..8].copy_from_slice(&2_u32.to_le_bytes());
    assert!(matches!(
        Snapshot::decode(&unknown_schema),
        Err(ContractError::Unsupported(_))
    ));
}

#[test]
fn malformed_snapshot_cannot_be_serialized_or_imported_as_internal_energy() {
    let mut snapshot = sample();
    snapshot.model = PhysicalModel::Compressible;
    assert!(matches!(
        snapshot.encode(),
        Err(ContractError::Unsupported(_))
    ));
    snapshot.model = PhysicalModel::LowMach;
    snapshot.component_mass_kg[3] = -0.1;
    assert!(snapshot.encode().is_err());
    snapshot.component_mass_kg[3] = f64::NAN;
    assert!(snapshot.encode().is_err());
    snapshot.component_mass_kg[3] = 0.0;
    snapshot.fixed_wall[0] = 2;
    assert!(snapshot.encode().is_err());
    snapshot.fixed_wall[0] = 0;
    snapshot.catalogue_sha256 = [0; 32];
    assert!(snapshot.encode().is_err());
    snapshot.catalogue_sha256 = [7; 32];
    snapshot.energy_reference = EnergyReference::InternalEnergy {
        reference: NonZeroU32::new(1).unwrap(),
    };
    assert!(matches!(
        snapshot.encode(),
        Err(ContractError::Unsupported(_))
    ));
    snapshot.energy_reference = EnergyReference::PassiveMarker;
    snapshot.chemical_reference = ChemicalReference::Formation {
        source: NonZeroU32::new(1).unwrap(),
        version: NonZeroU32::new(1).unwrap(),
    };
    assert!(matches!(
        snapshot.encode(),
        Err(ContractError::Unsupported(_))
    ));
    snapshot.chemical_reference = ChemicalReference::Disabled;
    snapshot.component_mass_kg.pop();
    assert!(snapshot.encode().is_err());
}

#[test]
fn shader_scalar_abi_and_mac_indices_have_explicit_offsets() {
    assert_eq!(REPRESENTED_DEPTH_M, 0.01);
    assert_eq!(size_of::<GpuCellHeader>(), 8);
    assert_eq!(align_of::<GpuCellHeader>(), 4);
    assert_eq!(size_of::<GpuGridParams>(), 16);
    assert_eq!(align_of::<GpuGridParams>(), 16);
    let cell = GpuCellHeader::new(12.5, 1).unwrap();
    assert_eq!(cell.to_le_bytes(), [0, 0, 72, 65, 1, 0, 0, 0]);
    assert_eq!(
        GpuCellHeader::from_le_bytes(cell.to_le_bytes()).unwrap(),
        cell
    );
    assert!(GpuCellHeader::new(f64::INFINITY, 0).is_err());
    assert!(GpuCellHeader::new(0.0, 2).is_err());
    let grid = Grid::new(2.0, 3.0).unwrap();
    assert_eq!(grid.cell_index(1, 2), Some(5));
    assert_eq!(grid.u_face_index(2, 2), Some(8));
    assert_eq!(grid.v_face_index(1, 3), Some(7));
    assert_eq!(grid.u_face_index(3, 1), None);
    assert_eq!(grid.v_face_index(1, 4), None);
    assert_eq!(mass_byte_offset(grid, 2, 1, 5).unwrap(), 44);
    assert!(mass_byte_offset(grid, 2, 2, 0).is_err());
    let params = GpuGridParams::new(grid, 2).unwrap();
    assert_eq!(
        params.to_le_bytes(),
        [2, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]
    );
}
