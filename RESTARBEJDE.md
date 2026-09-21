# Restarbejde: gennemført

**Statusdato: 20. september 2026.** Den tidligere prioriterede backlog R1–R8 er
implementeret som en afgrænset hybridmodel. Den autoritative beskrivelse af skala,
enheder, bevarelse og forenklinger findes i [MODEL.md](MODEL.md).

## Leverede ændringer

| Punkt | Resultat | Primære filer |
| --- | --- | --- |
| R1 · skala og bevarelse | Fælles 1 cm celleskala, 1 cm dybde og 60 Hz; masse, volumen, hastighed, tryk, ilt og kemisk energi ejes af verden og flyttes atomisk. Kilde-/tabsledger og numeriske tolerancer er defineret. | `physical-scale.ts`, `world.ts`, `diagnostics.ts`, `MODEL.md` |
| R2 · væske og bølger | Hybrid væskesolver afleder volumen, højde og lodret hastighed pr. søjle fra autoritative celleparceller. Forbindelser følger gittergeometri, transport flytter hele parceltilstanden, og viskositet dæmper impuls materialeforskelligt. Den visuelle bølgeeffekt er fortsat tydeligt isoleret. | `fluid-solver.ts`, `hydrostatics.ts`, `motion.ts` |
| R3 · termisk skala | Diffusion har eksplicit tidskridt, cellestørrelse og repræsenteret dybde. Masse og volumen bevares gennem fase-ID-skift, og volumen opdateres efter fasens densitet. Latent varme bevares. | `thermal.ts`, `world.ts`, `physical-scale.ts` |
| R4 · faste materialer | Forankret terræn er adskilt fra udtrykkeligt dynamiske faste parceller. Dynamiske sten, metal og glas får tyngde, densitetsbaseret opdrift, kollision og termisk dissipering. | `solid-mechanics.ts`, `world.ts` |
| R5 · gastryk | Gasmasse, volumen og temperatur giver tryk med idealgasloven. Trykgradienter kobles som impuls til gas, væske og dynamiske faste parceller; cellulær transport respekterer vægge og åbninger. | `gas-dynamics.ts` |
| R6 · reaktioner | Træ, olie og planter har begrænset kemisk energi. Forbrænding kræver ilt, overfører kemisk energi til varme og stopper uden brændsel/ilt. Røgudløb registreres som massetab. Biologisk vækst er fortsat markeret som en spilregel. | `reactions.ts`, `material-physics.ts` |
| R7 · validering | Fysisk og kosmetisk tilfældighed er separate seedede strømme. Diagnostik måler alle sporede energikomponenter, masse, tryk og fart. Nye referenceforsøg dækker replay, regnskaber, væskesøjler, viskositet, idealgas, dynamiske faste celler og afgrænset forbrænding. En headless DOM-test dækker materialevalg, pause, hastighed, reset/clear, bølger og touch. Et reproducerbart benchmark måler tomme, fyldte og fragmenterede verdener mod 60 Hz-budgettet. CI kører lint, typer, tests og build. | `random.ts`, `diagnostics.ts`, `tests/physical-model.test.ts`, `tests/reactions.test.ts`, `tests/browser-controls.test.ts`, `benchmarks/physics.bench.ts`, `.github/workflows/ci.yml` |
| R8 · filansvar | Materiale-ID'er, fysiske data og præsentation er adskilt. De foreslåede solver- og diagnostikmoduler har egne ansvar. README og UI beskriver og viser den nye model. | `material-ids.ts`, `material-physics.ts`, `material-presentation.ts`, `README.md` |

## Bevidste modelgrænser

“Gennemført” betyder, at backloggens ansvar og testbare mekanismer findes; det betyder
ikke, at sandkassen er blevet en fuld kontinuumssolver. Følgende er eksplicitte
forenklinger og derfor ikke skjult restarbejde:

- Den hybride væskemodel har én diskret parcelplacering pr. celle og kan ikke vise en
  glat fri overflade under celleopløsningen.
- Dynamiske faste celler er stive parceller, ikke deformerbare eller brudbare legemer.
- Idealgasloven bruges uden fugtigheds-, realgas- eller flerkomponentkorrektioner.
- Vandets kogepunkt følger en Clausius-Clapeyron-tilnærmelse omkring normalpunktet;
  flerkomponentvæsker og kritiske tilstande er ikke modelleret.
- Stråling, avanceret konvektion og en reversibel glasmodel er fravalgt. De kan tilføjes
  som nye modeludvidelser, hvis der vælges konkrete materialer og referenceforsøg.
- Plantevækst er en separat spilregel. Den indgår ikke i et fuldt kulstof- og
  fotosynteseregnskab.

Disse grænser står også i [MODEL.md](MODEL.md), så UI eller dokumentation ikke kan
forveksles med en påstand om laboratorienøjagtighed.

## Verifikation

### Rettelser efter visuel afprøvning

- Alle pensler maler kontinuerligt ved fastholdt mus/touch.
- Ild antænder træ uden at erstatte det; brændende træ afgiver flammer og kan
  antænde nabotræ. Olie/vand fortrænger hinanden uden at blive blandet til ét stof.
- Ildpenslen bruger en separat, kortlivet antændelsesprofil fra materialekataloget.
  Fastholdt input giver en lille flamme uden røg i tom luft; almindelige brændselsbrande
  bevares. `tests/fire-brush.test.ts` dækker tre penselstørrelser, slip/slukning,
  tilstandstransport, masse/varme og vedvarende antændelse af træ, olie og planter.
- Faldende vand holdes sammen; hydrostatik fastholder ikke frit faldende strøg.
- Overfladelys tegner ikke falske huller eller dråber ved væskegrænser.
- Lava og andre væsker bytter ikke længere hvilepositioner frem og tilbage;
  farvenuancer følger parcellerne, og udligning virker også under lettere væsker.
- Ild/røg/damp frigiver luft efter diagonal bevægelse; ny gas kan bevæge sig straks.
  Seedet drift og intermitterende diffusion modvirker gitterlåste skakbrætmønstre.
  Gasregressioner dækker sammenhængende plumer, lofter, lukkede hjørner og replay.
- Brede, ensartet opvarmede olieflader afgiver flammer asynkront i stedet for
  vandrette lag. Regressioner følger både opstart og vedvarende brand ved tre
  temperaturer og tre seeds, inklusive kontrol af varme- og brændselsregnskabet.
- Vand/damp kræver fuldført latent varmeovergang, også ved ændringer i tryk.
- Kogende vand danner individuelle energifinansierede dampbobler. Hydrostatikken
  sender ikke længere neddykkede bobler direkte til overfladen. Et opvarmet kar
  testes med tre seeds, inklusive synlig opstigning, dampafgivelse, fravær af
  synkrone damplag og bevarelse af masse/varme (`tests/boiling.test.ts`).
- Temperaturkort med `T`, fast farveforklaring og temperaturvisning inklusive luft.
- Ét materialekatalog driver fysik, fasekurver, reaktionstærskler og materialevælger.
- Kataloget er hærdet med konkrete `MaterialId`-/`ToolId`-typer, tydelige navne for
  kalibrerede celleegenskaber, opstartsvalidering og frosne delte profiler/opslag.
  `tests/material-catalogue.test.ts` samt en særskilt typekontrol med
  `noUncheckedIndexedAccess` beskytter nye materialeændringer uden at ændre værdier.

Regressioner findes i `tests/interaction-regressions.test.ts` samt kontrol-,
rendering- og termiske tests. Det er afgrænsede fejlrettelser i cellemodellen;
de udgør ikke en fuld kontinuerlig væske- eller gassolver.

Kør følgende før levering:

```text
bun run lint
bun run check
bun run build
```

Referenceforsøgene er deterministiske og kræver ingen manuel tegning. De oprindelige
kar-, barriere-, rendering-, fase- og klokketests er bevaret som regressionstests.
