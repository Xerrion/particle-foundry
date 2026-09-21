# Particle Foundry

En browserbaseret falling-sand-sandkasse med reproducerbar fysik, varmeledning,
faseændringer, væsketransport, idealgastryk, bevægelige faste parceller og begrænset
forbrænding. Verden er som standard et gitter på 240 × 135 celler.

Læs [modelspecifikationen](MODEL.md) for enheder, tilstandsejerskab,
bevarelsesregler og modellens bevidste grænser. [Restarbejdsrapporten](RESTARBEJDE.md)
viser, hvordan den tidligere backlog er afsluttet.

## Kør lokalt

Projektet bruger Bun 1.3.12:

| Kommando | Formål |
| --- | --- |
| `bun install` | Installer låste afhængigheder. |
| `bun run dev` | Start Vite med live reload. |
| `bun run lint` | Kør Biome formatterings- og lintkontrol. |
| `bun run typecheck` | Kør streng TypeScript-kontrol. |
| `bun run typecheck:catalogue` | Kontrollér ID-typer og katalog med `noUncheckedIndexedAccess`. |
| `bun run test` | Kør alle modul- og integrationstests. |
| `bun run check` | Kør begge typekontroller og tests. |
| `bun run build` | Byg produktionsudgaven i `dist/`. |
| `bun run bench` | Mål tom, fyldt og fragmenteret 240 × 135-verden mod 60 Hz-budgettet. |
| `bun run preview` | Servér den byggede udgave lokalt. |

GitHub Actions kører lint, checks og build ved push og pull requests.

## Betjening

- Vælg et materiale og hold musen/fingeren stille for at male kontinuerligt, eller træk
  en streg. `1`–`9` vælger de mærkede materialer, `0` viskelæder, `H` varme,
  `C` kulde og `B` **Blast**.
- `Space` eller pauseknappen stopper og starter simulationen. Hastighedsvælgeren
  ændrer antallet af faste 1/60-sekunders fysiktrin.
- Varme og kulde ændrer termisk entalpi. Værktøjerne er registrerede eksterne
  energikilder/-tab og er begrænset til -200 °C og 3.000 °C.
- Cellemåleren viser materiale og temperatur. Statuslinjen viser stofmasse, samlet
  sporet energi og største gastryk.
- **Surface shimmer** giver lysvariation på faktiske frie væskeoverflader. Effekten
  tegner ikke ekstra væske eller huller og skærer ikke gennem olie/vand-grænsen.
- **Temperature map** (`T`) viser temperatur i hele feltet, inklusive luft, med en
  fast, ikke-lineær farveskala fra -200 til 3.000 °C. Farver uden for intervallet
  mættes; cellemåleren viser den faktiske temperatur og materialet. Kortet ændrer
  ingen energi og virker også under pause.
- **Pressure map** (`P`) viser over- og undertryk i kPa i gas og væske. Trykket i
  væskesøjler stiger med dybden; faste celler vises mørke. **Velocity map** (`V`)
  viser parcelfart i m/s. Cellemåleren viser tryk samt fart og de vandrette/lodrette
  hastighedskomponenter. Kortene er visninger af modeltilstanden og virker under pause.
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
- **Reset scene** genskaber startscenen med samme seed. **Clear all** nulstiller
  verden til luft ved 22 °C og nulstiller de seedede tilfældighedsstrømme.

## Fysisk model

Hvert parcel har autoritativ masse, volumen, termisk og kemisk energi, ilt,
hastighed og tryk i `world.ts`. Alle felter flyttes samlet. Faseændringer beholder
masse og entalpi, mens volumen følger den nye fases densitet.

Varmeledning er synkron og konservativ over fire naboer. Solveren modtager
eksplicit cellestørrelse, repræsenteret dybde og tidskridt; standarderne kommer fra
`physical-scale.ts`. Is/vand/damp, metal/smeltet metal og sten/lava har reversible
latente overgange. Sand bliver irreversibelt til glas i den nuværende model.

Væske bruger en hybridmodel: cellegitteret afgør forbindelser omkring vægge,
kanaler og overhæng, mens `fluid-solver.ts` afleder volumen, overfladehøjde og
lodret hastighed pr. søjle. Vand, olie, lava og smeltet metal har forskellig
viskositetsdæmpning. Transport flytter masse, entalpi og impuls samlet.

`solid-mechanics.ts` flytter kun faste parceller, som udtrykkeligt er markeret
dynamiske. Det holder terræn og malede kar forankrede. Modellen anvender tyngde,
densitetsbaseret opdrift, kollision og omdanner kollisionsenergi til varme.

`gas-dynamics.ts` udleder gastryk med idealgasloven og væsketryk af søjledybde.
Trykforskelle giver vandrette og lodrette impulser gennem åbne naboceller; væskens
vandrette hastighed påvirker dens foretrukne strømningsretning. Gastransport følger
åbninger i gitteret og krydser ikke vægge. Vandets kogepunkt følger trykket omkring
normalpunktet. `reactions.ts`
bruger et seedet forløb, lagret brændselsenergi og lokal ilt; røg, der forlader
modellen, registreres som et åbent massetab.

`explosions.ts` omsætter et gunpowder-parcels begrænsede kemiske energi til varme og
bevægelse. Blast-værktøjet fører sin energi som ekstern kilde i regnskabet. Den første
udvidelsesfase dækker lokale eksplosioner; [funktionsplanen](ROADMAP.md) beskriver
elektricitet, syrer og de 118 kemiske grundstoffer i efterfølgende faser.

Ild og damp følger forskellige gasprofiler. Ild flimrer, stiger hurtigere end damp,
overfører varme konservativt til brændbart nabomateriale og fortsætter kun med at
frigive varme, mens der er både brændsel og ilt. En nymalet flamme er et tidsbegrænset
varmt gasparcel og forsvinder derfor ikke efter ét trin. Damp stiger gennem tungere
luft og røg, breder sig under barrierer og kondenserer igen, når temperatur og det
lokale tryk kræver det.

## Kildestruktur

`material-definitions.ts` er materialernes eneste autoritative katalog: ID, navn,
farver, genvej, starttemperatur, densitet, varmeegenskaber, antændelse, fasefamilie,
bevægelse og overfladeprofil. De gamle `material-ids.ts`, `material-physics.ts` og
`material-presentation.ts` re-eksporterer kun kataloget for kompatibilitet.
Materialevælgeren og dens farveprøver genereres fra kataloget.

`MaterialId` er unionen af de faktiske register-ID'er, og `ToolId` omfatter kun
værktøjer. `isMatterId(number)` indsnævrer typen; `physicalProperties(number)` er
det kontrollerede opslag for ukendt input. `materialsById` er et ID-indekseret
hot-path-opslag til de samme komplette definitioner, ikke en separat fysiktabel.
`pickerIds` omfatter kun valgbare materialer og værktøjer; indholdet er uændret.
Katalogets delte profiler og de afledte opslag er frosset ved runtime.

`validate-material-catalogue.ts` kører én gang ved import, også i udvikling og
headless tests. Den afviser bl.a. dublerede ID'er/genveje, brudte fasereferencer,
inkonsistente entalpiintervaller og manglende gas-/væskeinput. Gas kræver positiv
molarmasse og gasbevægelse; væske kræver flow. Nul brændselsenergi og uendelig
antændelsestærskel er gyldige markører for ikke-brændbart materiale. `falls` er
uafhængig af fysisk tilstand, så is stadig kan være et faldende fast stof.
Enheder og forskellen på fysiske værdier og tuning står i [MODEL.md](MODEL.md).

Et nyt materiale med eksisterende adfærd tilføjes med et ubrugt ID i 0–255 og
en `define(...)`-post i kataloget. En ny smelte-/kogefamilie beskrives med fase-ID'er
og entalpiendepunkter; den generiske termiske solver læser disse data. Helt nye
reaktionstyper kræver stadig en algoritme og tests, ikke kun en ny tabelpost.

- `world.ts`: autoritativ tilstand og atomiske celleoperationer
- `physics.ts`: rækkefølge for ét fast simulationstrin
- `physical-scale.ts`: fælles skala, enheder og tolerancer
- `material-definitions.ts`: samlet materialekatalog og delte fasefamilier
- `validate-material-catalogue.ts`: opstartsvalidering af katalogets relationer og input
- `materials.ts`: offentlig importflade til kataloget
- `thermal.ts`: generisk fortolkning af fasekurver og varmeledning
- `boiling.ts`: lokale, energifinansierede dampbobler ved væske/gas-overgange
- `motion.ts` / `hydrostatics.ts`: lokal cellulær transport og forbindelsesgraf
- `fluid-solver.ts`: afledte væskesøjler, viskositet og fysisk impuls
- `solid-mechanics.ts`: dynamiske faste parceller, opdrift og kollision
- `gas-dynamics.ts`: idealgastryk og trykimpulser
- `reactions.ts`: stofomdannelse, ilt og kemisk energi
- `diagnostics.ts`: rene masse- og energiregnskaber
- `random.ts`: reproducerbare, seedede tilfældighedsstrømme
- `brush.ts` / `starter-scene.ts`: brugerredigering og starttilstand
- `renderer.ts` / `visual-waves.ts` / `temperature-map.ts`: rendering uden fysisk mutation
- `controls.ts` / `app.ts`: DOM-binding og browserens frame-loop
- `simulation-clock.ts`: forløbet tid til faste fysiktrin
- `sandbox.ts`: offentlig API og sammensætning

Testpakken dækker både interne invariants, headless DOM-forløb og referenceadfærd for
diffusion, hydrostatik, viskositet, gastryk, faste kollisioner, reaktioner og reproducerbarhed.
Simulationen er stadig en grovkornet undervisningsmodel, ikke et værktøj til
dimensionering eller sikkerhedskritiske beregninger.
