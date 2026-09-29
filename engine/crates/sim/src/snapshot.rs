//! Versioned, explicit checkpoint schema; no normal-frame world readback.

use std::num::NonZeroU32;

use crate::{
    CELL_WIDTH_M, Grid,
    contracts::{
        ActiveComponents, Boundaries, Boundary, ChemicalReference, ComponentDefinition,
        ComponentId, ComponentRole, ContractError, EnergyReference, FormId, MAX_ACTIVE_COMPONENTS,
        PhysicalModel, PropertyDomain, SpeciesId,
    },
};

const MAGIC: [u8; 4] = *b"PFSN";
/// Current explicit checkpoint schema, independent of the legacy cell format.
pub const SNAPSHOT_VERSION: u32 = 1;
/// Bounded reserved extension size; no allocation is made when it is absent.
pub const MAX_EXTENSION_BYTES: usize = 65_536;

/// Optional future state carried only by an explicit checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NuclearExtension {
    /// Only version 1 is round-tripped; other versions fail preflight.
    pub version: u32,
    /// Opaque reserved payload, not an enabled nuclear simulation.
    pub payload: Vec<u8>,
}

/// One consistent, detached generation; it is not a continuously ticking CPU mirror.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Validated physical geometry.
    pub grid: Grid,
    /// Snapshot physical model; only lowMach is enabled in this schema revision.
    pub model: PhysicalModel,
    /// Hash of the catalogue projection that supplied active definitions.
    pub catalogue_sha256: [u8; 32],
    /// Declared future reaction network ID and version; presence does not enable chemistry.
    pub network: Option<(NonZeroU32, NonZeroU32)>,
    /// Prevents passive energy from being interpreted as thermodynamic U.
    pub energy_reference: EnergyReference,
    /// Chemical energy convention; chemistry is disabled in schema version 1.
    pub chemical_reference: ChemicalReference,
    /// One shared boundary selection for momentum, mass, species and energy.
    pub boundaries: Boundaries,
    /// Scene generation for stale-command and readback rejection.
    pub epoch: u64,
    /// Last committed outer tick.
    pub tick: u64,
    /// Total accepted physical time, excluding rejected candidates.
    pub accepted_time_s: f64,
    /// Last applied command, retained for exactly-once source accounting.
    pub last_applied_sequence: u64,
    /// Counter-based random stream position.
    pub random_counter: u64,
    /// Compact scene-local component mapping.
    pub active: ActiveComponents,
    /// Component-major authoritative mass in kg: slot * cell_count + cell.
    pub component_mass_kg: Vec<f64>,
    /// Per-cell energy marker in J until a supported U reference exists.
    pub energy_j: Vec<f64>,
    /// MAC horizontal face velocity in m/s.
    pub u_m_s: Vec<f64>,
    /// MAC vertical face velocity in m/s.
    pub v_m_s: Vec<f64>,
    /// Fixed-wall occupancy, one byte per cell, values zero or one.
    pub fixed_wall: Vec<u8>,
    /// Cumulative signed source mass in kg for each active component.
    pub source_mass_kg: Vec<f64>,
    /// Cumulative signed boundary mass in kg: slot * 4 + outer face.
    pub boundary_mass_kg: Vec<f64>,
    /// Cumulative signed external source energy in J.
    pub source_energy_j: f64,
    /// Cumulative signed boundary energy in J, in boundary order.
    pub boundary_energy_j: [f64; 4],
    /// Reserved versioned extension, absent in nonnuclear scenes.
    pub nuclear_extension: Option<NuclearExtension>,
}

impl Snapshot {
    /// Checks a complete detached generation before serializing or loading it.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.grid.cell_width_m() != CELL_WIDTH_M {
            return Err(ContractError::Unsupported(
                "snapshot v1 cannot encode nondefault cell width",
            ));
        }
        if self.model != PhysicalModel::LowMach {
            return Err(ContractError::Unsupported("snapshot model is not lowMach"));
        }
        if self.energy_reference != EnergyReference::PassiveMarker {
            return Err(ContractError::Unsupported(
                "internal-energy conversion needs a validated property model",
            ));
        }
        if self.chemical_reference != ChemicalReference::Disabled {
            return Err(ContractError::Unsupported(
                "chemical energy requires a validated network and property model",
            ));
        }
        if self.catalogue_sha256 == [0; 32] {
            return Err(ContractError::Invalid(
                "snapshot catalogue fingerprint is missing",
            ));
        }
        if self.epoch == 0 || !self.accepted_time_s.is_finite() || self.accepted_time_s < 0.0 {
            return Err(ContractError::Invalid(
                "invalid snapshot epoch or accepted time",
            ));
        }
        let cells = self.grid.cells();
        let components = self.active.definitions().len();
        if self.component_mass_kg.len() != cells * components
            || self.energy_j.len() != cells
            || self.u_m_s.len() != self.grid.u_faces()
            || self.v_m_s.len() != self.grid.v_faces()
            || self.fixed_wall.len() != cells
            || self.source_mass_kg.len() != components
            || self.boundary_mass_kg.len() != components * 4
        {
            return Err(ContractError::Invalid(
                "snapshot field lengths do not match geometry and active set",
            ));
        }
        if self
            .component_mass_kg
            .iter()
            .any(|mass| !mass.is_finite() || *mass < 0.0)
            || self.fixed_wall.iter().any(|wall| *wall > 1)
            || self
                .energy_j
                .iter()
                .chain(&self.u_m_s)
                .chain(&self.v_m_s)
                .chain(&self.source_mass_kg)
                .chain(&self.boundary_mass_kg)
                .chain(&self.boundary_energy_j)
                .chain(std::iter::once(&self.source_energy_j))
                .any(|value| !value.is_finite())
        {
            return Err(ContractError::Invalid(
                "snapshot contains nonfinite values, negative mass or invalid wall flags",
            ));
        }
        if let Some(extension) = &self.nuclear_extension {
            if extension.version != 1 {
                return Err(ContractError::Unsupported(
                    "unknown nuclear extension version",
                ));
            }
            if extension.payload.len() > MAX_EXTENSION_BYTES {
                return Err(ContractError::Capacity {
                    requested: extension.payload.len(),
                    maximum: MAX_EXTENSION_BYTES,
                });
            }
        }
        Ok(())
    }

    /// Serializes a validated checkpoint in a fixed little-endian wire format.
    pub fn encode(&self) -> Result<Vec<u8>, ContractError> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&MAGIC);
        put_u32(&mut bytes, SNAPSHOT_VERSION);
        put_u32(&mut bytes, self.grid.width());
        put_u32(&mut bytes, self.grid.height());
        put_u32(&mut bytes, 1); // lowMach
        bytes.extend_from_slice(&self.catalogue_sha256);
        if let Some((id, version)) = self.network {
            put_u32(&mut bytes, id.get());
            put_u32(&mut bytes, version.get());
        } else {
            put_u32(&mut bytes, 0);
        }
        put_u32(&mut bytes, 0); // passive energy marker
        put_u32(&mut bytes, 0); // chemistry disabled
        for boundary in self.boundaries {
            put_u32(
                &mut bytes,
                match boundary {
                    Boundary::Closed => 0,
                    Boundary::Open { reservoir } => reservoir.get(),
                },
            );
        }
        put_u64(&mut bytes, self.epoch);
        put_u64(&mut bytes, self.tick);
        put_f64(&mut bytes, self.accepted_time_s);
        put_u64(&mut bytes, self.last_applied_sequence);
        put_u64(&mut bytes, self.random_counter);
        put_u32(&mut bytes, self.active.definitions().len() as u32);
        for definition in self.active.definitions() {
            put_u32(&mut bytes, definition.id.get());
            put_u32(&mut bytes, definition.species.get());
            put_u32(&mut bytes, definition.form.get());
            put_u32(
                &mut bytes,
                definition.isotope_signature.map_or(0, NonZeroU32::get),
            );
            put_u32(&mut bytes, definition.role.code());
            if let Some(domain) = &definition.property_domain {
                put_u32(&mut bytes, domain.source.get());
                put_u32(&mut bytes, domain.version.get());
                for value in domain.temperature_k.into_iter().chain(domain.pressure_pa) {
                    put_f64(&mut bytes, value);
                }
            } else {
                put_u32(&mut bytes, 0);
            }
        }
        for values in [
            &self.component_mass_kg,
            &self.energy_j,
            &self.u_m_s,
            &self.v_m_s,
            &self.source_mass_kg,
            &self.boundary_mass_kg,
        ] {
            for value in values {
                put_f64(&mut bytes, *value);
            }
        }
        bytes.extend_from_slice(&self.fixed_wall);
        put_f64(&mut bytes, self.source_energy_j);
        for value in self.boundary_energy_j {
            put_f64(&mut bytes, value);
        }
        if let Some(extension) = &self.nuclear_extension {
            put_u32(&mut bytes, extension.version);
            put_u32(&mut bytes, extension.payload.len() as u32);
            bytes.extend_from_slice(&extension.payload);
        } else {
            put_u32(&mut bytes, 0);
        }
        Ok(bytes)
    }

    /// Parses and validates a complete checkpoint without accepting trailing or unknown data.
    pub fn decode(bytes: &[u8]) -> Result<Self, ContractError> {
        let mut reader = Reader::new(bytes);
        if reader.array::<4>()? != MAGIC {
            return Err(ContractError::Invalid("snapshot magic mismatch"));
        }
        if reader.u32()? != SNAPSHOT_VERSION {
            return Err(ContractError::Unsupported("snapshot schema version"));
        }
        let grid = Grid::new(reader.u32()? as f64, reader.u32()? as f64)
            .map_err(ContractError::Invalid)?;
        if reader.u32()? != 1 {
            return Err(ContractError::Unsupported("snapshot model is not lowMach"));
        }
        let catalogue_sha256 = reader.array::<32>()?;
        let network_id = reader.u32()?;
        let network = if network_id == 0 {
            None
        } else {
            Some((
                NonZeroU32::new(network_id).expect("checked nonzero"),
                NonZeroU32::new(reader.u32()?)
                    .ok_or(ContractError::Invalid("zero network version"))?,
            ))
        };
        if reader.u32()? != 0 {
            return Err(ContractError::Unsupported(
                "internal-energy conversion needs a validated property model",
            ));
        }
        if reader.u32()? != 0 {
            return Err(ContractError::Unsupported(
                "chemical energy requires a validated network and property model",
            ));
        }
        let mut boundaries = [Boundary::Closed; 4];
        for boundary in &mut boundaries {
            if let Some(reservoir) = NonZeroU32::new(reader.u32()?) {
                *boundary = Boundary::Open { reservoir };
            }
        }
        let epoch = reader.u64()?;
        let tick = reader.u64()?;
        let accepted_time_s = reader.f64()?;
        let last_applied_sequence = reader.u64()?;
        let random_counter = reader.u64()?;
        let count = reader.u32()? as usize;
        if count == 0 || count > MAX_ACTIVE_COMPONENTS {
            return Err(ContractError::Capacity {
                requested: count,
                maximum: MAX_ACTIVE_COMPONENTS,
            });
        }
        let mut definitions = Vec::with_capacity(count);
        for _ in 0..count {
            let id = ComponentId::new(reader.u32()?)?;
            let species = SpeciesId::new(reader.u32()?)?;
            let form = FormId::new(reader.u32()?)?;
            let isotope_signature = NonZeroU32::new(reader.u32()?);
            let role = ComponentRole::from_code(reader.u32()?)?;
            let source = NonZeroU32::new(reader.u32()?);
            let property_domain = if let Some(source) = source {
                let version = NonZeroU32::new(reader.u32()?)
                    .ok_or(ContractError::Invalid("zero property version"))?;
                Some(PropertyDomain {
                    source,
                    version,
                    temperature_k: [reader.f64()?, reader.f64()?],
                    pressure_pa: [reader.f64()?, reader.f64()?],
                })
            } else {
                None
            };
            definitions.push(ComponentDefinition {
                id,
                species,
                form,
                isotope_signature,
                role,
                property_domain,
            });
        }
        let active = ActiveComponents::new(definitions)?;
        let component_mass_kg = reader.f64_vec(grid.cells() * count)?;
        let energy_j = reader.f64_vec(grid.cells())?;
        let u_m_s = reader.f64_vec(grid.u_faces())?;
        let v_m_s = reader.f64_vec(grid.v_faces())?;
        let source_mass_kg = reader.f64_vec(count)?;
        let boundary_mass_kg = reader.f64_vec(count * 4)?;
        let fixed_wall = reader.take(grid.cells())?.to_vec();
        let source_energy_j = reader.f64()?;
        let boundary_energy_j = [reader.f64()?, reader.f64()?, reader.f64()?, reader.f64()?];
        let extension_version = reader.u32()?;
        let nuclear_extension = if extension_version == 0 {
            None
        } else {
            if extension_version != 1 {
                return Err(ContractError::Unsupported(
                    "unknown nuclear extension version",
                ));
            }
            let length = reader.u32()? as usize;
            if length > MAX_EXTENSION_BYTES {
                return Err(ContractError::Capacity {
                    requested: length,
                    maximum: MAX_EXTENSION_BYTES,
                });
            }
            Some(NuclearExtension {
                version: extension_version,
                payload: reader.take(length)?.to_vec(),
            })
        };
        if reader.remaining() != 0 {
            return Err(ContractError::Invalid("trailing snapshot bytes"));
        }
        let snapshot = Self {
            grid,
            model: PhysicalModel::LowMach,
            catalogue_sha256,
            network,
            energy_reference: EnergyReference::PassiveMarker,
            chemical_reference: ChemicalReference::Disabled,
            boundaries,
            epoch,
            tick,
            accepted_time_s,
            last_applied_sequence,
            random_counter,
            active,
            component_mass_kg,
            energy_j,
            u_m_s,
            v_m_s,
            fixed_wall,
            source_mass_kg,
            boundary_mass_kg,
            source_energy_j,
            boundary_energy_j,
            nuclear_extension,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], ContractError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ContractError::Invalid("snapshot length overflow"))?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ContractError::Invalid("truncated snapshot"))?;
        self.position = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ContractError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32, ContractError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, ContractError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn f64(&mut self) -> Result<f64, ContractError> {
        Ok(f64::from_le_bytes(self.array()?))
    }
    fn f64_vec(&mut self, count: usize) -> Result<Vec<f64>, ContractError> {
        let length = count
            .checked_mul(8)
            .ok_or(ContractError::Invalid("snapshot field length overflow"))?;
        let bytes = self.take(length)?;
        Ok(bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|chunk| f64::from_le_bytes(*chunk))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(grid: Grid) -> Snapshot {
        let active = ActiveComponents::new(vec![ComponentDefinition {
            id: ComponentId::new(1).unwrap(),
            species: SpeciesId::new(1).unwrap(),
            form: FormId::new(1).unwrap(),
            isotope_signature: None,
            role: ComponentRole::Carrier,
            property_domain: None,
        }])
        .unwrap();
        Snapshot {
            grid,
            model: PhysicalModel::LowMach,
            catalogue_sha256: [1; 32],
            network: None,
            energy_reference: EnergyReference::PassiveMarker,
            chemical_reference: ChemicalReference::Disabled,
            boundaries: [Boundary::Closed; 4],
            epoch: 1,
            tick: 0,
            accepted_time_s: 0.0,
            last_applied_sequence: 0,
            random_counter: 0,
            active,
            component_mass_kg: vec![1.0; grid.cells()],
            energy_j: vec![0.0; grid.cells()],
            u_m_s: vec![0.0; grid.u_faces()],
            v_m_s: vec![0.0; grid.v_faces()],
            fixed_wall: vec![0; grid.cells()],
            source_mass_kg: vec![0.0],
            boundary_mass_kg: vec![0.0; 4],
            source_energy_j: 0.0,
            boundary_energy_j: [0.0; 4],
            nuclear_extension: None,
        }
    }

    #[test]
    fn version_one_round_trips_default_grid_and_rejects_refined_grid() {
        let default = sample(Grid::new(1.0, 1.0).unwrap());
        assert_eq!(
            Snapshot::decode(&default.encode().unwrap()).unwrap(),
            default
        );

        let refined = sample(Grid::with_cell_width(1.0, 1.0, 0.005).unwrap());
        assert!(matches!(
            refined.encode(),
            Err(ContractError::Unsupported(_))
        ));
    }
}
