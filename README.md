# Particle Foundry

En browserbaseret falling-sand-sandkasse med reproducerbar fysik, varmeledning,
faseændringer, væsketransport, idealgastryk, bevægelige faste parceller og begrænset
forbrænding. Verden er som standard et gitter på 480 × 270 celler med zoom og panorering.

Læs [modelspecifikationen](docs/model.md) for enheder, tilstandsejerskab,
bevarelsesregler og modellens bevidste grænser. [Restarbejdet](docs/remaining-work.md)
beskriver aktiv backlog og leveret historik; [roadmap](docs/roadmap.md) viser
rækkefølgen for GPU-redesign og den planlagte opfølgning med trykbølger og brud.

## Kør lokalt

Installer [mise](https://mise.jdx.dev/getting-started.html), og kør fra projektets rod:

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

| Kommando | Formål |
| --- | --- |
| `mise run setup` | Installer låste appafhængigheder og dokumentationens renderer. |
| `mise run dev` | Start Vite med live reload. |
| `mise run lint` | Kør Biome formatterings- og lintkontrol. |
| `mise run format` | Formatér kode og konfiguration med Biome. |
| `mise run typecheck` | Kør streng TypeScript-kontrol. |
| `mise run typecheck:catalogue` | Kontrollér ID-typer og katalog med `noUncheckedIndexedAccess`. |
| `mise run test` | Kør alle modul- og integrationstests. |
| `mise run check` | Kør begge typekontroller og tests. |
| `mise run build` | Byg produktionsudgaven i `web/dist/`. |
| `mise run bench` | Mål 240 × 135-belastninger og 480 × 270-startscenen mod 60 Hz-budgettet. |
| `mise run preview` | Byg og servér produktionsudgaven lokalt. |
| `mise run docs:check` | Kontrollér dokumentation, links og historisk evidens. |
| `mise run docs:render` | Generér GPU-planens HTML fra Markdown. |
| `mise run ci` | Kør lint, typekontroller, tests, build og dokumentationskontrol. |

GitHub Actions kører `mise run ci` ved push og pull requests med samme versioner og
kommandoer som lokalt. `mise tasks` viser alle opgaver. Opgaverne installerer selv
deres afhængigheder; ekstra argumenter sendes videre, f.eks.
`mise run dev --host 127.0.0.1`.

## Projektstruktur

| Sti | Ansvar |
| --- | --- |
| `web/` | TypeScript-app, HTML/CSS, tests, benchmarks og frontendkonfiguration |
| `engine/` | Placering for den planlagte Rust/WASM-motor og WGSL-shaders |
| `docs/` | Aktuel model, arkitektur, migrationsplaner og historisk evidens |
| `mise.toml` | Fælles værktøjsversioner og opgaver; appopgaver kører automatisk i `web/` |
| [AGENTS.md](AGENTS.md) | Projektets regler for ejerskab, imports, tests og dokumentation |

Start i [projektets vidensbase](docs/README.md). Se
[betjening og eksisterende implementering](docs/legacy-sandbox.md),
[projektstrukturen](docs/project-structure.md) og
[Rust-motorens placering](docs/project-structure.md#planned-rust-workspace).

Den kørende motor er samlet i `web/src/legacy/`. Browserens kontroller bruger
`web/src/engine-client/`; Rust-crates og WASM-bindings er endnu ikke implementeret.
Lokale profiler, logs, browseroptagelser og snapshots ligger i ignoreret `artifacts/`.
