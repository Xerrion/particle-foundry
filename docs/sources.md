# Sources, provenance, and interpretation

**Reference review date:** 21 September 2026. Links below were used to verify the new scientific/API framing. They do not establish that Particle Foundry implements or validates any corresponding behavior. New implementation constants must be imported or transcribed with their own exact dataset/version, uncertainty, units, and validity range.

The proposed cohorts, architecture, milestones, UI, and acceptance process are design decisions. They are not statements from the cited institutions. The complete previous documentation and its historical references are preserved under [history](history/README.md).

## iupac-periodic-table

**IUPAC: periodic table of the elements.** [Official reference](https://iupac.org/what-we-do/periodic-table-of-elements/).

Use for element names, symbols, atomic numbers, and distinctions between standard atomic weights and specific isotope mass numbers. The project scope is identities Z=1..118. Do not interpret a bracketed mass number as a natural isotope mixture, or a standard atomic weight as the mass of one isotope. Cohort placement and runtime material IDs are separate project choices.

## rsc-carbon

**Royal Society of Chemistry: carbon.** [Element overview](https://periodic-table.rsc.org/element/6/carbon).

Supports distinguishing carbon forms such as graphite and diamond. This motivates separate material forms; it does not supply every constitutive model, transformation rate, or pressure/temperature domain needed by the game.

## rsc-phosphorus

**Royal Society of Chemistry: phosphorus.** [Element overview](https://periodic-table.rsc.org/element/15/phosphorus).

Supports distinct phosphorus forms rather than one universal physical/reactive material. Use separate property and reaction records when implementing each form. No preparation procedure is part of this documentation plan.

## rsc-gallium

**Royal Society of Chemistry: gallium.** [Element overview](https://periodic-table.rsc.org/element/31/gallium).

Provides the near-room-temperature melting-point example used to prioritize a phase-change fixture. It is an overview reference, not validation of a GPU phase solver or an unrestricted thermodynamic table.

## rsc-oganesson

**Royal Society of Chemistry: oganesson.** [Element overview](https://periodic-table.rsc.org/element/118/oganesson).

Lists important bulk properties, including density and phase-transition temperatures, as unknown. Do not convert an overview's qualitative or predicted state into measured bulk data. Use evaluated nuclide sources, not the overview's rounded nuclear entries, for decay behavior.

## nist-webbook

**NIST Chemistry WebBook, Standard Reference Database 69.** [Database](https://webbook.nist.gov/chemistry/).

Use source-specific thermochemical and selected physical-property records for defined species. Preserve state, units, reference conditions, underlying reference, and uncertainty. A formation-enthalpy record does not define reaction kinetics or make a mixture model valid. Existing water/CO2/SO2 references remain in [elements.md](elements.md).

## nndc-nudat

**National Nuclear Data Center: NuDat 3 and nuclear databases.** [NuDat](https://www.nndc.bnl.gov/nudat3/), [database descriptions](https://www.nndc.bnl.gov/databases/).

Use evaluated nuclear structure/decay information for specific nuclides and states. Half-lives, emissions, branches, and state identity belong here rather than in an element-level material flag. Pin the actual evaluation and imported subset at implementation time.

## iaea-livechart

**IAEA Nuclear Data Services: LiveChart API guide.** [API/data guide](https://www-nds.iaea.org/relnsd/vcharthtml/api_v0_guide.html).

Provides structured access to nuclide, decay-radiation, and selected fission-yield data. Inspect column definitions and units; distinguish independent and cumulative yields and carry the provenance of the underlying evaluation. Fetch during reproducible data preparation, not during gameplay. No fixed API result or lifetime constant is embedded by this documentation update.

## nndc-endf

**NNDC: Evaluated Nuclear Data File.** [ENDF access](https://www.nndc.bnl.gov/endf/).

Use evaluated reaction information for the explicitly selected nuclides/channels and energy domains. Loading a table alone does not create a validated transport model. Preserve data processing, group/energy assumptions, and library version. Nuclear structure, decay data, and reaction cross sections are not interchangeable data products.

## iter-fusion

**ITER: what is fusion?** [Conceptual reference](https://www.iter.org/fusion-energy/what-fusion).

Supports the introductory distinction between hydrogen isotopes and elements and the deuterium/tritium reaction products. It is not a source for the game's rate tables, magnetic-confinement implementation, or plasma equation of state. Those are separate implementation requirements.

## wgsl

**W3C: WebGPU Shading Language.** [Specification](https://www.w3.org/TR/WGSL/).

Use the implementation-supported specification for data layout, numeric types, atomics, and synchronization. This plan selects a portable f32/integer baseline; optional types/features are not correctness dependencies. A workgroup barrier is not a cross-workgroup/global barrier. Record the actual browser/compiler feature set during GPU validation.

## webgpu-mapping

**MDN Web API documentation: GPUBuffer.mapAsync.** [API reference](https://developer.mozilla.org/en-US/docs/Web/API/GPUBuffer/mapAsync).

Mapped buffers are unavailable to GPU commands until unmapped. The plan therefore uses separate staging buffers and asynchronous tick-stamped probes rather than synchronous reads of live state. The API reference does not guarantee adapter availability or throughput on the target device.

## webgpu-limits

**MDN Web API documentation: GPUSupportedLimits.** [API reference](https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits).

Query and request the actual device limits; do not assume one developer GPU's resource capacity is portable. Record buffers, bindings, workgroup sizes, staging resources, and active-component counts in the allocation manifest.

## basilisk-mac-vof

**Basilisk: staggered MAC and VOF implementation references.** [MAC](https://basilisk.fr/src/navier-stokes/mac.h), [VOF](https://basilisk.fr/src/vof.h).

References for compatible staggered operators, interface transport, and phase-associated tracers. The GPU plan's equations and tests are project requirements, not a claim to copy or reproduce Basilisk's complete solver. Review licenses before reusing code. Scheme-specific CFL limits must be re-established for the chosen discretisation and source terms.

## clawpack-euler

**Clawpack Riemann book: approximate Euler solvers.** [Numerical reference](https://www.clawpack.org/riemann_book/html/Euler_approximate.html).

Reference for the separate compressible-gas work. Positivity of a low-order flux alone does not guarantee the entire reconstructed/source-coupled scheme is positive. Limit reconstruction, validate the complete update, and retry failed steps.

## water-and-existing-references

The original GPU plan references [IAPWS IF97](https://iapws.org/technical-guidance/release/IF97-Rev) for a deliberately bounded water-property model. That existing choice remains provisional and must be recorded at M4. No IF97 table was generated or numerically checked in this documentation revision. Existing fracture and other references retained inside the supplied plans likewise are not new measurements.

## Data import rules

Record source URL or DOI, publisher, evaluation/version/date, retrieval date, license/redistribution status, checksum, units, reference states, uncertainty, valid domain, transformation script/version, and tests of sampled imported values. Keep raw input distinct from derived runtime tables. Record disagreements rather than silently choosing whichever value makes a fixture pass.

Use a property-level status of `measured`, `evaluated`, `predicted`, `gameplay-override`, or `unknown`. These are proposed application labels, not a promise of scientific certainty. A missing required datum blocks that capability; it does not require removing the element identity from the reference catalogue.

## wgpu-platforms

**wgpu project: portable graphics library.** [Official overview](https://wgpu.rs/).

Reviewed for this Rust/WASM revision. Documents native graphics backends and browser WebGPU through WASM, with WGSL shaders. Supports the portability rationale, not a guarantee of faster identical shader execution than TypeScript submission.

## wgpu-web

**wgpu: running on the web.** [Official web platform guide](https://wgpu.rs/doc/wgpu/documentation/platforms/web/index.html).

Documents the WASM target, browser execution, bindings generation and secure-context requirements. Pin and test a compatible dependency/CLI toolchain in M1 rather than copy a floating latest version. The examples are wgpu's examples, not existing Particle Foundry build scripts.

## wgpu-buffers

**wgpu: Buffer.** [Official API documentation](https://wgpu.rs/doc/wgpu/struct.Buffer.html).

Documents GPU buffers, asynchronous mapping, mapped-versus-GPU-use exclusivity and potential browser/WASM copies. Motivates persistent GPU state, staging readbacks and explicit completion handling. It does not imply zero-copy access from Rust or a currently synchronized CPU world.

## wgpu-downlevel

**wgpu: DownlevelFlags, COMPUTE_SHADERS.** [Official capability reference](https://wgpu.rs/doc/wgpu/struct.DownlevelFlags.html#associatedconstant.COMPUTE_SHADERS).

Explicitly distinguishes compute support from WebGL2/GLES 3.0. WebGL2 is not a transparent fallback for the selected compute solver. A CPU fallback is a separate implementation with its own supported-scene requirements.

## wgpu-limits

**wgpu: Limits.** [Official resource-limit reference](https://wgpu.rs/doc/wgpu/struct.Limits.html).

Use actual queried/requested limits, binding layouts and allocation results. Published defaults do not justify assuming every optional feature or unlimited memory. Record active-set and scratch/staging requirements; allocation may fail even below a maximum size.

## wasm-bindgen-async

**wasm-bindgen project: Promises and Futures.** [Official guide](https://wasm-bindgen.github.io/wasm-bindgen/reference/js-promises-and-rust-futures.html).

Documents Rust async exports returning JavaScript Promises. Supports the proposed initialization/probe/snapshot API. The engine's command sequencing, backpressure and state ownership are project design requirements, not supplied by a Promise alone.

## html-canvas-context

**WHATWG HTML standard: canvas.** [Official canvas interface](https://html.spec.whatwg.org/dev/canvas.html).

Specifies that acquiring a different context type after canvas initialization returns null. The existing Canvas-2D-first startup therefore needs renderer selection before context acquisition or a new canvas for the GPU path. The standard does not supply application event rebinding or world migration.
