# Restarbejde og leveret historik

**Statusdato: 21. september 2026.** Den tidligere backlog R1–R8 er leveret som
en afgrænset hybridmodel. Det afslutter ikke fysikredesignet: konservativ
fluidtransport, GPU-backend, opløste chokbølger og materialebrud er fortsat planlagt
arbejde. Den aktuelle model og dens begrænsninger beskrives i [model.md](model.md).

## Aktivt restarbejde

Implementeringsrækkefølgen følger [GPU-redesignet](plans/fluid-gpu-redesign/plan.md)
og den efterfølgende [plan for trykbølger og materialebrud](plans/pressure-waves-breakage/plan.md).
Ingen af nedenstående punkter er markeret som implementeret.

| Punkt | Leverance og afslutningskrav | Afhængighed |
| --- | --- | --- |
| R9 · baseline og backend | Bevar den eksisterende arbejdsmappe, genskab audit/check-baseline, og indfør versioneret tilstand, kommandoer og ét autoritativt tilstandsejerskab pr. scene. | GPU M0–M1 |
| R10 · konservativ fluid og GPU | CPU-reference, GPU-paritet og direkte rendering; dokumentér numeriske fejl og faktisk ydelse. Første demonstrator har faste vægge og ingen chokbølger eller brud. | GPU M2–M3 |
| R11 · komplet fluidintegration | Termodynamik, delvise faser, endelig udluftning, korn, bevægelige faste materialer, reaktioner, recovery og valideret overgang til ny backend. | GPU M4–M7 |
| R12 · kompressible trykbølger | Særskilt kompressibel CPU/GPU-model med lokal trykudbredelse, refleksion, udstrømning og energifinansieret forbrænding. Validér mod shock-tube-reference og indelukket antændelse med åbning. | Efter M3; egen CPU-reference før GPU-port |
| R13 · materialebrud | Spændings- og geometriafhængigt brud for metal, glas, sten og træ; bevægelige fragmenter og regnskab for elastisk energi, brud og dissipation. | Kompressibel reference og M5's solid/fluid-kobling |
| R14 · betjening og accept | Breakage-toggle som standard til, langsom gengivelse, deltrinsinspektion, trykvisning, CPU/GPU-sammenligning og målt 480×270-ydelse. | R12–R13 før understøttede scener promoveres |

Breakage-toggle stopper nye brud uden at hele skader eller standse eksisterende
fragmenter; clear/reset bevarer valget. Første kompressible scener understøtter
gas, reaktive korn og faste materialer. Kompressible scener med væske kræver senere
valideret flerfasekobling. Den eksisterende backend bevares til øvrige scener;
to transportsystemer må ikke eje samme scenetilstand.

## Leverede ændringer i den oprindelige hybridmodel

| Punkt | Resultat | Primære filer |
| --- | --- | --- |
| R1 · skala og bevarelse | Fælles 1 cm celleskala, 1 cm dybde og 60 Hz; masse, volumen, hastighed, tryk, ilt og kemisk energi ejes af verden og flyttes atomisk. Kilde-/tabsledger og numeriske tolerancer er defineret. | `src/simulation/physical-scale.ts`, `src/simulation/world.ts`, `src/simulation/diagnostics.ts`, `docs/model.md` |
| R2 · væske og bølger | Hybrid væskesolver afleder volumen, højde og lodret hastighed pr. søjle fra autoritative celleparceller. Forbindelser følger gittergeometri, transport flytter hele parceltilstanden, og viskositet dæmper impuls materialeforskelligt. Den visuelle bølgeeffekt er fortsat tydeligt isoleret. | `src/physics/fluid-solver.ts`, `src/physics/hydrostatics.ts`, `src/physics/motion.ts` |
| R3 · termisk skala | Diffusion har eksplicit tidskridt, cellestørrelse og repræsenteret dybde. Masse og volumen bevares gennem fase-ID-skift, og volumen opdateres efter fasens densitet. Latent varme bevares. | `src/physics/thermal.ts`, `src/simulation/world.ts`, `src/simulation/physical-scale.ts` |
| R4 · faste materialer | Forankret terræn er adskilt fra udtrykkeligt dynamiske faste parceller. Dynamiske sten, metal og glas får tyngde, densitetsbaseret opdrift, kollision og termisk dissipering. | `src/physics/solid-mechanics.ts`, `src/simulation/world.ts` |
| R5 · gastryk | Gasmasse, volumen og temperatur giver tryk med idealgasloven. Trykgradienter kobles som impuls til gas, væske og dynamiske faste parceller; cellulær transport respekterer vægge og åbninger. | `src/physics/gas-dynamics.ts` |
| R6 · reaktioner | Træ, olie og planter har begrænset kemisk energi. Forbrænding kræver ilt, overfører kemisk energi til varme og stopper uden brændsel/ilt. Røgudløb registreres som massetab. Biologisk vækst er fortsat markeret som en spilregel. | `src/physics/reactions.ts`, `src/materials/physical-properties.ts` |
| R7 · validering | Fysisk og kosmetisk tilfældighed er separate seedede strømme. Diagnostik måler alle sporede energikomponenter, masse, tryk og fart. Nye referenceforsøg dækker replay, regnskaber, væskesøjler, viskositet, idealgas, dynamiske faste celler og afgrænset forbrænding. En headless DOM-test dækker materialevalg, pause, hastighed, reset/clear, bølger og touch. Et reproducerbart benchmark måler tomme, fyldte og fragmenterede verdener mod 60 Hz-budgettet. CI kører lint, typer, tests og build. | `src/simulation/random.ts`, `src/simulation/diagnostics.ts`, `tests/integration/physical-model.test.ts`, `tests/physics/reactions.test.ts`, `tests/app/browser-controls.test.ts`, `benchmarks/physics.bench.ts`, `.github/workflows/ci.yml` |
| R8 · filansvar | Materiale-ID'er, fysiske data og præsentation er adskilt. De foreslåede solver- og diagnostikmoduler har egne ansvar. README og UI beskriver og viser den nye model. | `src/materials/ids.ts`, `src/materials/physical-properties.ts`, `src/materials/presentation.ts`, `README.md` |

## Bevidste modelgrænser

Den leverede R1–R8-historik beskriver afgrænsede mekanismer, ikke en fuld
kontinuumssolver. Følgende er den nuværende models begrænsninger; de punkter, som
redesignet skal erstatte, fremgår nu eksplicit af det aktive restarbejde ovenfor:

- Den hybride væskemodel har én diskret parcelplacering pr. celle og kan ikke vise en
  glat fri overflade under celleopløsningen.
- Dynamiske faste celler er stive parceller, ikke deformerbare eller brudbare legemer.
- Eksplosioner giver lokal varme og radiale impulser, men ingen opløste chokbølger.
- Idealgasloven bruges uden fugtigheds-, realgas- eller flerkomponentkorrektioner.
- Vandets kogepunkt følger en Clausius-Clapeyron-tilnærmelse omkring normalpunktet;
  flerkomponentvæsker og kritiske tilstande er ikke modelleret.
- Stråling, avanceret konvektion og en reversibel glasmodel er fravalgt. De kan tilføjes
  som nye modeludvidelser, hvis der vælges konkrete materialer og referenceforsøg.
- Plantevækst er en separat spilregel. Den indgår ikke i et fuldt kulstof- og
  fotosynteseregnskab.

Disse grænser står også i [model.md](model.md), så UI eller dokumentation ikke kan
forveksles med en påstand om laboratorienøjagtighed.

## Verifikation

### Modulopdeling 21. september 2026

Kildekode og tests er grupperet efter ansvar; navngivning og placeringer fremgår af
[projektstrukturen](project-structure.md). Materialeklassifikation er flyttet ud af
bevægelsessolveren, og materialevælger, feltvisninger og koordinatkonvertering er
adskilt fra inputstyringen. DOM-opslag og cellekoordinatvalidering har fælles ejere.
En regression beskytter mod, at brøkkoordinater ændrer en anden celles dynamiske
tilstand. Dette er vedligeholdelse af den eksisterende model, ikke levering af
GPU-redesign, kompressible bølger eller materialebrud.

Historikken nedenfor beskriver eksisterende regressioner. Den er ikke dokumentation
for, at GPU-redesign, kompressible bølger eller materialebrud er implementeret eller
valideret. Nye acceptkrav omfatter chokreference, refleksion, endelig udstrømning,
strukturbrud, toggle-adfærd og bevarelse med fragmenter. CPU/GPU sammenlignes ved
samme fysiske tidspunkt; FPS og simulerede sekunder pr. vægurssekund måles separat.

### Rettelser efter visuel afprøvning

- Alle pensler maler kontinuerligt ved fastholdt mus/touch.
- Ild antænder træ uden at erstatte det; brændende træ afgiver flammer og kan
  antænde nabotræ. Olie/vand fortrænger hinanden uden at blive blandet til ét stof.
- Ildpenslen bruger en separat, kortlivet antændelsesprofil fra materialekataloget.
  Fastholdt input giver en lille flamme uden røg i tom luft; almindelige brændselsbrande
  bevares. `tests/tools/fire-brush.test.ts` dækker tre penselstørrelser, slip/slukning,
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
  synkrone damplag og bevarelse af masse/varme (`tests/physics/boiling.test.ts`).
- Temperaturkort med `T`, fast farveforklaring og temperaturvisning inklusive luft.
- Ét materialekatalog driver fysik, fasekurver, reaktionstærskler og materialevælger.
- Kataloget er hærdet med konkrete `MaterialId`-/`ToolId`-typer, tydelige navne for
  kalibrerede celleegenskaber, opstartsvalidering og frosne delte profiler/opslag.
  `tests/materials/material-catalogue.test.ts` samt en særskilt typekontrol med
  `noUncheckedIndexedAccess` beskytter nye materialeændringer uden at ændre værdier.

Regressioner findes i `tests/integration/interaction-regressions.test.ts` samt kontrol-,
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
