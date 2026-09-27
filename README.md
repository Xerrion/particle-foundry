# Particle Foundry

En browserbaseret falling-sand-sandkasse med reproducerbar fysik, varmeledning,
faseændringer, væsketransport, idealgastryk, bevægelige faste parceller og begrænset
forbrænding. Verden er som standard et gitter på 480 × 270 celler med zoom og panorering.

Læs [modelspecifikationen](docs/model.md) for enheder, tilstandsejerskab,
bevarelsesregler og modellens bevidste grænser. [Restarbejdet](docs/remaining-work.md)
beskriver aktiv backlog og leveret historik; [roadmap](docs/roadmap.md) viser
rækkefølgen for GPU-redesign og den planlagte opfølgning med trykbølger og brud.

## Kør lokalt

Projektet bruger Bun 1.3.12. Rust/WASM-porten bevarer TypeScript-brugerfladen;
standardscenen bruger fortsat den eksisterende motor. Build og typekontrol kræver
de genererede WASM-bindings. Med Rustup installeret køres følgende én gang:

```sh
bun install --frozen-lockfile
rustup show
cargo install wasm-bindgen-cli --version 0.2.108 --locked
bun run build:wasm
```

`rust-toolchain.toml` låser Rust 1.98.1 og WASM-target; `Cargo.lock` låser Rust-afhængighederne.
Se [E01-verifikation](docs/validation/p1-m1.md) for macOS SDK-workaround og browservalg.

| Kommando | Formål |
| --- | --- |
| `bun install --frozen-lockfile` | Installer låste afhængigheder. |
| `bun run dev` | Start Vite med live reload. |
| `bun run lint` | Kør Biome formatterings- og lintkontrol. |
| `bun run typecheck` | Kør streng TypeScript-kontrol. |
| `bun run typecheck:catalogue` | Kontrollér ID-typer og katalog med `noUncheckedIndexedAccess`. |
| `bun run test` | Kør alle modul- og integrationstests. |
| `bun run check` | Kør begge typekontroller og tests. |
| `bun run build:wasm` | Byg Rust/WASM og generér JS/TypeScript-bindings. |
| `bun run check:wasm` | Kontrollér genererede filer byte for byte mod et nyt build. |
| `bun run build` | Generér WASM-bindings og byg produktionsudgaven i `dist/`. |
| `bun run test:browser` | Byg og test WASM-bootstrap i en isoleret headless-browser. |
| `bun run bench` | Mål 240 × 135-belastninger og 480 × 270-startscenen mod 60 Hz-budgettet. |
| `bun run preview` | Servér den byggede udgave lokalt. |

GitHub Actions er konfigureret til Rust-format/tests/Clippy, WASM-generering,
frontend-checks og browser-smoke ved push og pull requests. GPU-smoke er valgfri
i CI og rapporterer manglende adapter særskilt; den kan kræves med
`bun run test:browser artifacts/validation/browser/nyt-run --require-gpu`.
`CHROME_BIN` vælger browserens executable. Smoke-testen bruger et produktionsbundle
under `/engine-smoke/`, mens det almindelige app-bundle fortsat kun bruger legacy-motoren.

## Betjening

- Vælg et materiale og hold musen/fingeren stille for at male kontinuerligt, eller træk
  en streg. `1`–`9` vælger de mærkede materialer, `0` viskelæder, `H` varme,
  `C` kulde og `B` **Blast**.
- `Space` eller pauseknappen stopper og starter simulationen. Hastighedsvælgeren
  ændrer antallet af faste 1/60-sekunders fysiktrin. Ved tunge verdener sænkes
  simulationens fremdrift automatisk, så lærredet stadig kan reagere på input.
- Rul over lærredet eller brug `+`/`−` for at zoome ind og ud. Træk med Shift,
  højre eller midterste museknap for at panorere. `1×` viser hele verden.
- Varme og kulde ændrer termisk entalpi. Værktøjerne er registrerede eksterne
  energikilder/-tab og er begrænset til -200 °C og 3.000 °C.
- Cellemåleren viser materiale og temperatur. Statuslinjen viser stofmasse, samlet
  sporet energi og største gastryk.
- **Surface shimmer** giver lysvariation på faktiske frie væskeoverflader. Effekten
  tegner ikke ekstra væske eller huller og skærer ikke gennem olie/vand-grænsen.
- **World boundaries** under **Settings** kan slås fra for at åbne toppen og
  siderne, så gas, varm luft og overtryk kan forlade verden. Startscenens gulv
  bliver stående, også efter **Reset scene**. Verden er stadig et endeligt gitter;
  stof og energi, som forlader det, bogføres som eksterne udvekslinger.
- **Temperature map** (`T`) lægger en temperaturfarve oven på scenen, inklusive
  luft, med en fast, ikke-lineær skala fra -200 til 3.000 °C. Ved 22 °C er
  overlejringen usynlig; stærkere afvigelser giver tydeligere farve, mens
  materialerne forbliver synlige. Cellemåleren viser den faktiske temperatur.
  Kortet ændrer ingen energi og virker også under pause.
- **Pressure map** (`P`) viser over- og undertryk i kPa i gas og væske. Trykket i
  væskesøjler stiger med dybden; faste celler beholder deres materialefarve.
  **Velocity map** (`V`) viser fart i både materialer og usynlig luft. Pile viser
  luftens og andre gassers retning. Naboceller udveksler bevægelse, så vind kan
  brede sig gennem luft og føre røg, damp og flammer sidelæns; faste vægge
  blokerer. Begge kort lægger en farve oven på scenen, som bliver tydeligere ved
  større afvigelse fra normaltryk eller nul hastighed. Cellemåleren viser tryk
  samt fart og de vandrette/lodrette hastighedskomponenter. Kortene virker også
  under pause.
- Opvarm vand eller bunden af et kar med **Heat** (`H`). Når der er lagret nok
  latent varme, dannes individuelle dampbobler, som stiger gennem vandet og bryder
  overfladen. Vand under kogepunktet får ikke kunstige bobler.
- Mal **Fire** på træ, planter eller olie for at antænde dem. Træ bliver stående,
  gløder og afgiver flammer, mens den begrænsede brændselsenergi bruges. Vand slukker.
  I tom luft giver penslen kun en lille, kortlivet antændelsesflamme uden røg;
  den fastholdte pensel genstarter ikke eksisterende flammers levetid.
- Mal **Gunpowder** og antænd det med ild, varme eller **Blast**. Det lagrede
  kemiske energilager bliver til varme, flammer, et kortvarigt overtryk og udadgående
  hastighed. Blast-værktøjet tilfører registreret ekstern energi; forankrede vægge
  afskærmer effekten. Dette er en kalibreret sandkasseregel, ikke en præcis
  sprængningsmodel.
- Søg eller filtrér materialerne efter kategori. Byg en sammenhængende kreds fra
  **Battery** gennem **Wire** eller **Metal** og **Lamp** til **Ground**. Batteriets
  begrænsede kemiske energi bliver til varme i lederne, så lampen kan gløde.
  Luft, sten og diagonale berøringer lukker ikke kredsen. Resistans og 12 V er
  kalibrerede spilværdier.
- Sidepanelet har fanerne **Materials**, **Elements** og **Settings**. Valgt pensel,
  radius, værktøjer og sceneknapper forbliver tilgængelige, mens indholdet ruller.
  **What can I do with this?** viser den valgte models interaktioner og grænser.
- **Elements** tilbyder otte særskilt modellerede grundstoffer: hydrogen, helium,
  carbon, nitrogen, oxygen, svovl, jern og kobber. De øvrige 110 er deaktiverede referenceposter og vises
  kun med **Show unmodeled elements**. Søg efter navn, symbol eller atomnummer.
  [elements.md](docs/elements.md) beskriver data, understøttede interaktioner og grænser.
- Hydrogen i direkte kontakt med oxygen reagerer efter opvarmning/antændelse:
  **2 H₂ + O₂ → 2 H₂O**. Fire på begge sider af kontakten virker også.
  Antændt hydrogen kan desuden brænde i åben luft; oxygen alene brænder ikke.
  Carbon og svovl danner henholdsvis CO₂ og SO₂ ved kontakt med oxygen efter antændelse.
  Masse, overskydende reaktant og frigivet energi bogføres.
  Helium og nitrogen er inerte. Jern og kobber har egne fasekurver og forskellige termiske/
  elektriske egenskaber; rust, korrosion og legeringer er endnu ikke modelleret.
- **Hydrochloric acid 1 M** og **Sulfuric acid 0.5 M** er malbare vandige presets.
  Begge giver ca. 1 mol syreækvivalenter pr. liter i denne model. Ved kontakt
  med **Sodium hydroxide 1 M** bliver et par naboceller til neutraliseret
  opløsning og afgiver begrænset varme. Vand eller en væg imellem dem reagerer
  ikke. Dette er en cellemodel for neutralisation, endnu uden fortynding,
  korrosion, pH-felt eller særskilte saltarter.
- **Reset scene** genskaber startscenen med samme seed. **Clear all** nulstiller
  verden til luft ved 22 °C og nulstiller de seedede tilfældighedsstrømme.

## Fysisk model

Hvert parcel har autoritativ masse, volumen, termisk og kemisk energi, ilt,
hastighed og tryk i `src/simulation/world.ts`. Alle felter flyttes samlet. Faseændringer beholder
masse og entalpi, mens volumen følger den nye fases densitet.

Termisk energi er masseafhængig parcelentalpi i joule. Varmeledning er synkron
og konservativ over fire naboer. Solveren modtager
eksplicit cellestørrelse, repræsenteret dybde og tidskridt; standarderne kommer fra
`src/simulation/physical-scale.ts`. Is/vand/damp, metal/smeltet metal og sten/lava har reversible
latente overgange. Sand bliver irreversibelt til glas i den nuværende model.

Væske bruger en hybridmodel: cellegitteret afgør forbindelser omkring vægge,
kanaler og overhæng, mens `src/physics/fluid-solver.ts` afleder volumen, overfladehøjde og
lodret hastighed pr. søjle. Vand, olie, lava og smeltet metal har forskellig
viskositetsdæmpning. Transport flytter masse, entalpi og impuls samlet.

`src/physics/solid-mechanics.ts` flytter kun faste parceller, som udtrykkeligt er markeret
dynamiske. Det holder terræn og malede kar forankrede. Modellen anvender tyngde,
densitetsbaseret opdrift, kollision og omdanner kollisionsenergi til varme.

`src/physics/gas-dynamics.ts` udleder gastryk med idealgasloven og væsketryk af søjledybde.
Trykforskelle giver vandrette og lodrette impulser gennem åbne naboceller; væskens
vandrette hastighed påvirker dens foretrukne strømningsretning. Gastransport følger
åbninger i gitteret og krydser ikke vægge. Vandets kogepunkt følger trykket omkring
normalpunktet; i et lukket kar kan vand derfor blive varmere end 100 °C, før det
koger. `src/physics/reactions.ts`
bruger et seedet forløb, lagret brændselsenergi og lokal ilt; røg, der forlader
modellen, registreres som et åbent massetab.

`src/physics/explosions.ts` omsætter et gunpowder-parcels begrænsede kemiske energi til varme og
bevægelse. Blast-værktøjet fører sin energi som ekstern kilde i regnskabet.
`src/physics/electricity.ts` løser tilstødende ledere som et resistivt netværk og trækker
Joule-varmen fra batteriets kemiske lager. [Funktionsplanen](docs/roadmap.md) beskriver
de næste trin for syrekemi samt flere simulerede grundstoffer og spilelementer.
`src/physics/neutralization.ts` reagerer et tilstødende syre/base-par ad gangen og finansierer
varmen fra syrens begrænsede kemiske lager.

Ild og damp følger forskellige gasprofiler. Ild flimrer, stiger hurtigere end damp,
overfører varme konservativt til brændbart nabomateriale og fortsætter kun med at
frigive varme, mens der er både brændsel og ilt. En nymalet flamme er et tidsbegrænset
varmt gasparcel og forsvinder derfor ikke efter ét trin. Damp stiger gennem tungere
luft og røg, breder sig under barrierer og kondenserer igen, når temperatur og det
lokale tryk kræver det.

## Kildestruktur

Koden er opdelt efter ansvar i `src/app/`, `src/materials/`, `src/physics/`,
`src/rendering/`, `src/scenes/`, `src/simulation/` og `src/tools/`. Browseren starter
i `src/main.ts` via rodens `index.html`; styles ligger i `src/styles/`.
Tests følger de samme områder med fælles fixtures og særskilte integrationstests.
[Projektstrukturen](docs/project-structure.md) beskriver placeringer og konventioner.
Lokale profiler, logs, browseroptagelser og snapshots ligger i den ignorerede
`artifacts/`-mappe.

`src/materials/definitions.ts` er materialernes eneste autoritative katalog: ID, navn,
farver, genvej, starttemperatur, densitet, varmeegenskaber, antændelse, fasefamilie,
bevægelse og overfladeprofil. De gamle `src/materials/ids.ts`, `src/materials/physical-properties.ts` og
`src/materials/presentation.ts` re-eksporterer kun kataloget for kompatibilitet.
Materialevælgeren og dens farveprøver genereres fra kataloget.

`MaterialId` er unionen af de faktiske register-ID'er, og `ToolId` omfatter kun
værktøjer. `isMatterId(number)` indsnævrer typen; `physicalProperties(number)` er
det kontrollerede opslag for ukendt input. `materialsById` er et ID-indekseret
hot-path-opslag til de samme komplette definitioner, ikke en separat fysiktabel.
`pickerIds` omfatter kun valgbare materialer og værktøjer.
Katalogets delte profiler og de afledte opslag er frosset ved runtime.

`src/materials/validation.ts` kører én gang ved import, også i udvikling og
headless tests. Den afviser bl.a. dublerede ID'er/genveje, brudte fasereferencer,
inkonsistente entalpiintervaller og manglende gas-/væskeinput. Gas kræver positiv
molarmasse og gasbevægelse; væske kræver flow. Nul brændselsenergi og uendelig
antændelsestærskel er gyldige markører for ikke-brændbart materiale. `falls` er
uafhængig af fysisk tilstand, så is stadig kan være et faldende fast stof.
Enheder og forskellen på fysiske værdier og tuning står i [model.md](docs/model.md).

Et nyt materiale med eksisterende adfærd tilføjes med et ubrugt ID i 0–255 og
en `define(...)`-post i kataloget. En ny smelte-/kogefamilie beskrives med fase-ID'er
og entalpiendepunkter; den generiske termiske solver læser disse data. Helt nye
reaktionstyper kræver stadig en algoritme og tests, ikke kun en ny tabelpost.

- `src/simulation/world.ts`: autoritativ tilstand og atomiske celleoperationer
- `src/simulation/physics.ts`: rækkefølge for ét fast simulationstrin
- `src/simulation/physical-scale.ts`: fælles skala, enheder og tolerancer
- `src/materials/definitions.ts`: samlet materialekatalog og delte fasefamilier
- `src/materials/validation.ts`: opstartsvalidering af katalogets relationer og input
- `src/materials/index.ts`: offentlig importflade til kataloget
- `src/materials/queries.ts`: fælles materialeklassifikation og frosne bevægelsesprofiler
- `src/physics/thermal.ts`: generisk fortolkning af fasekurver og varmeledning
- `src/physics/boiling.ts`: lokale, energifinansierede dampbobler ved væske/gas-overgange
- `src/physics/motion.ts` / `src/physics/hydrostatics.ts`: lokal cellulær transport og forbindelsesgraf
- `src/physics/fluid-solver.ts`: afledte væskesøjler, viskositet og fysisk impuls
- `src/physics/solid-mechanics.ts`: dynamiske faste parceller, opdrift og kollision
- `src/physics/gas-dynamics.ts`: idealgastryk og trykimpulser
- `src/physics/reactions.ts`: stofomdannelse, ilt og kemisk energi
- `src/physics/electricity.ts`: begrænsede batterier, resistive forbindelser og Joule-varme
- `src/physics/neutralization.ts`: vandige syre/base-presets og energifinansieret neutralisation
- `src/materials/element-reference.ts`: 118 navne, symboler og atomnumre til opslag, ikke fysikdata
- `src/simulation/diagnostics.ts`: rene masse- og energiregnskaber
- `src/simulation/random.ts`: reproducerbare, seedede tilfældighedsstrømme
- `src/tools/brush.ts` / `src/scenes/starter-scene.ts`: brugerredigering og starttilstand
- `src/rendering/renderer.ts` / `src/rendering/visual-waves.ts` / `src/rendering/temperature-map.ts`: rendering uden fysisk mutation
- `src/app/controls.ts` / `src/main.ts`: DOM-binding og browserens frame-loop
- `src/app/material-picker.ts` / `src/app/view-controls.ts`: materialevalg og feltvisninger
- `src/app/pointer-mapping.ts` / `src/app/dom.ts`: koordinatkonvertering og valideret DOM-opslag
- `src/simulation/simulation-clock.ts`: forløbet tid til faste fysiktrin
- `src/simulation/sandbox.ts`: offentlig API og sammensætning

Testpakken dækker både interne invariants, headless DOM-forløb og referenceadfærd for
diffusion, hydrostatik, viskositet, gastryk, faste kollisioner, reaktioner og reproducerbarhed.
Simulationen er stadig en grovkornet undervisningsmodel, ikke et værktøj til
dimensionering eller sikkerhedskritiske beregninger.
