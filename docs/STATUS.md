# Hvor er vi nu?

**Opdateret: 14. oktober 2026. Vi er i P1. Det prioriterede arbejdsspor er opfølgning på M3/E08: få GPU-scenen til at fungere godt i realtid.**

| Niveau | Hvor vi er | Hvad det betyder |
| --- | --- | --- |
| **P1 · Fase** | Flyt det eksisterende sandbox-spil til Rust/GPU | Alle nuværende materialer, værktøjer og interaktioner skal med. Fasen er ikke færdig. |
| **M3 · Milepæl** | Opfølgning på den lille GPU-scene | Den oprindelige demo er valideret lokalt. Vi arbejder videre med brugbarhed og hastighed i `/gpu.html`. |
| **E08 · Engineering-item** | Opfølgning på browserens GPU-visning og betjening | Den oprindelige opgave er valideret. Det aktuelle performancearbejde følges i [XER-64](https://linear.app/xerrion/issue/XER-64/run-the-experimental-gpu-scene-in-real-time). |

P, M og E er projektets egne betegnelser for fase, milepæl og konkret udviklingsopgave. De er ikke Linear-statusser.

## Senest færdigt

- [PR #28](https://github.com/Xerrion/particle-foundry/pull/28) er merged. Vandpenslen tegner nu sammenhængende streger, så separate penselaftryk ikke skaber det gentagne tandmønster.
- [PR #27](https://github.com/Xerrion/particle-foundry/pull/27) er merged. Undersøgelsen fandt ikke områder, som sikkert kunne springes over i den afprøvede væsketransport.

Rettelsen giver ingen påvist forbedring af simulationshastigheden. **60 viste FPS og simulering i realtid er stadig ikke opnået.** Merge, lokal test og verificeret drift er forskellige ting; den seneste rettelse er endnu ikke verificeret i offentlig drift. [Målinger og testresultater](validation/p1-m3-e08.md#continuous-gpu-brush-strokes-on-14-october-2026).

## Hvorfor siger planen M4/E09?

**P1/M4/E09 er migrationsplanens næste item.** Det dækker varme, termodynamik og faseskift og er allerede påbegyndt i [XER-33](https://linear.app/xerrion/issue/XER-33/add-full-supported-thermodynamics-and-phase-closure). En afgrænset Rust-reference findes; hele opgaven er ikke færdig. Performanceopfølgningen på M3/E08 har fået prioritet, før vi fortsætter dette spor.

Planens rækkefølge og det valgte arbejdsspor kan derfor være forskellige. M1/E04 om elektriske kredsløb mangler også. Vi har ikke afsluttet alt før M4.

## Næste skridt

Merge og statusopdatering afslutter den seneste aftalte opgave. Der er ikke startet en ny optimering. Forslaget er at undersøge trykløserens arbejde og ventetiden på GPU-resultater; det kræver, at vi vælger det som næste opgave.

Slutmålet er hele det eksisterende sandbox-spil med **60 viste FPS, simulering i realtid, hurtig betjening og fysisk troværdig adfærd**. Den lille GPU-demo er kun en del af det. Det normale spil bruger fortsat den eksisterende TypeScript-motor.

## Sådan holdes status ajour

Opdater denne side i samme ændring, når vi vælger et nyt arbejdsspor, afslutter eller merger et item, ændrer en blokering eller verificerer drift. Ret dato, P/M/E, senest færdigt og næste skridt. Kontrollér live Linear-status og de relevante testresultater først. Siden er et læsevenligt overblik; den ændrer ikke planens acceptkrav.

Kilder: [aktivt arbejde](remaining-work.md), [fasernes rækkefølge](data/development-phases.json), [engineering-items](data/engine-migration-work.json) og [forklaring af milepælene](PHASES.md).
