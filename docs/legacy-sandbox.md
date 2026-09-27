# Den eksisterende sandkasse

Betjening og implementering af den nuværende TypeScript-motor.
Se [dokumentationsindekset](README.md) og [projektstrukturen](project-structure.md)
for navigation; Rust/WASM-migrationen er fortsat planlagt.

## Betjening

- Vælg et materiale og hold musen/fingeren stille for at male kontinuerligt, eller træk
  en streg. `1`-`9` vælger de mærkede materialer, `0` viskelæder, `H` varme,
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
  [elements.md](elements.md) beskriver data, understøttede interaktioner og grænser.
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
hastighed og tryk i `web/src/legacy/simulation/world.ts`. Alle felter flyttes samlet. Faseændringer beholder
masse og entalpi, mens volumen følger den nye fases densitet.

Termisk energi er masseafhængig parcelentalpi i joule. Varmeledning er synkron
og konservativ over fire naboer. Solveren modtager
eksplicit cellestørrelse, repræsenteret dybde og tidskridt; standarderne kommer fra
`web/src/legacy/simulation/physical-scale.ts`. Is/vand/damp, metal/smeltet metal og sten/lava har reversible
latente overgange. Sand bliver irreversibelt til glas i den nuværende model.

Væske bruger en hybridmodel: cellegitteret afgør forbindelser omkring vægge,
kanaler og overhæng, mens `web/src/legacy/physics/fluid-solver.ts` afleder volumen, overfladehøjde og
lodret hastighed pr. søjle. Vand, olie, lava og smeltet metal har forskellig
viskositetsdæmpning. Transport flytter masse, entalpi og impuls samlet.

`web/src/legacy/physics/solid-mechanics.ts` flytter kun faste parceller, som udtrykkeligt er markeret
dynamiske. Det holder terræn og malede kar forankrede. Modellen anvender tyngde,
densitetsbaseret opdrift, kollision og omdanner kollisionsenergi til varme.

`web/src/legacy/physics/gas-dynamics.ts` udleder gastryk med idealgasloven og væsketryk af søjledybde.
Trykforskelle giver vandrette og lodrette impulser gennem åbne naboceller; væskens
vandrette hastighed påvirker dens foretrukne strømningsretning. Gastransport følger
åbninger i gitteret og krydser ikke vægge. Vandets kogepunkt følger trykket omkring
normalpunktet; i et lukket kar kan vand derfor blive varmere end 100 °C, før det
koger. `web/src/legacy/physics/reactions.ts`
bruger et seedet forløb, lagret brændselsenergi og lokal ilt; røg, der forlader
modellen, registreres som et åbent massetab.

`web/src/legacy/physics/explosions.ts` omsætter et gunpowder-parcels begrænsede kemiske energi til varme og
bevægelse. Blast-værktøjet fører sin energi som ekstern kilde i regnskabet.
`web/src/legacy/physics/electricity.ts` løser tilstødende ledere som et resistivt netværk og trækker
Joule-varmen fra batteriets kemiske lager. [Funktionsplanen](roadmap.md) beskriver
de næste trin for syrekemi samt flere simulerede grundstoffer og spilelementer.
`web/src/legacy/physics/neutralization.ts` reagerer et tilstødende syre/base-par ad gangen og finansierer
varmen fra syrens begrænsede kemiske lager.

Ild og damp følger forskellige gasprofiler. Ild flimrer, stiger hurtigere end damp,
overfører varme konservativt til brændbart nabomateriale og fortsætter kun med at
frigive varme, mens der er både brændsel og ilt. En nymalet flamme er et tidsbegrænset
varmt gasparcel og forsvinder derfor ikke efter ét trin. Damp stiger gennem tungere
luft og røg, breder sig under barrierer og kondenserer igen, når temperatur og det
lokale tryk kræver det.

## Kildestruktur

Browserkode ligger i `web/src/app/`, materialekataloget i `web/src/materials/`
og den eksisterende motor i `web/src/legacy/`. Browseren starter i `web/src/main.ts`
via `web/index.html`; styles ligger i `web/src/styles/`. UI bruger motoren gennem
`web/src/engine-client/`. Tests følger ejerskabet i `web/tests/` med motorens tests
og fixtures samlet i `web/tests/legacy/`.
[Projektstrukturen](project-structure.md) beskriver placeringer og konventioner.
Lokale profiler, logs, browseroptagelser og snapshots ligger i den ignorerede
`artifacts/`-mappe.

`web/src/materials/definitions.ts` er materialernes eneste autoritative katalog: ID, navn,
farver, genvej, starttemperatur, densitet, varmeegenskaber, antændelse, fasefamilie,
bevægelse og overfladeprofil. De gamle `web/src/materials/ids.ts`, `web/src/materials/physical-properties.ts` og
`web/src/materials/presentation.ts` re-eksporterer kun kataloget for kompatibilitet.
Materialevælgeren og dens farveprøver genereres fra kataloget.

`MaterialId` er unionen af de faktiske register-ID'er, og `ToolId` omfatter kun
værktøjer. `isMatterId(number)` indsnævrer typen; `physicalProperties(number)` er
det kontrollerede opslag for ukendt input. `materialsById` er et ID-indekseret
hot-path-opslag til de samme komplette definitioner, ikke en separat fysiktabel.
`pickerIds` omfatter kun valgbare materialer og værktøjer.
Katalogets delte profiler og de afledte opslag er frosset ved runtime.

`web/src/materials/validation.ts` kører én gang ved import, også i udvikling og
headless tests. Den afviser bl.a. dublerede ID'er/genveje, brudte fasereferencer,
inkonsistente entalpiintervaller og manglende gas-/væskeinput. Gas kræver positiv
molarmasse og gasbevægelse; væske kræver flow. Nul brændselsenergi og uendelig
antændelsestærskel er gyldige markører for ikke-brændbart materiale. `falls` er
uafhængig af fysisk tilstand, så is stadig kan være et faldende fast stof.
Enheder og forskellen på fysiske værdier og tuning står i [model.md](model.md).

Et nyt materiale med eksisterende adfærd tilføjes med et ubrugt ID i 0-255 og
en `define(...)`-post i kataloget. En ny smelte-/kogefamilie beskrives med fase-ID'er
og entalpiendepunkter; den generiske termiske solver læser disse data. Helt nye
reaktionstyper kræver stadig en algoritme og tests, ikke kun en ny tabelpost.

- `web/src/legacy/simulation/world.ts`: autoritativ tilstand og atomiske celleoperationer
- `web/src/legacy/simulation/physics.ts`: rækkefølge for ét fast simulationstrin
- `web/src/legacy/simulation/physical-scale.ts`: fælles skala, enheder og tolerancer
- `web/src/materials/definitions.ts`: samlet materialekatalog og delte fasefamilier
- `web/src/materials/validation.ts`: opstartsvalidering af katalogets relationer og input
- `web/src/materials/index.ts`: offentlig importflade til kataloget
- `web/src/materials/queries.ts`: fælles materialeklassifikation og frosne bevægelsesprofiler
- `web/src/legacy/physics/thermal.ts`: generisk fortolkning af fasekurver og varmeledning
- `web/src/legacy/physics/boiling.ts`: lokale, energifinansierede dampbobler ved væske/gas-overgange
- `web/src/legacy/physics/motion.ts` / `web/src/legacy/physics/hydrostatics.ts`: lokal cellulær transport og forbindelsesgraf
- `web/src/legacy/physics/fluid-solver.ts`: afledte væskesøjler, viskositet og fysisk impuls
- `web/src/legacy/physics/solid-mechanics.ts`: dynamiske faste parceller, opdrift og kollision
- `web/src/legacy/physics/gas-dynamics.ts`: idealgastryk og trykimpulser
- `web/src/legacy/physics/reactions.ts`: stofomdannelse, ilt og kemisk energi
- `web/src/legacy/physics/electricity.ts`: begrænsede batterier, resistive forbindelser og Joule-varme
- `web/src/legacy/physics/neutralization.ts`: vandige syre/base-presets og energifinansieret neutralisation
- `web/src/materials/element-reference.ts`: 118 navne, symboler og atomnumre til opslag, ikke fysikdata
- `web/src/legacy/simulation/diagnostics.ts`: rene masse- og energiregnskaber
- `web/src/legacy/simulation/random.ts`: reproducerbare, seedede tilfældighedsstrømme
- `web/src/legacy/tools/brush.ts` / `web/src/legacy/scenes/starter-scene.ts`: brugerredigering og starttilstand
- `web/src/legacy/rendering/renderer.ts` / `web/src/legacy/rendering/visual-waves.ts` / `web/src/legacy/rendering/temperature-map.ts`: rendering uden fysisk mutation
- `web/src/app/controls.ts` / `web/src/main.ts`: DOM-binding og browserens frame-loop
- `web/src/app/material-picker.ts` / `web/src/app/view-controls.ts`: materialevalg og feltvisninger
- `web/src/app/pointer-mapping.ts` / `web/src/app/dom.ts`: koordinatkonvertering og valideret DOM-opslag
- `web/src/app/simulation-clock.ts`: forløbet tid til faste fysiktrin
- `web/src/legacy/simulation/sandbox.ts`: offentlig API og sammensætning

Testpakken dækker både interne invariants, headless DOM-forløb og referenceadfærd for
diffusion, hydrostatik, viskositet, gastryk, faste kollisioner, reaktioner og reproducerbarhed.
Simulationen er stadig en grovkornet undervisningsmodel, ikke et værktøj til
dimensionering eller sikkerhedskritiske beregninger.
