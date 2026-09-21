# Individual element models

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
The eight models are individually specified and regression-tested within these
limits; they are not complete real-world element simulations.
