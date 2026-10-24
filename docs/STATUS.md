# Hvor er vi nu?

**Status opdateret: 24. oktober 2026. Vi er i P1.**
Standardspillet bruger stadig TypeScript. GPU-demoen har endnu ingen dokumenteret
realtid eller 60 viste FPS. [Fortsæt arbejdet fra håndbogen](README.md).

| Niveau                 | Aktuelt spor                                                                                                        | Betydning                                                                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| P1 · Fase              | Flyt det eksisterende spil til Rust/GPU                                                                             | Alle nuværende materialer, værktøjer og interaktioner skal med. Fasen er ikke færdig.                           |
| M3 · Milepæl           | Opfølgning på den begrænsede GPU-scene                                                                              | Demoen har lokal GPU-evidens. Det samlede krav om realtid, billedrate og respons mangler.                       |
| E08 · Engineering-item | GPU-performance under [XER-64](https://linear.app/xerrion/issue/XER-64/run-the-experimental-gpu-scene-in-real-time) | XER-97 er lokalt implementeret og valideret. Ændringen er flyttet til en isoleret featurebranch. XER-64 er ikke afsluttet. |

P betyder fase, M betyder milepæl, og E betyder engineering-item.
[Milestenene forklaret](PHASES.md) beskriver deres indhold.

## Seneste leveringer

XER-97 vælger 16 trykiterationer pr. browserbatch. Tre parrede målinger reducerer
køretiden med en median på 11,98% mod otte iterationer. Hele felter og accepteret
fremdrift er identiske, og overførslerne er afstemt. Native builds beholder otte.
[Målinger og afgrænsning](validation/p1-m3-e08.md#coarse-pressure-completion-batching-on-24-october-2026).
Fuld lokal CI, tre native GPU-regressioner og den krævede vedvarende browsertest
består. Featurebranchen `perf/xer-97-pressure-completion-batches` er oprettet i et separat
worktree på `E:/Projects/particle-foundry-xer-97`. Den verificerede patch er overført.
Det delte checkout på `main` og dets eksisterende ændringer er bevaret.
Git-beskyttelsen er stadig slået til. Den blokerede det første forsøg med
`git switch`, men tillader `git worktree add`.
Se [XER-97](https://linear.app/xerrion/issue/XER-97) for aktuel PR- og reviewstatus.
Dette er ikke en merge- eller deploymentbekræftelse.

1. Profilering i [PR #31](https://github.com/Xerrion/particle-foundry/pull/31) er merged den 15. oktober ved `3257888`. [XER-80](https://linear.app/xerrion/issue/XER-80/profile-pressure-convergence-and-gpu-completion-timing) er Done. Værktøjerne viser trykkonvergens, GPU-timing og browserspor. De dokumenterer ingen ny hastighedsforbedring. [Resultater og begrænsninger](validation/p1-m3-e08.md#pressure-convergence-and-completion-profiling-on-14-october-2026).
2. Brandrettelsen for træ og olie i [PR #32](https://github.com/Xerrion/particle-foundry/pull/32) er merged ved `a8d9cc2`. [XER-99](https://linear.app/xerrion/issue/XER-99/restore-playable-wood-and-oil-burning-in-the-current-sandbox) er Done. Rettelsen gælder TypeScript-spillet. Almindelig spilafprøvning mangler, og Rust/GPU-brand i E11 er stadig planlagt. [Metode og evidens](combustion.md#legacy-woodoil-tempo-checkpoint-on-14-october-2026).
3. RTX-baseline og krævede GPU-checks er dokumenteret i den mergede [PR #30](https://github.com/Xerrion/particle-foundry/pull/30). Vandpenslens sammenhængende streger er merged i [PR #28](https://github.com/Xerrion/particle-foundry/pull/28). [GPU-evidensen](validation/p1-m3-e08.md) ejer målinger og testopsætning.

Merged kode, lokale målinger og verificeret drift er forskellige leveringsstadier.
De nye browsermålinger kvalificerer hverken realtid, 60 viste FPS eller offentlig drift.

## Næste beslutning i GPU-sporet

Den lokale implementering og validering af XER-97 er færdig.
Leveringen fortsætter fra det separate worktree.
[XER-97](https://linear.app/xerrion/issue/XER-97) viser aktuel leverings- og reviewstatus.
XER-96 om viste billeder og respons er næste måleopgave, men er ikke valgt.
XER-98 afventer den endelige kvalifikation. Ingen af opgaverne startes her.

Slutmålet er hele det eksisterende spil med 60 viste FPS, simulering i realtid,
hurtig betjening, fysisk troværdig adfærd og spilbart brandtempo.
[Det fælles P1-krav](plans/fluid-gpu-redesign/plan.md#final-gameplay-target-clarified-on-13-october-2026)
ejer accepten. Den begrænsede demo kan ikke afslutte hele migrationen.

## Hvorfor siger planen M4/E09?

P1/M4/E09 er manifesternes næste migrationsitem. Det dækker varme, termodynamik
og faseskift. [XER-33](https://linear.app/xerrion/issue/XER-33/add-full-supported-thermodynamics-and-phase-closure)
er In Progress. En [afgrænset Rust-reference](thermodynamics.md) findes.
Hele opgaven er ikke færdig. M1/E04 om elektriske kredsløb mangler også.
Det valgte GPU-spor og planens rækkefølge kan være forskellige.

[Håndbogen](README.md) er startpunktet. [Fasemanifestet](data/development-phases.json)
og [engineering-manifestet](data/engine-migration-work.json) ejer planens rækkefølge.
[Dokumentationsguiden](documentation.md) beskriver, hvornår denne status skal opdateres.
