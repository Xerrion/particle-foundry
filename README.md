# Particle Foundry

En browserbaseret falling-sand-sandkasse med reproducerbar fysik, varmeledning,
faseændringer, væsketransport, idealgastryk, bevægelige faste parceller og begrænset
forbrænding. Verden er som standard et gitter på 480 × 270 celler med zoom og panorering.

Se [faser og milepæle i almindeligt sprog](docs/PHASES.md) for projektets retning.
Læs [modelspecifikationen](docs/model.md) for enheder, tilstandsejerskab,
bevarelsesregler og modellens bevidste grænser. [Restarbejdet](docs/remaining-work.md)
beskriver aktiv backlog og leveret historik; [roadmap](docs/roadmap.md) viser
rækkefølgen for GPU-redesign og den planlagte opfølgning med trykbølger og brud.

## Kør lokalt

Installer [mise](https://mise.jdx.dev/getting-started.html) og
[Rustup](https://rustup.rs/), og kør fra projektets rod:

```sh
mise trust
mise install
mise run setup
mise run dev
```

`mise.toml` samler fastlåste versioner af Bun, Node.js og Python samt alle
udviklingskommandoer. JavaScript-værktøjerne kommer fra `web/bun.lock`, og dokumentationens
Markdown-renderer installeres i projektets ignorerede `.venv/`. Node.js bruges af
Vite og TypeScript; Bun bruges til afhængigheder, tests og benchmarks.
`engine/rust-toolchain.toml` fastlåser Rust og WASM-target, og `engine/Cargo.lock`
låser motorens afhængigheder. Setup installerer den tilhørende wasm-bindgen CLI;
build og typekontrol genererer bindings i ignoreret `web/generated/wasm/`.

| Kommando | Formål |
| --- | --- |
| `mise run setup` | Installer appafhængigheder, dokumentationens renderer og Rust/WASM-værktøjer. |
| `mise run dev` | Start Vite med live reload. |
| `mise run lint` | Kør Biome formatterings- og lintkontrol. |
| `mise run format` | Formatér kode og konfiguration med Biome. |
| `mise run typecheck` | Kør streng TypeScript-kontrol. |
| `mise run typecheck:catalogue` | Kontrollér ID-typer og katalog med `noUncheckedIndexedAccess`. |
| `mise run test` | Kør alle modul- og integrationstests. |
| `mise run check` | Kør begge typekontroller og tests. |
| `mise run rust:test` | Kør Rust-workspacets native tests. |
| `mise run rust:clippy` | Kontrollér Rust-koden med Clippy. |
| `mise run build:wasm` | Byg WASM og generér JavaScript/TypeScript-bindings. |
| `mise run check:wasm` | Sammenlign genererede filer byte for byte med et nyt build. |
| `mise run test:browser` | Test WASM-livscyklus og tilgængelig WebGPU i en isoleret browser. |
| `mise run build` | Byg produktionsudgaven i `web/dist/`. |
| `mise run bench` | Mål 240 × 135-belastninger og 480 × 270-startscenen mod 60 Hz-budgettet. |
| `mise run preview` | Byg og servér produktionsudgaven lokalt. |
| `mise run docs:check` | Kontrollér dokumentation, links og historisk evidens. |
| `mise run docs:render` | Generér GPU-planens HTML fra Markdown. |
| `mise run ci` | Kør TypeScript/Rust-checks, tests, WASM-build/freshness, browser-smoke og docs. |

GitHub Actions kører `mise run ci` ved push og pull requests med samme versioner og
kommandoer som lokalt. `mise tasks` viser alle opgaver. Opgaverne installerer selv
deres afhængigheder; ekstra argumenter sendes videre, f.eks.
`mise run dev --host 127.0.0.1`.

Browser-smoke bruger Chrome; `CHROME_BIN` kan vælge en anden executable. Kør
`mise run test:browser artifacts/validation/browser/nyt-run --require-gpu` for
at kræve en GPU-adapter. En manglende adapter rapporteres særskilt og tæller ikke
som en GPU-test, der er bestået. Se [genoptagelse af Rust-porten](docs/RESUME.md).

## Projektstruktur

| Sti | Ansvar |
| --- | --- |
| `web/` | TypeScript-app, HTML/CSS, tests, benchmarks og frontendkonfiguration |
| `engine/` | Rust/WASM-kontrakter, eksperimentel GPU-simulering og WGSL-shaders |
| `docs/` | Aktuel model, arkitektur, migrationsplaner og historisk evidens |
| `mise.toml` | Fælles værktøjsversioner og opgaver; appopgaver kører automatisk i `web/` |
| [AGENTS.md](AGENTS.md) | Projektets regler for ejerskab, imports, tests og dokumentation |

Start i [projektets vidensbase](docs/README.md). Se
[betjening og eksisterende implementering](docs/legacy-sandbox.md),
[projektstrukturen](docs/project-structure.md) og
[Rust-motorens placering](docs/project-structure.md#rust-workspace).

Den kørende motor er samlet i `web/src/legacy/`. Browserens kontroller bruger
`web/src/engine-client/`. E01-bootstrap og E02-kontrakter er implementeret.
En eksperimentel GPU-scene med Rust/WGSL-simulering og rendering kan prøves på
`/gpu.html`; en lokal browserprøve på 60 trin og ét sekund modeltid er bestået.
Scenen er stadig en begrænset prøve, og standardscenen bruger fortsat
legacy-motoren.
Lokale profiler, logs, browseroptagelser og snapshots ligger i ignoreret `artifacts/`.
