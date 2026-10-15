# Hvor er vi nu?

**Opdateret: 14. oktober 2026. Vi er i P1. Profileringsleveringen i PR #31 er samlet med seneste `main` og består lokal CI. Brandrettelsen for træ og olie er merged. GPU-scenen har stadig ingen dokumenteret realtid eller 60 viste FPS.**

| Niveau | Hvor vi er | Hvad det betyder |
| --- | --- | --- |
| **P1 · Fase** | Flyt det eksisterende sandbox-spil til Rust/GPU | Alle nuværende materialer, værktøjer og interaktioner skal med. Fasen er ikke færdig. |
| **M3 · Milepæl** | Opfølgning på den lille GPU-scene | Den oprindelige demo er valideret lokalt. Realtid og 60 viste FPS mangler i `/gpu.html`. |
| **E08 / E11 · Engineering-item** | GPU-profilering og kommende brandmigration | [XER-80](https://linear.app/xerrion/issue/XER-80/profile-pressure-convergence-and-gpu-completion-timing) er til review under [XER-64](https://linear.app/xerrion/issue/XER-64/run-the-experimental-gpu-scene-in-real-time). [XER-99](https://linear.app/xerrion/issue/XER-99/restore-playable-wood-and-oil-burning-in-the-current-sandbox) er Done; rettelsen gælder TypeScript-spillet og afslutter ikke E11. |

P, M og E er projektets egne betegnelser for fase, milepæl og konkret udviklingsopgave. De er ikke Linear-statusser.

## Senest færdigt

- Brandrettelsen i [PR #32](https://github.com/Xerrion/particle-foundry/pull/32) er merged på `main` ved `a8d9cc2`. Små, faste testscener forbruger 95 % af brændstoffet på **5,12 simulerede sekunder for træ og 17,22 for olie**. Før rettelsen var kun 13,94 % og 5,31 % forbrugt efter 60 sekunder. Den består samlet lokal CI med 522 TypeScript-tests. [Metode, tests og begrænsninger](combustion.md#legacy-woodoil-tempo-checkpoint-on-14-october-2026). Afprøvning i almindeligt spil mangler stadig.
- [RTX-verifikation og frisk baseline](validation/p1-m3-e08.md#rtx-qualification-and-baseline-after-reboot-on-14-october-2026) er dokumenteret i den mergede [PR #30](https://github.com/Xerrion/particle-foundry/pull/30), `f47e2eb`. Genstart løste driverkonflikten. De krævede GPU-tests og de numeriske checks består med den dokumenterede Linux/NVIDIA-testopsætning.
- [PR #28](https://github.com/Xerrion/particle-foundry/pull/28) er merged. Vandpenslen tegner sammenhængende streger. [PR #27](https://github.com/Xerrion/particle-foundry/pull/27) er også merged; undersøgelsen fandt ikke transportområder, som sikkert kunne springes over.

[Acceptkravet om spilbart brandtempo](plans/fire-combustion/plan.md#playable-wood-and-oil-burn-tempo) er skrevet ind i E11 og Linear. Rust/GPU-brand og de korrigerede brandgates er stadig planlagt.

## Profilering og næste skridt

Profileringsværktøjerne i [PR #31](https://github.com/Xerrion/particle-foundry/pull/31) er tidligere verificeret med lokal CI, krævede RTX-browserchecks og uafhængig kodegennemgang. Med værktøjerne slået fra matcher felter og tællere kontrollen; aktiv profilering tilføjer cirka 5 % i den målte kørsel. [Profileringsresultater og begrænsninger](validation/p1-m3-e08.md#pressure-convergence-and-completion-profiling-on-14-october-2026) peger på at afprøve færre statusaflæsninger fra GPU'en. **Ingen ny optimering af trykløseren eller transporten er valgt.** PR'en er åben og ikke merged. [Integrationen med den mergede brandrettelse](validation/p1-m3-e08.md#profiling-integration-with-main-on-14-october-2026) består fuld lokal CI med 527 TypeScript-tests. Næste skridt er review; GitHub-checks følges på PR'en.

Den friske RTX-baseline fra `427f162` bruger i gennemsnit **19,12 virkelige sekunder på ét simulationssekund** i tre korte kørsler. Den længere kørsel simulerer **fem sekunder på 49,94 sekunder**. Renderingsrytmen er **6,68 indsendelser pr. sekund**, hvilket ikke måler viste FPS. Profileringen dokumenterer ingen ny hastighedsforbedring. [Målemetode, tal og begrænsninger](validation/p1-m3-e08.md#rtx-qualification-and-baseline-after-reboot-on-14-october-2026).

XER-64 er fortsat igangværende. Arbejdet er fordelt på 19 sub-issues: XER-81 til XER-95 er 15 afsluttede opgaver, XER-80 er til review, og XER-96 til XER-98 ligger i backloggen. Beskrivelserne indeholder opgaverne; fremdrift og resultater ligger i kommentarer. [Fordeling og ansvar](remaining-work.md#xer-64-work-split-on-14-october-2026).

Slutmålet er hele det eksisterende sandbox-spil med **60 viste FPS, simulering i realtid, hurtig betjening, fysisk troværdig adfærd og spilbart brandtempo**. Det normale spil bruger fortsat TypeScript-motoren. Den lille GPU-demo er kun en del af målet.

## Hvorfor siger planen M4/E09?

**P1/M4/E09 er migrationsplanens næste item.** Det dækker varme, termodynamik og faseskift og er påbegyndt i [XER-33](https://linear.app/xerrion/issue/XER-33/add-full-supported-thermodynamics-and-phase-closure). En afgrænset Rust-reference findes; hele opgaven er ikke færdig. M1/E04 om elektriske kredsløb mangler også. Planens rækkefølge og det valgte arbejdsspor kan være forskellige.

## Sådan holdes status ajour

Opdater denne side i samme ændring, når vi vælger et nyt arbejdsspor, afslutter eller merger et item, ændrer en blokering eller verificerer drift. Ret dato, P/M/E, senest færdigt og næste skridt. Kontrollér live Linear-status og de relevante testresultater først. Siden er et læsevenligt overblik; den ændrer ikke planens acceptkrav.

Kilder: [aktivt arbejde](remaining-work.md), [fasernes rækkefølge](data/development-phases.json), [engineering-items](data/engine-migration-work.json) og [forklaring af milepælene](PHASES.md).
