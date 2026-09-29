//! Stable identity, active-component and command metadata for one owned scene.

use std::num::NonZeroU32;

/// Maximum scene-local component slots in the initial GPU contract.
pub const MAX_ACTIVE_COMPONENTS: usize = 16;

/// A contract failure rejects an input before a scene or snapshot is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    /// An input is malformed or out of its declared range.
    Invalid(&'static str),
    /// A valid schema needs a capability that is not enabled.
    Unsupported(&'static str),
    /// An active-component reservation exceeds the declared capacity.
    Capacity {
        /// Number of requested active slots or extension bytes.
        requested: usize,
        /// Maximum supported by this contract.
        maximum: usize,
    },
    /// A source or network requires a missing product or reactant.
    MissingComponent(ComponentId),
    /// A stable component ID was reused for a different definition.
    ConflictingComponent(ComponentId),
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ContractError {}

/// Atomic number in the preserved 1..=118 identity roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId(u8);

impl ElementId {
    /// Rejects zero and values outside the existing element roster.
    pub fn new(value: u32) -> Result<Self, ContractError> {
        if !(1..=118).contains(&value) {
            return Err(ContractError::Invalid(
                "element atomic number must be 1..=118",
            ));
        }
        Ok(Self(value as u8))
    }

    /// Stable atomic number.
    pub fn get(self) -> u8 {
        self.0
    }
}

/// A nuclide identity; no mass or half-life property is inferred from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NuclideId {
    /// Atomic-number identity.
    pub element: ElementId,
    /// Nucleon count, not a rounded atomic weight.
    pub mass_number: u16,
    /// Explicit nuclear-state key; zero denotes the ground state.
    pub state: u16,
}

impl NuclideId {
    /// Rejects a mass number smaller than the atomic number.
    pub fn new(element: ElementId, mass_number: u16, state: u16) -> Result<Self, ContractError> {
        if mass_number < element.get() as u16 {
            return Err(ContractError::Invalid(
                "nuclide mass number is smaller than atomic number",
            ));
        }
        Ok(Self {
            element,
            mass_number,
            state,
        })
    }
}

macro_rules! stable_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(NonZeroU32);

        impl $name {
            /// Rejects zero, which is reserved for absence in the wire format.
            pub fn new(value: u32) -> Result<Self, ContractError> {
                NonZeroU32::new(value)
                    .map(Self)
                    .ok_or(ContractError::Invalid(concat!(
                        stringify!($name),
                        " must be nonzero"
                    )))
            }

            /// Stable integer identity.
            pub fn get(self) -> u32 {
                self.0.get()
            }
        }
    };
}

stable_id!(
    SpeciesId,
    "Stable chemical-species identity, distinct from an element."
);
stable_id!(
    FormId,
    "Stable material-form identity, distinct from a species."
);
stable_id!(
    ComponentId,
    "Stable transported-component identity, distinct from a scene index."
);

/// Existing TypeScript material/tool byte, retained only at the adapter boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacyId(u8);

impl LegacyId {
    /// Preserves the complete legacy byte without wrapping a larger value.
    pub fn new(value: u32) -> Result<Self, ContractError> {
        Ok(Self(u8::try_from(value).map_err(|_| {
            ContractError::Invalid("legacy ID exceeds u8")
        })?))
    }

    /// Original byte value.
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Contextual meaning of the legacy FIRE label, never a species identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyFireUse {
    /// A funded ignition command from the brush.
    IgnitionCommand,
    /// A display label in imported legacy cells.
    VisualOnly,
}

/// Maps only the FIRE ID supplied by the authored TypeScript catalogue.
pub fn legacy_fire_use(id: LegacyId, fire_id: LegacyId, brush: bool) -> Option<LegacyFireUse> {
    (id == fire_id).then_some(if brush {
        LegacyFireUse::IgnitionCommand
    } else {
        LegacyFireUse::VisualOnly
    })
}

/// Physical model, separate from the execution backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicalModel {
    /// Existing TypeScript-only model.
    LegacyCellular,
    /// P1 conservative low-Mach model.
    LowMach,
    /// Future P2 model, represented but not enabled here.
    Compressible,
}

/// Execution ownership, not a physical model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    /// Existing TypeScript/Canvas session.
    Legacy,
    /// Rust f64 reference session.
    CpuReference,
    /// Rust-owned wgpu session.
    Wgpu,
}

/// Component's intended inventory role; this does not enable a reaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentRole {
    /// Nonreacting gas carrier.
    Carrier,
    /// The initial liquid.
    Liquid,
    /// Transported passive composition marker.
    Marker,
    /// Declared future fuel inventory.
    Fuel,
    /// Declared future oxidizer inventory.
    Oxidizer,
    /// Declared future product inventory.
    Product,
    /// Physical particulate inventory, including soot when supported.
    Particulate,
}

impl ComponentRole {
    /// Stable code in snapshot component definitions.
    pub fn code(self) -> u32 {
        match self {
            Self::Carrier => 1,
            Self::Liquid => 2,
            Self::Marker => 3,
            Self::Fuel => 4,
            Self::Oxidizer => 5,
            Self::Product => 6,
            Self::Particulate => 7,
        }
    }

    /// Rejects undefined role codes.
    pub fn from_code(code: u32) -> Result<Self, ContractError> {
        match code {
            1 => Ok(Self::Carrier),
            2 => Ok(Self::Liquid),
            3 => Ok(Self::Marker),
            4 => Ok(Self::Fuel),
            5 => Ok(Self::Oxidizer),
            6 => Ok(Self::Product),
            7 => Ok(Self::Particulate),
            _ => Err(ContractError::Invalid("unknown component role")),
        }
    }
}

/// Source-qualified domain for a supported physical property record.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDomain {
    /// Authored dataset or override identity; zero is invalid.
    pub source: NonZeroU32,
    /// Version of that record within the authored dataset.
    pub version: NonZeroU32,
    /// Inclusive temperature range in kelvin.
    pub temperature_k: [f64; 2],
    /// Inclusive pressure range in pascals.
    pub pressure_pa: [f64; 2],
}

impl PropertyDomain {
    /// Ensures a source and finite, ordered physical domain were declared.
    pub fn validate(&self) -> Result<(), ContractError> {
        let [t0, t1] = self.temperature_k;
        let [p0, p1] = self.pressure_pa;
        if !t0.is_finite() || !t1.is_finite() || t0 <= 0.0 || t0 > t1 {
            return Err(ContractError::Invalid(
                "invalid property temperature domain",
            ));
        }
        if !p0.is_finite() || !p1.is_finite() || p0 < 0.0 || p0 > p1 {
            return Err(ContractError::Invalid("invalid property pressure domain"));
        }
        Ok(())
    }
}

/// One immutable link to future catalogue data; no physical property is invented here.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDefinition {
    /// Stable component key across saves.
    pub id: ComponentId,
    /// Molecular or declared pseudo-species identity.
    pub species: SpeciesId,
    /// Structure/phase identity.
    pub form: FormId,
    /// Optional isotope-signature-table key; zero is absence.
    pub isotope_signature: Option<NonZeroU32>,
    /// Physical inventory role, not a reaction rule.
    pub role: ComponentRole,
    /// Missing means unknown, never zero-valued physical properties.
    pub property_domain: Option<PropertyDomain>,
}

/// Scene-local stable-ID to compact-slot map; this is metadata, not a second world.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveComponents {
    definitions: Vec<ComponentDefinition>,
}

impl ActiveComponents {
    /// Validates a bounded, duplicate-free active set.
    pub fn new(definitions: Vec<ComponentDefinition>) -> Result<Self, ContractError> {
        if definitions.is_empty() {
            return Err(ContractError::Invalid(
                "active scene needs at least one component",
            ));
        }
        if definitions.len() > MAX_ACTIVE_COMPONENTS {
            return Err(ContractError::Capacity {
                requested: definitions.len(),
                maximum: MAX_ACTIVE_COMPONENTS,
            });
        }
        for (index, definition) in definitions.iter().enumerate() {
            if definitions[..index]
                .iter()
                .any(|prior| prior.id == definition.id)
            {
                return Err(ContractError::ConflictingComponent(definition.id));
            }
            if let Some(domain) = &definition.property_domain {
                domain.validate()?;
            }
        }
        Ok(Self { definitions })
    }

    /// Compact definitions in snapshot/GPU slot order.
    pub fn definitions(&self) -> &[ComponentDefinition] {
        &self.definitions
    }

    /// Finds a compact slot without allocating a dense global catalogue.
    pub fn index_of(&self, id: ComponentId) -> Option<u16> {
        self.definitions
            .iter()
            .position(|definition| definition.id == id)
            .map(|index| index as u16)
    }

    /// Preflights every input and product needed by a declared network.
    pub fn require(&self, ids: &[ComponentId]) -> Result<(), ContractError> {
        for id in ids {
            if self.index_of(*id).is_none() {
                return Err(ContractError::MissingComponent(*id));
            }
        }
        Ok(())
    }

    /// Reserves all new identities atomically in metadata before storage allocation.
    pub fn with_reserved(&self, new: &[ComponentDefinition]) -> Result<Self, ContractError> {
        let mut definitions = self.definitions.clone();
        for candidate in new {
            match definitions
                .iter()
                .find(|existing| existing.id == candidate.id)
            {
                Some(existing) if existing == candidate => continue,
                Some(_) => return Err(ContractError::ConflictingComponent(candidate.id)),
                None => definitions.push(candidate.clone()),
            }
        }
        Self::new(definitions)
    }
}

/// One outer face setting shared by pressure, species, momentum and energy flux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// No flux through this face unless an explicit source applies.
    Closed,
    /// Finite flux with a declared reservoir; never an interior composition reset.
    Open {
        /// Stable identity of the finite boundary reservoir.
        reservoir: NonZeroU32,
    },
}

/// Left, right, top and bottom outer boundaries in that order.
pub type Boundaries = [Boundary; 4];

/// Energy semantics that prevent legacy parcel enthalpy from masquerading as U.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnergyReference {
    /// M2's nonreactive energy marker with no thermodynamic claim.
    PassiveMarker,
    /// Future SI internal-energy convention with a declared property reference.
    InternalEnergy {
        /// Validated thermodynamic property/reference-state identity.
        reference: NonZeroU32,
    },
}

/// Chemical-energy accounting convention; disabled until a validated network exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChemicalReference {
    /// No chemical source may commit; the M3 transport demonstrator uses this.
    Disabled,
    /// Future formation-energy convention, tied to an authored dataset version.
    Formation {
        /// Stable property dataset identity.
        source: NonZeroU32,
        /// Version of the formation-energy records.
        version: NonZeroU32,
    },
}

/// A command's epoch/order/scheduling intent, without a JS cell-array payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandStamp {
    /// Session generation; reset/load replaces it.
    pub epoch: u64,
    /// Monotonic ID for acknowledgement and retry deduplication.
    pub sequence: u64,
    /// First accepted outer tick at which the intent may apply.
    pub intended_tick: u64,
}

/// A queue receipt is not proof that physics committed the command.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandStatus {
    /// Accepted into a bounded queue, not yet applied.
    Queued(CommandStamp),
    /// Applied once at this completed state.
    Applied(CommandStamp, ObservationStamp),
    /// Rejected without changing the authoritative state.
    Rejected(CommandStamp, ContractError),
}

/// A detached observation's completed state, never a synchronous live-cell view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObservationStamp {
    /// Session generation.
    pub epoch: u64,
    /// Last accepted outer tick.
    pub tick: u64,
    /// Accumulated accepted physical seconds.
    pub accepted_time_s: f64,
}

/// Proposed mass/energy/volume change for one owning-backend commit barrier.
/// This is a schema seam, not a reaction solver or a second inventory.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceTransaction {
    /// Unique event identity within the epoch, including internal source events.
    pub stamp: CommandStamp,
    /// Signed kg changes by stable component identity; products must be present.
    pub component_delta_kg: Vec<(ComponentId, f64)>,
    /// Signed change to the declared thermal/passive energy channel in J.
    pub energy_delta_j: f64,
    /// Signed geometric volume change in m3 for a later coupled closure stage.
    pub volume_delta_m3: f64,
}

impl SourceTransaction {
    /// Preflights shape and active-set closure before any source is committed.
    /// Stoichiometric and thermodynamic validation is required when chemistry is enabled.
    pub fn validate(&self, active: &ActiveComponents) -> Result<(), ContractError> {
        if self.stamp.epoch == 0 || self.stamp.sequence == 0 {
            return Err(ContractError::Invalid(
                "source event identity must be nonzero",
            ));
        }
        if self.component_delta_kg.len() > active.definitions().len() {
            return Err(ContractError::Capacity {
                requested: self.component_delta_kg.len(),
                maximum: active.definitions().len(),
            });
        }
        if !self.energy_delta_j.is_finite() || !self.volume_delta_m3.is_finite() {
            return Err(ContractError::Invalid("nonfinite source energy or volume"));
        }
        for (index, (id, mass)) in self.component_delta_kg.iter().enumerate() {
            if !mass.is_finite() {
                return Err(ContractError::Invalid("nonfinite source component delta"));
            }
            if self.component_delta_kg[..index]
                .iter()
                .any(|(prior, _)| prior == id)
            {
                return Err(ContractError::ConflictingComponent(*id));
            }
            active.require(&[*id])?;
        }
        Ok(())
    }
}

/// Renderer inputs derived from tracked state, not a second fuel/soot authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireVisualInputs {
    /// Completed-state stamp for stale-result handling.
    pub stamp: ObservationStamp,
    /// Physical heat-release rate in watts; zero when no reaction committed.
    pub heat_release_w: f64,
    /// Derived local temperature in kelvin.
    pub temperature_k: f64,
    /// Derived luminous share in 0..=1, never an ignition or fuel flag.
    pub luminous_fraction: f64,
    /// Concentration derived from tracked particulate mass and physical volume.
    pub soot_concentration_kg_m3: f64,
}
