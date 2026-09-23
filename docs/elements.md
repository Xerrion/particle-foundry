# Element coverage and material implementation

**Revision:** 21 September 2026. **Current documented scope:** eight paintable legacy element models and 110 reference-only entries. **Target:** all 118 identities with explicit levels of material, chemical, nuclear, and creative support. No new element behavior is implemented by this documentation package.

Start development at [GPU P1/M0](START_HERE.md), not by making all reference entries paintable in the existing cellular engine. The full implementation sequence is in [roadmap.md](roadmap.md).

## Selection policy

Include all 118 identities. Develop 40 core elemental-material identities first, add 43 for an 83-material palette, then 20 nuclear-first and 15 exotic identities. These are delivery cohorts, not classifications of stability, natural occurrence, or experimental accessibility. Thorium and uranium are core material candidates with nuclear behavior deferred to P6. Some members of the 83 cohort are radioactive; the cohort is not a list of stable elements.

A material form, a chemical species, and a nuclide are not interchangeable. Carbon can have graphite and diamond forms; ordinary hydrogen gas uses H2, while nuclear behavior distinguishes hydrogen isotopes. Do not assign one physical or nuclear profile to an entire element. See [matter architecture](architecture/matter-model.md), [IUPAC](sources.md#iupac-periodic-table), and [carbon](sources.md#rsc-carbon).

## Core gameplay priorities

| Priority | Elements or forms | Proposed implementation emphasis |
| --- | --- | --- |
| Connected chemistry | H, C, N, O, S, P | Defined reactants/products and energy rather than contact-only effects |
| Accessible phase behavior | Ga, Hg, Sn, Bi | Distinct liquid/solid behavior and energy-funded phase transitions |
| Gas variety | He, Ne, Ar, Xe and molecular gases | Shared gas transport; separate excitation/chemical capabilities |
| Reactive surfaces | Li, Na, K, Mg, Al, Ca, halogens | Selected amount-limited reactions and passivation where supported |
| Structures and circuits | Fe, Co, Ni, Cu, Zn, Ti, Cr, Ag, Au, W, Pt, Pb | Thermal/electrical properties, alloys, and later bounded magnetic/catalytic features |
| Minerals and forms | B, Si, graphite, diamond | Compound/form distinction, silica-based sand, and glass presets |
| Nuclear foundation | Th, U, later radioactive isotopes of existing elements | Ordinary material support now; evaluated nuclide behavior later |

This is a development-priority table, not a list of measured universal behavior. Each enabled mechanism needs its own data and tests. Gallium's near-room-temperature melting point makes a useful first physical reference; unknown exotic bulk properties must remain unknown. [Gallium](sources.md#rsc-gallium), [oganesson](sources.md#rsc-oganesson).

## Complete delivery roster

The machine-readable source of cohort membership is [element-roadmap.json](data/element-roadmap.json), validated by [its schema](data/element-roadmap.schema.json) and the documentation validator. Z is an atomic number, never a runtime material ID. `Legacy model documented` means reported by the supplied files, not validated on the new GPU backend.

<!-- BEGIN ELEMENT ROSTER -->
### Core: 40 elemental-material identities

| Z | Symbol | Element | Target phase | Legacy model documented |
| ---: | --- | --- | --- | --- |
| 1 | H | Hydrogen | P4 | Yes |
| 2 | He | Helium | P4 | Yes |
| 3 | Li | Lithium | P4 | No |
| 5 | B | Boron | P4 | No |
| 6 | C | Carbon | P4 | Yes |
| 7 | N | Nitrogen | P4 | Yes |
| 8 | O | Oxygen | P4 | Yes |
| 9 | F | Fluorine | P4 | No |
| 10 | Ne | Neon | P4 | No |
| 11 | Na | Sodium | P4 | No |
| 12 | Mg | Magnesium | P4 | No |
| 13 | Al | Aluminium | P4 | No |
| 14 | Si | Silicon | P4 | No |
| 15 | P | Phosphorus | P4 | No |
| 16 | S | Sulfur | P4 | Yes |
| 17 | Cl | Chlorine | P4 | No |
| 18 | Ar | Argon | P4 | No |
| 19 | K | Potassium | P4 | No |
| 20 | Ca | Calcium | P4 | No |
| 22 | Ti | Titanium | P4 | No |
| 24 | Cr | Chromium | P4 | No |
| 26 | Fe | Iron | P4 | Yes |
| 27 | Co | Cobalt | P4 | No |
| 28 | Ni | Nickel | P4 | No |
| 29 | Cu | Copper | P4 | Yes |
| 30 | Zn | Zinc | P4 | No |
| 31 | Ga | Gallium | P4 | No |
| 35 | Br | Bromine | P4 | No |
| 47 | Ag | Silver | P4 | No |
| 50 | Sn | Tin | P4 | No |
| 53 | I | Iodine | P4 | No |
| 54 | Xe | Xenon | P4 | No |
| 74 | W | Tungsten | P4 | No |
| 78 | Pt | Platinum | P4 | No |
| 79 | Au | Gold | P4 | No |
| 80 | Hg | Mercury | P4 | No |
| 82 | Pb | Lead | P4 | No |
| 83 | Bi | Bismuth | P4 | No |
| 90 | Th | Thorium | P4 | No |
| 92 | U | Uranium | P4 | No |

### Extended: 43 additional identities, 83 cumulative

| Z | Symbol | Element | Target phase | Legacy model documented |
| ---: | --- | --- | --- | --- |
| 4 | Be | Beryllium | P5 | No |
| 21 | Sc | Scandium | P5 | No |
| 23 | V | Vanadium | P5 | No |
| 25 | Mn | Manganese | P5 | No |
| 32 | Ge | Germanium | P5 | No |
| 33 | As | Arsenic | P5 | No |
| 34 | Se | Selenium | P5 | No |
| 36 | Kr | Krypton | P5 | No |
| 37 | Rb | Rubidium | P5 | No |
| 38 | Sr | Strontium | P5 | No |
| 39 | Y | Yttrium | P5 | No |
| 40 | Zr | Zirconium | P5 | No |
| 41 | Nb | Niobium | P5 | No |
| 42 | Mo | Molybdenum | P5 | No |
| 44 | Ru | Ruthenium | P5 | No |
| 45 | Rh | Rhodium | P5 | No |
| 46 | Pd | Palladium | P5 | No |
| 48 | Cd | Cadmium | P5 | No |
| 49 | In | Indium | P5 | No |
| 51 | Sb | Antimony | P5 | No |
| 52 | Te | Tellurium | P5 | No |
| 55 | Cs | Caesium | P5 | No |
| 56 | Ba | Barium | P5 | No |
| 57 | La | Lanthanum | P5 | No |
| 58 | Ce | Cerium | P5 | No |
| 59 | Pr | Praseodymium | P5 | No |
| 60 | Nd | Neodymium | P5 | No |
| 62 | Sm | Samarium | P5 | No |
| 63 | Eu | Europium | P5 | No |
| 64 | Gd | Gadolinium | P5 | No |
| 65 | Tb | Terbium | P5 | No |
| 66 | Dy | Dysprosium | P5 | No |
| 67 | Ho | Holmium | P5 | No |
| 68 | Er | Erbium | P5 | No |
| 69 | Tm | Thulium | P5 | No |
| 70 | Yb | Ytterbium | P5 | No |
| 71 | Lu | Lutetium | P5 | No |
| 72 | Hf | Hafnium | P5 | No |
| 73 | Ta | Tantalum | P5 | No |
| 75 | Re | Rhenium | P5 | No |
| 76 | Os | Osmium | P5 | No |
| 77 | Ir | Iridium | P5 | No |
| 81 | Tl | Thallium | P5 | No |

### Nuclear-first: 20 additional identities, 103 cumulative

| Z | Symbol | Element | Target phase | Legacy model documented |
| ---: | --- | --- | --- | --- |
| 43 | Tc | Technetium | P6 | No |
| 61 | Pm | Promethium | P6 | No |
| 84 | Po | Polonium | P6 | No |
| 85 | At | Astatine | P6 | No |
| 86 | Rn | Radon | P6 | No |
| 87 | Fr | Francium | P6 | No |
| 88 | Ra | Radium | P6 | No |
| 89 | Ac | Actinium | P6 | No |
| 91 | Pa | Protactinium | P6 | No |
| 93 | Np | Neptunium | P6 | No |
| 94 | Pu | Plutonium | P6 | No |
| 95 | Am | Americium | P6 | No |
| 96 | Cm | Curium | P6 | No |
| 97 | Bk | Berkelium | P6 | No |
| 98 | Cf | Californium | P6 | No |
| 99 | Es | Einsteinium | P6 | No |
| 100 | Fm | Fermium | P6 | No |
| 101 | Md | Mendelevium | P6 | No |
| 102 | No | Nobelium | P6 | No |
| 103 | Lr | Lawrencium | P6 | No |

### Exotic: 15 additional identities, 118 cumulative

| Z | Symbol | Element | Target phase | Legacy model documented |
| ---: | --- | --- | --- | --- |
| 104 | Rf | Rutherfordium | P8 | No |
| 105 | Db | Dubnium | P8 | No |
| 106 | Sg | Seaborgium | P8 | No |
| 107 | Bh | Bohrium | P8 | No |
| 108 | Hs | Hassium | P8 | No |
| 109 | Mt | Meitnerium | P8 | No |
| 110 | Ds | Darmstadtium | P8 | No |
| 111 | Rg | Roentgenium | P8 | No |
| 112 | Cn | Copernicium | P8 | No |
| 113 | Nh | Nihonium | P8 | No |
| 114 | Fl | Flerovium | P8 | No |
| 115 | Mc | Moscovium | P8 | No |
| 116 | Lv | Livermorium | P8 | No |
| 117 | Ts | Tennessine | P8 | No |
| 118 | Og | Oganesson | P8 | No |

<!-- END ELEMENT ROSTER -->

## What counts as supported

For the core and extended cohorts, require at least one bounded material form with source-qualified properties, validated transport/thermal behavior, explicit supported interactions, model/backend eligibility, and save/load coverage. Additional allotropes, phases, alloys, compounds, and reactions are separate capabilities and do not increase the element-identity count.

For nuclear-first entries, count only a documented nuclide representation with supported data/network closure and an honest behavior badge. Do not claim the 103 milestone means 103 validated bulk substances. For exotic entries, keep evaluated nuclide information, predicted bulk models, and fictional creative profiles visibly distinct. Missing properties may leave bulk painting unavailable in science-oriented mode.

Compounds are essential content alongside elements: H2O, CO2, selected oxides, salts/ions, silica, simple fuels, and documented mixtures. Sand is not automatically elemental silicon, alloys are not new elements, and wood/oil/smoke may remain explicitly simplified presets. [Materials plan](plans/materials-and-chemistry/plan.md).

## Legacy eight-element models

The following is the retained implementation specification from the supplied archive, with its original constants and limitations. It is not the new backend's acceptance evidence. Confirm it against the actual source at M0. The original unedited document is preserved in [the input archive](history/README.md).


Hydrogen, Helium, Carbon, Nitrogen, Oxygen, Sulfur, Iron and Copper are paintable periodic elements.
The other 110 entries are reference data and disabled. Generic sand, metal, oil,
acid and circuit presets remain separate sandbox materials. An element being
paintable does not mean every chemical reaction involving it is implemented.

## Supported behavior

| Element | Phases and properties | Explicit interactions and limits |
| --- | --- | --- |
| H₂ | Gas; molar mass 0.002016 kg/mol, specific heat 14,304 J/(kg K), density 0.08324 kg/m³ | Direct O₂ contact reacts at 500 °C or beside Fire/burning matter. Ignited H₂ also burns against air connected to an open edge; atmospheric oxygen is an accounted external source. No sealed air-mixture chemistry, cryogenic phases, ionization or dissociation. |
| He | Gas; 0.0040026 kg/mol, 5,193 J/(kg K), 0.1653 kg/m³ | Buoyancy, heat exchange and confinement pressure. Does not burn or supply oxygen. No cryogenic/superfluid model. |
| C | Graphite solid; 2,200 kg/m³ and 709 J/(kg K) | Ignited carbon consumes touching O₂ to form CO₂, with limiting-reactant and heat accounting. No CO, diamond, sublimation or reaction with ordinary air. |
| N₂ | Gas, liquid and solid; 0.028014 kg/mol; transitions −195.795 °C and −210 °C | Inert gas: carries heat and pressure but supplies no oxygen. No nitrogen fixation or ammonia chemistry. |
| O₂ | Gas, liquid and solid; 0.031998 kg/mol; transitions −182.95 °C and −218.79 °C | Oxidizer for H₂, carbon, sulfur and generic fuel. It does not burn by itself. Fire on the O₂ side of an H₂/O₂ interface heats that contact. The standard Cool tool stops at −200 °C; solid oxygen is accessible through the model API or colder matter. |
| S | Solid, liquid and vapor; 2,070 kg/m³ solid; transitions 115.21 °C and 444.61 °C | Ignited sulfur consumes touching O₂ to form SO₂. Liquid and vapor sulfur keep the same chemical store. No sulfuric acid synthesis or gas-phase allotrope equilibrium. |
| Fe | Solid, liquid and vapor; 7,870 kg/m³ solid; transitions 1538 °C and 2861 °C | Thermal conduction, latent transitions and solid-state electrical conduction. Painted structures are anchored; molten parcels freeze as movable solids. No rust, acid corrosion, alloys or magnetic model. |
| Cu | Solid, liquid and vapor; 8,960 kg/m³ solid; transitions 1084.62 °C and 2562 °C | Distinct heat capacity and lower scaled circuit resistance than iron. No oxidation, corrosion or alloy model. |

Each phase has an explicit density and constant heat capacity. Runtime enthalpy
and latent heat scale with actual parcel mass. Element transition temperatures
are normal-pressure constants; only the existing water family shifts its boiling
curve with pressure. Heat-transfer coefficients, viscosities used for damping,
cellular flow cadence and circuit resistance are model approximations. No
temperature-dependent transport data or chemical equilibrium solver is included.

## Try the interaction

Pause and clear the scene. Select Hydrogen in Elements and paint a small patch.
Apply Fire to that patch to burn hydrogen against ordinary open air. Alternatively
paint a touching Oxygen patch and apply Fire to either side of their contact, or
Heat to bring a reactant above 500 °C. The reaction creates water vapor and leaves
the excess pure reactant. The vapor can cool through the water phase model. Pure
H₂/O₂ chemistry requires orthogonal contact; a wall or diagonal-only contact
blocks it. Oxygen alone never ignites.

The reaction extent uses the molecular mass ratio
`m(O₂) / m(H₂) = 0.031998 / (2 × 0.002016)` and a finite chemical store based on
241.826 kJ per mole of H₂ consumed. Product and residual parcels share momentum;
lost kinetic energy and changed grid potential energy join their thermal budget.
This is an instantaneous reaction extent, not flame kinetics or a
prediction of adiabatic flame temperature. At high temperatures, dissociation
would matter in reality and is absent here. Reaction heat is distributed by mass,
not by an equilibrium-temperature solve across product and residual species.
The open-air path draws O₂ from an atmospheric reservoir and records incoming
mass and enthalpy in the world ledger. It is restricted to air connected to a
world edge. Carbon and sulfur currently require a separate pure O₂ parcel.

Iron and Copper can replace Wire/Metal in a Battery → conductor → Lamp → Ground
path. Heat causes melting after the entire latent interval has been supplied.
Cool reverses the transition without replacing the parcel's mass. Liquid metal
does not conduct in the current solid-only circuit solver.

## Data provenance and model limits

Element identities, normal transition temperatures, solid densities and molar
masses were checked against the Royal Society of Chemistry pages for
[Hydrogen](https://periodic-table.rsc.org/element/1/hydrogen),
[Helium](https://periodic-table.rsc.org/element/2/helium),
[Carbon](https://periodic-table.rsc.org/element/6/carbon),
[Nitrogen](https://periodic-table.rsc.org/element/7/nitrogen),
[Oxygen](https://periodic-table.rsc.org/element/8/oxygen),
[Sulfur](https://periodic-table.rsc.org/element/16/sulfur),
[Iron](https://periodic-table.rsc.org/element/26/iron) and
[Copper](https://periodic-table.rsc.org/element/29/copper).
The water-vapor reaction energy uses the standard gas formation enthalpy reported
by [NIST](https://webbook.nist.gov/cgi/cbook.cgi?Name=water&cTC=on&cTG=on).
The CO₂ and SO₂ reaction energies use standard formation enthalpies from
[NIST CO₂](https://webbook.nist.gov/cgi/cbook.cgi?ID=C124389&Mask=2189) and
[NIST SO₂](https://webbook.nist.gov/cgi/cbook.cgi?ID=C7446095&Mask=1).
These references do not validate the solver or all of its transport/phase constants;
constant heat capacities and liquid/vapor properties are approximate model inputs.

The existing grid still stores one parcel per cell. Free-expansion reference
volumes and sealed cavity volumes are inconsistent, and fluid occupancy remains
cellular rather than conservatively advected by a face velocity field. These are
open architecture items from the attached review, documented in [model.md](model.md).
The supplied documentation reports the eight models as individually specified and
regression-tested within these limits. Those tests were not rerun for this docs-only
revision; these are not complete real-world element simulations.
