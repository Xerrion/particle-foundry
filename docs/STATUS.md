# Hvor er vi nu?

**Opdateret: 14. oktober 2026. Vi er i P1. Den valgte opgave er nu at rette det langsomme brandforløb for træ og olie i det nuværende TypeScript-spil, fulgt i XER-99. GPU-sporets realtidsmål er stadig ikke opfyldt.**

| Niveau | Hvor vi er | Hvad det betyder |
| --- | --- | --- |
| **P1 · Fase** | Flyt det eksisterende sandbox-spil til Rust/GPU | Alle nuværende materialer, værktøjer og interaktioner skal med. Fasen er ikke færdig. |
| **M3 · Milepæl** | Opfølgning på den lille GPU-scene | Den oprindelige demo er valideret lokalt. Vi arbejder videre med brugbarhed og hastighed i `/gpu.html`. |
| **E08 / E11 · Engineering-item** | GPU-performance og kommende brandmigration | [XER-64](https://linear.app/xerrion/issue/XER-64/run-the-experimental-gpu-scene-in-real-time) følger GPU-målet. Den valgte rettelse i [XER-99](https://linear.app/xerrion/issue/XER-99/restore-playable-wood-and-oil-burning-in-the-current-sandbox) gælder TypeScript-spillet og afslutter ikke E11. |

P, M og E er projektets egne betegnelser for fase, milepæl og konkret udviklingsopgave. De er ikke Linear-statusser.

## Senest færdigt

- [RTX-verifikation og frisk baseline](validation/p1-m3-e08.md#rtx-qualification-and-baseline-after-reboot-on-14-october-2026) er dokumenteret i den mergede [PR #30](https://github.com/Xerrion/particle-foundry/pull/30), `f47e2eb`. Genstart løste driverkonflikten. De krævede GPU-tests og de numeriske checks består med den dokumenterede Linux/NVIDIA-testopsætning.
- GPU-profilering er leveret i [draft PR #31](https://github.com/Xerrion/particle-foundry/pull/31) og følges i [XER-80](https://linear.app/xerrion/issue/XER-80). PR'en er ikke merged. Profilering giver måleværktøjer; den dokumenterer ingen ny hastighedsforbedring.
- [PR #28](https://github.com/Xerrion/particle-foundry/pull/28) er merged. Vandpenslen tegner nu sammenhængende streger, så separate penselaftryk ikke skaber det gentagne tandmønster.
- [PR #27](https://github.com/Xerrion/particle-foundry/pull/27) er merged. Undersøgelsen fandt ikke områder, som sikkert kunne springes over i den afprøvede væsketransport.

Rettelsen giver ingen påvist forbedring af simulationshastigheden. **60 viste FPS og simulering i realtid er stadig ikke opnået.** Merge, lokal test og verificeret drift er forskellige ting; den seneste rettelse er endnu ikke verificeret i offentlig drift. [Målinger og testresultater](validation/p1-m3-e08.md#continuous-gpu-brush-strokes-on-14-october-2026).

## Hvorfor siger planen M4/E09?

**P1/M4/E09 er migrationsplanens næste item.** Det dækker varme, termodynamik og faseskift og er allerede påbegyndt i [XER-33](https://linear.app/xerrion/issue/XER-33/add-full-supported-thermodynamics-and-phase-closure). En afgrænset Rust-reference findes; hele opgaven er ikke færdig. Performanceopfølgningen på M3/E08 har fået prioritet, før vi fortsætter dette spor.

Planens rækkefølge og det valgte arbejdsspor kan derfor være forskellige. M1/E04 om elektriske kredsløb mangler også. Vi har ikke afsluttet alt før M4.

## Næste skridt

Træ og olie går næsten i stå ved brandmodellens temperaturloft. Den lokale
rettelse fører overskydende varme ud gennem den eksisterende luftmodel og
bogfører energitabet. I små, faste testscener forbruges 95% af brændstoffet på
5,12 simulerede sekunder for træ og 17,22 for olie. Før rettelsen var kun 13,94%
og 5,31% forbrugt efter 60 sekunder. [Metode, tests og begrænsninger](combustion.md#legacy-woodoil-tempo-checkpoint-on-14-october-2026).

Rettelsen består samlet lokal CI, herunder 522 TypeScript-tests, og er leveret
på en separat featurebranch til draft PR og review via XER-99. Den er ikke
merged. Næste skridt er review og afprøvning af brandtempoet i almindeligt spil.
[Acceptkravet om spilbart brandtempo](plans/fire-combustion/plan.md#playable-wood-and-oil-burn-tempo)
er også skrevet ind i E11 og Linear. Rust/GPU-brand er stadig planlagt.

Den aftalte RTX-baseline for XER-64 er målt fra daværende `main`, `427f162`, uden nye ændringer i simuleringen. Ét sekund i simuleringen tager i gennemsnit **19,12 sekunder** i tre korte kørsler. Den længere kørsel simulerer **fem sekunder på 49,94 sekunder**. Renderingsrytmen er **6,68 indsendelser pr. sekund**. Dette er ikke en måling af viste FPS. [Målemetode, tal og begrænsninger](validation/p1-m3-e08.md#rtx-qualification-and-baseline-after-reboot-on-14-october-2026) er gemt.

XER-64 er fortsat igangværende, fordi realtid og 60 viste FPS mangler. Baseline og profilering giver udgangspunktet for næste valgte performancearbejde.

Der er ikke startet en ny optimering. Forslaget om trykløserens arbejde og ventetiden på GPU-resultater kræver fortsat, at vi vælger det som næste opgave.

Slutmålet er hele det eksisterende sandbox-spil med **60 viste FPS, simulering i realtid, hurtig betjening og fysisk troværdig adfærd**. Den lille GPU-demo er kun en del af det. Det normale spil bruger fortsat den eksisterende TypeScript-motor.

## Sådan holdes status ajour

Opdater denne side i samme ændring, når vi vælger et nyt arbejdsspor, afslutter eller merger et item, ændrer en blokering eller verificerer drift. Ret dato, P/M/E, senest færdigt og næste skridt. Kontrollér live Linear-status og de relevante testresultater først. Siden er et læsevenligt overblik; den ændrer ikke planens acceptkrav.

Kilder: [aktivt arbejde](remaining-work.md), [fasernes rækkefølge](data/development-phases.json), [engineering-items](data/engine-migration-work.json) og [forklaring af milepælene](PHASES.md).
