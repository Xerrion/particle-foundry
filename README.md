# Particle Foundry

En browserbaseret falling-sand-sandkasse med fysik, varmeledning, faseskift,
væsketransport, idealgastryk og begrænset forbrænding. Standardverdenen er et
gitter på 480 × 270 celler med zoom og panorering.

**Fortsæt projektarbejde fra [håndbogen](docs/README.md).** Den samler aktuel
status, opgavevalg, tekniske referencer og testkrav.

## Kør lokalt

Installer [mise](https://mise.jdx.dev/getting-started.html) og
[Rustup](https://rustup.rs/). Kør fra projektets rod:

```sh
mise trust
mise install
mise run setup
mise run dev
```

`mise.toml` ejer værktøjsversioner og kommandoer. JavaScript-afhængigheder kommer
fra `web/bun.lock`. Rust-værktøjer og afhængigheder følger `engine/`.
Setup installerer også dokumentationens renderer i ignoreret `.venv/`.

| Kommando              | Formål                                                            |
| --------------------- | ----------------------------------------------------------------- |
| `mise run dev`        | Start spillet med live reload.                                    |
| `mise run build`      | Byg produktionsudgaven i `web/dist/`.                             |
| `mise run ci`         | Kør projektets samlede lokale kontrol.                            |
| `mise run docs:check` | Kontrollér dokumentation, links, manifester og historisk evidens. |
| `mise tasks`          | Vis alle tilgængelige opgaver.                                    |

## Læs dokumentationen i browseren

```sh
mise run docs:build
mise run docs:serve
```

Åbn <http://127.0.0.1:8000>. Hele håndbogen ligger også som én HTML-side i
`artifacts/docs/index.html`. Den kan åbnes direkte uden en server.
Kopiér hele `artifacts/docs/` for at medtage tilknyttede kildefiler og evidens.
[Dokumentationsguiden](docs/documentation.md) beskriver indhold og vedligeholdelse.

## Aktuel afgrænsning

Det normale spil bruger fortsat TypeScript-motoren. `/gpu.html` viser en
begrænset Rust/GPU-scene. P1 skal flytte hele det eksisterende spil til Rust/GPU
med 60 viste FPS, simulering i realtid og fysisk troværdig adfærd.
Det er et acceptkrav, som den eksperimentelle scene endnu ikke opfylder.

[Håndbogen](docs/README.md) viser status og de styrende krav.
[Projektstrukturen](docs/project-structure.md) beskriver kodeejerskab.
[CI-politikken](.github/README.md) beskriver mergekrav, og
[deployguiden](docs/deployment.md) beskriver Coolify.
