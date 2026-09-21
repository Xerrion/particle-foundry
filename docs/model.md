# Fysisk modelspecifikation

Denne fil er den autoritative beskrivelse af den fysiske model. Simulationen er en
reproducerbar, grovkornet hybridmodel; den er ikke CFD eller en sikker ingeniørmodel.

Beskrivelsen nedenfor gælder den nuværende implementering. Det planlagte
fluid-/GPU-redesign og opfølgningen med kompressible trykbølger og materialebrud er
beskrevet særskilt til sidst; deres egenskaber er endnu ikke implementeret.

## Skala og enheder

| Størrelse | Enhed og standardværdi | Ejer |
| --- | --- | --- |
| Cellebredde | 0,01 m | `src/simulation/physical-scale.ts` |
| Repræsenteret dybde | 0,01 m | `src/simulation/physical-scale.ts` |
| Cellevolumen | 0,000001 m³ | afledt i `src/simulation/physical-scale.ts` |
| Fysiktrin | 1/60 s | `src/simulation/physical-scale.ts` og `src/simulation/simulation-clock.ts` |
| Masse | kg | `world.massKg` |
| Volumen | m³ | `world.volumeM3` |
| Hastighed | m/s | `world.velocityX/Y` |
| Tryk | Pa | `world.pressurePa` |
| Kemisk, kinetisk og potentiel energi | kJ | verdensfelter/`src/simulation/diagnostics.ts` |
| Termisk parcelentalpi | J | `world.energy` |

Termisk energi er nu total parcelentalpi i joule: `E = m × h(T, fase, p)`.
Katalogets kurver angiver energi og varmekapacitet for en referencemasse på 0,001 kg.
Runtime skalerer både følbar og latent varme med faktisk masse. Derfor kræver
vand, som bliver til damp, mere varme end en meget lettere nymalet dampcelle for
samme temperaturstigning. Faseændringer nulstiller aldrig massen.

- `cellHeatCapacity`: J/K for referencemassen; divider med 0,001 kg for J/(kg K).
- `lowerCellEnthalpy` / `upperCellEnthalpy`: J for referencemassen.
- `heatTransferCoefficient`: kalibreret J/K pr. 1/60 s trin, ikke W/(m K).
- `displacementDensity`: dimensionsløs rækkefølge for cellebytning.
- `flow.interval`, levetider og generiske reaktionsrater er fortsat kalibrerede regler.

De eksisterende konstanter er ikke dermed laboratorievaliderede. De fem
elementmodeller og deres afgrænsninger beskrives i [elements.md](elements.md).

## Tilstand og ejerskab

`src/simulation/world.ts` ejer alle autoritative arrays. Et materialeparcels masse, volumen, termiske
entalpi, kemiske energi, ilt, hastighed og øvrige metadata flyttes samlet ved `swap()`.
Solvers må have scratch-arrays, men må ikke spejle en anden autoritativ væskemængde.

En nymalet celle starter som ét fuldt cellevolumen. Ved faseændring bevares masse og
entalpi; parcelvolumen genberegnes fra den nye fases densitet. Et lukket gasområde
har dog kun de cellevolumener, som det faktisk optager. Eksplosionsgas får højst ét
cellevolumen ved dannelsen. Den cellulære geometri kan kun placere ét parcel pr.
celle, så kraftig ekspansion repræsenteres som gastryk frem for at skabe skjulte
celler. Væskesøjler i `src/physics/fluid-solver.ts` er afledte målinger af parcelvolumen og ejer
ikke væske.

## Bevarelse og åbne systemer

- `swap()` flytter tilstanden uændret. Fysisk transport bruger `transport()`, som
  også bogfører begge parcellers ændrede potentielle energi. Samlet sporet energi
  bevares; termisk energi alene kan ændres ved arbejde og dissipation.
- Diffusion anvender lige store og modsatrettede energioverførsler. Standardgrænsen er
  isoleret. Et ændret tidskridt og en ændret geometri gives eksplicit til solveren.
- Kollisionstab bliver termisk energi. Kemisk energi reduceres, når forbrænding frigiver
  samme energimængde som varme.
- Batterier afgiver højst deres lagrede kemiske energi. En elektrisk kant tilfører
  den tilsvarende Joule-varme ligeligt til sine to celler.
- Neutralisation frigiver kun den varme, der er lagret som syrens kemiske energi.
  Syre og base skifter materiale-ID, mens parcelmasse og samlet energi bevares.
- Trykimpulser omsætter ændringen i kinetisk energi til en modsat ændring i lagret
  termisk energi. En eksplosion betaler varme og radial bevægelse fra sit kemiske
  energilager; Blast-værktøjet registrerer i stedet en ekstern energikilde.
- Pensler er åbne kilder eller tab. De registreres i `world.ledger`.
- Røg, hvis levetid udløber i et gasområde med forbindelse til verdens kant,
  registreres som massetab. Den luft, der erstatter røgen i cellen, registreres som
  massetilførsel. I lukkede hulrum bliver udløbet røg til luft med bevaret masse og
  varme. Verdens kant er åben atmosfære for ilt; indvendige, lukkede hulrum har kun
  deres lagrede ilt.
- `src/simulation/diagnostics.ts` opgør masse samt termisk, kemisk, kinetisk og potentiel energi uden
  at ændre verden. Absolut tolerance er 1e-9 og relativ tolerance 1e-10.

## Solvergrænser

Alle materialedata samles i `src/materials/definitions.ts`. Solvere kan kompilere
afledte opslagstabeller for hastighed, men må ikke eje egne materialekonstanter.
Fasefamilier deles mellem deres medlemmer. Tryk flytter begge entalpiendepunkter
for fordampning sammen; en ufuldstændig latent overgang beholder sin fase.
Væskens temperatur og varmeledning læser den samme trykflyttede kurve, så vand
kan opvarmes over 100 °C i et lukket kar, indtil det lokale kogepunkt nås.
Gasfasen bruger samme trykafhængige kurve. Gastrykket løses implicit sammen med
temperaturen for damp, så genberegning ikke veksler mellem to temperaturfortolkninger.
Modellen er fortsat en tilnærmelse, ikke en komplet trykafhængig entalpitabel.

Kogning har lokal, seedet kernedannelse styret af faseovergangens
`nucleationChancePerTick`. Allerede lagret latent varme fra en celle og dens direkte
naboer af samme væske kan samles til én fuldført dampovergang. Donorer beholder
mindst deres trykkorrigerede nedre entalpiendepunkt; hver dampboble betaler hele
overgangens energikrav. Ingen masse oprettes, og ingen varme lånes gennem vægge,
diagonale hjørner eller andre materialer. Dette er en grovkornet fasefordeling,
ikke en model for bobler mindre end en celle eller en sænkning af den latente varme.
Uden denne fordeling fordampede ensartet opvarmede rækker synkront.

Hydrostatisk udligning behandler ikke en neddykket dampboble som en lavere fri
overflade. Boblen flyttes op af lokal væskefortrængning frem for at blive flyttet
direkte til toppen langs en hel udligningssti. Overfladens bevægelse og dampen i
renderingen kommer fra de faktiske celleplaceringer, ikke ekstra tegnede partikler.

Tyngde behandles før hydrostatisk udligning; frit faldende områder udlignes ikke.
Vand kan fortrænge lettere olie både nedad og langs en understøttet overflade.
Væskerne beholder deres separate materiale-ID'er, masse og energi.
Sideværts transport kræver et lavere udløb eller plads til at sænke søjlen ovenover;
lige høje parceller byttes ikke frem og tilbage. Hydrostatisk udligning omfatter
også grænser mod lettere væsker, som fortrænges tilbage langs den samme væskesti.
Den kosmetiske overfladeeffekt ændrer nu kun lys, aldrig synlig cellebesættelse.
Farvevariationen følger parcellen og ændres ikke, blot fordi den flytter vandret.

Brændende træ/planter beholder deres faste form indtil brændselslageret er brugt.
Brændende olie kan stadig flyde. Varme til nye flammer trækkes fra brændslets
termiske energi, og kemisk energi omdannes til samme mængde varme. Ilt fra kanten
kan nå sammenhængende gasområder; lukkede lommer genopfyldes ikke. Dette er en
ventilations- og forbrændingsregel, ikke en fuld kemisk arts-/ilttransportmodel.
Flammeafgivelse er en uafhængig seedet hændelse pr. brændende celle med en fælles
sandsynlighed i materialekataloget. Ensartet varme olieflader udsender derfor ikke
synkrone vandrette flammelag. En udskudt afgivelse beholder varmen i brændslet;
forbrændingsraten, iltkravet og kravet om nok termisk energi gælder fortsat.

Ildpenslen er en ekstern antændelseskilde, ikke et vedvarende brændselslager.
På brændsel tilfører den registreret varme op til antændelsestærsklen med margin.
I luft bruger den FIRE-materialets særskilte `ignitionBrush`-profil: lavere temperatur,
kort levetid og en sparsom kant omkring penslens sikre centrum. Eksisterende ild,
røg og damp overskrives ikke. Disse antændelsesparceller mærkes i `world.ignitionFlame`,
som flyttes med parcellen og nulstilles ved materialeskift. Ved slukning eller udløb
bliver de luft igen med uændret masse og entalpi, uden at skabe sod/røg. Flammer fra
brændende træ, olie og planter følger stadig den almindelige forbrændingsmodel.

Væske er en hybrid mellem et cellegitter og afledte finite-volume-søjler. Gitterets
forbindelsesgraf afgør, om bassiner hænger sammen; derfor forbindes to lige overflader
ikke gennem en væg. Modellen kan beskrive overhæng og stablede hulrum, men ikke en glat
fri overflade mindre end én celle. Viskositet dæmper parcelmomentum, og hydrostatiske
overførsler giver faktisk flytning af masse, varme og impuls.

Malede faste vægge er forankrede. Frit flydende materiale forbliver dynamisk,
når det fryser; det bliver ikke til svævende terræn. `src/physics/solid-mechanics.ts` integrerer
fortegnet hastighed i begge akser og gemmer forskydning under én celle. Den
gennemløber kollisionsceller, så hurtig bevægelse ikke springer gennem vægge.
Vedvarende støtte balancerer tyngde; kun faktisk indkommende bevægelse bliver til
kollisionsvarme. Subcellearbejde bogføres midlertidigt som termisk debitering,
indtil cellepositionens potentielle energi ændres. Dette er en numerisk
energibogføring, ikke sammenhængende, deformerbare stive legemer.

Gas bruger den ideelle gaslov og udleder tryk af masse, molarmasse, temperatur og
volumen. Sammenhængende gas i et lukket hulrum deler ét tryk ud fra alle cellernes
samlede gasmængde, temperatur og geometriske volumen. Et gasområde, der når verdens
kant, bruger stadig referencevolumen for ikke-luft-gas, mens luft bruger cellens
volumen. Opvarmet luft beholder derfor et temperaturafhængigt tryk. Denne åbne/lukkede
volumenforskel er uafklaret; der findes endnu ikke konservativ ekspansion/udstrømning. Væskecellers tryk øges
med densitet, tyngde og celledybde langs en lodret sammenhængende søjle; tilstødende
gas kan sætte randtrykket. Det er en lokal
hydrostatisk tilnærmelse, ikke en fuld trykløsning for forbundne kar. Vandets kogepunkt
kobles til trykket med en Clausius-Clapeyron-tilnærmelse omkring normalpunktet.
Tryk ved nabofladen giver parvise, modsatrettede momentumimpulser mellem gas,
væske og dynamiske faste parceller. Hastighedsændringen afhænger af parcelmassen.
Impulsen er begrænset for stabilitet, og forankrede vægge blokerer koblingen.
Dette retter asymmetrien og den ulige momentumudveksling i den gamle parregel;
det erstatter ikke en MAC-trykprojektion. Dynamiske faste parceller
bruger atmosfæretrykket som reference på den faste side af grænsen.
Hver trykimpuls er begrænset til 3 m/s² pr. nabopar, så en meget lille gasvolumen
ikke kan give ubegrænset fart på ét trin. Naboluft deler normal impuls efter masse;
tabt bevægelsesenergi bliver varme. Fri gas mister desuden 2 % hastighed pr. trin
til uopløst turbulens og modstand, med samme energioverførsel til varme.
Lodrette gradienter mellem to væskeceller anvendes ikke som ekstra impuls oven i
tyngden. Vandret væskehastighed vælger første forsøgsretning ved sideværts transport.
Den cellulære gastransport følger åbne naboceller og kan ikke krydse en fast væg.
Gasplumer genbruger frigivet luft også efter diagonale skridt. Opdrift kan fortrænge
tungere gas, der allerede er skubbet ned i samme trin. Opstigningen har en lille seedet
sideværts drift, og vandret diffusion er intermitterende; sandsynlighederne står i
materialekataloget. Dette bryder gitterlåste striber, men er ikke en turbulenssolver.
Reaktionernes markering af ny gas nulstilles før transport, så nye flammer og røg kan
stige straks uden at reagere to gange i samme trin.

Gunpowder detonerer ved sin temperaturtærskel eller ved direkte flammekontakt.
Detonation omdanner højst den lagrede kemiske energi til varme og radiale impulser.
En kort flammefront kan antænde naboceller; en forankret, sammenhængende væg
stopper fronten. Ny eksplosionsgas starter med højst én celles volumen, så dens
første tryk ikke sænkes kunstigt af øjeblikkelig fri ekspansion. Modellen løser
ikke chokbølger, fragmentering eller gasprodukternes kemi, og blastens rækkevidde
og impulshastighed er kalibrerede spilparametre.

Antændelse i et metalrør giver derfor ikke nødvendigvis en trykdrevet udstrømning
og en vandrende chokfront. Malet metal har heller ingen brudtærskel i den aktuelle
model. Disse begrænsninger behandles af den planlagte opfølgning nedenfor.

Elektricitet i `src/physics/electricity.ts` forbinder kun ortogonalt tilstødende solide ledere.
Battery har en fast 12 V-terminal, Ground 0 V, og Wire, Metal og Lamp har positive,
kalibrerede resistanser. En iterativ løsning estimerer potentialet i hver leder.
Kantstrøm følger Ohms lov med middelmodstanden fra de to celler; varme er
`(ΔV)² / R · (1/60 s)` og trækkes fra batteriets kemiske energi. Ved tomt batteri
stopper strømmen. Grænser: potentialerne er kvasistatiske, iterationerne er
begrænsede, og modellen har ikke kapacitans, induktans, gnister eller realistisk
elektrokemi. En uforbundet komponent trækker ingen energi.

De 118 grundstoffer i `src/materials/element-reference.ts` er kun navn, symbol og atomnummer.
Listen er bygget fra `periodictable` 2.1.0 og kontrolleret mod
[IUPACs periodiske tabel](https://iupac.org/what-we-do/periodic-table-of-elements/)
og [PubChem](https://pubchem.ncbi.nlm.nih.gov/docs/elements). Den tilføjer ingen
stoffysik eller reaktioner til modellen.

Syre/base-presets er vandige enkeltparceller. Hydrochloric acid 1 M og Sulfuric
acid 0.5 M repræsenterer begge 1 mol syreækvivalenter pr. liter; Sodium hydroxide
1 M repræsenterer 1 mol baseækvivalenter pr. liter. Et ortogonalt nabopar med
tilstrækkeligt kemisk energilager reagerer én gang til uspecificeret neutraliseret
opløsning. Varme beregnes som `min(n_syreeq, n_baseeq) × 57.200 J/mol`, med
mol ud fra parcelvolumen. Den anvendte stærk syre/stærk base-entalpi ligger i det
[publicerede interval ca. 57–58 kJ/mol](https://chem.libretexts.org/Bookshelves/Physical_and_Theoretical_Chemistry_Textbook_Maps/Supplemental_Modules_%28Physical_and_Theoretical_Chemistry%29/Thermodynamics/Energies_and_Potentials/Enthalpy/Enthalpy_Change_of_Neutralization).
Dette er en diskret reaktionsregel, ikke en kontinuert pH- eller diffusionsmodel.
Produktet sporer ikke særskilte ioner/salte, og delvis neutralisation, fortynding
og korrosion er endnu ikke modelleret. Svovlsyrens to syreækvivalenter er en
tilnærmelse ved den valgte koncentration.

## Reproducerbarhed og referenceforsøg

Fysisk tilfældighed og kosmetisk farvevariation har separate seedede strømme. Samme seed
og samme inputsekvens giver samme forløb. Referenceforsøgene ligger i
`tests/integration/physical-model.test.ts`, `tests/physics/reactions.test.ts`, `tests/physics/thermal.test.ts`,
`tests/physics/hydrostatics.test.ts`, `tests/integration/physics.test.ts`, `tests/physics/explosions.test.ts`
`tests/physics/electricity.test.ts` og `tests/physics/neutralization.test.ts`.
De dækker regnskaber, geometri, viskositetsrækkefølge, idealgastryk, kollision,
forbrænding, eksplosioner, diffusion og forbundne kar. En bestået suite viser, at
modellen følger disse kontrollerede regler; den beviser ikke eksperimentel nøjagtighed.

## Status efter fysikreview 21. september 2026

Regressioner i `tests/integration/review-regressions.test.ts` dækker symmetrisk tryk, ulige
masse/momentum, opadgående faste parceller, stationær kontakt uden selvopvarmning,
dragvarme, masseafhængig entalpi, dampens tryk/temperatur, undertryk under væske,
fri størkning og plantevækst med korrekt ny masse/brændsel.

`measureWorld()` giver `totalMassKg` samt et uforanderligt snapshot af kildeledgeren.
`gasMassKg` omfatter alle gasser. UI-feltet `matterMassKg` betyder stadig ikke-luft;
det overlapper gasmassen og må ikke lægges sammen med den. Brug
`compareConservation(before, after, true)` til at fratrække registrerede eksterne
masse- og energikilder. Maling registrerer termisk, kemisk, kinetisk og potentiel
energi; vækst er eksplicit tilført biomasse, ikke fotosyntese uden en energikilde.

**Arkitekturarbejde resterer:** væske og korn flyttes fortsat cellulært med
hydrostatisk udligning. Lagret hastighed er ikke en konsekvent afledt af deres
position over tid. Faldarbejde og dæmpning er bogført, men en MAC-solver med
konservativ volumen-/artstransport er ikke implementeret. Åben/lukket gasvolumen
skifter fortsat konvention; stærk kompression og varme eksplosionsrester kan give
urealistiske tryk. Testene er regressioner for bestemte fejl, ikke validering som
CFD, højtrykstermodynamik eller universel kemi. Reviewets P04 og P09 samt den
anbefalede samlede væskearkitektur er derfor ikke afsluttet.

## Planlagt modeludvikling — ikke implementeret

[Fluid- og GPU-planen](plans/fluid-gpu-redesign/plan.md) indfører først en
konservativ CPU-reference og derefter samme model på WebGPU. Den første demonstrator
til og med M3 har én væske, bæregas og faste vægge. Dens low-Mach-trykprojektion
repræsenterer ikke opløste chokbølger. Senere milepæle tilføjer termodynamik,
delvise faser, reaktioner og bevægelige faste materialer.

[Trykbølger og materialebrud](plans/pressure-waves-breakage/plan.md) er en
separat opfølgning efter M3; fragmentkoblingen afhænger også af M5. Den ændrer ikke
den første demonstrators scope. Følgende er mål for denne opfølgning:

- **Ejerskab:** vælg `lowMach` eller `compressible` ved sceneindlæsning. Hver scene
  har én autoritativ tilstand og én transportejer; CPU-reference og GPU afprøves i
  separate verdener. Den nye tilstand må ikke synkroniseres med legacy-parceller
  som et ekstra transportsystem pr. trin.
- **Gas:** kompressibel finite-volume-transport af artsmasser, impuls og total
  energi. Lokalt tryk og temperatur afledes af sammensætning, indre energi og
  tilgængeligt volumen. Ingen øjeblikkelig kammerudligning eller radiusbestemt
  impuls erstatter bølgeudbredelsen. Legacy-entalpi må ikke blot omdøbes til indre
  energi; energireferencer og konvertering følger redesignets tilstandskontrakt.
- **Tid og grænser:** akustisk CFL-styrede deltrin, refleksion ved intakte vægge
  og endelig udveksling med atmosfæren ved åbne grænser. Ugyldige deltrin prøves
  igen med kortere tid; belastning må sænke fremdriften, ikke skjult ændre tryk
  eller overskride stabilitetsgrænsen. GPU og CPU bruger samme ligninger.
- **Forbrænding:** begrænset brændselsforbrug frigiver varme og gasprodukter lokalt
  over tid. Transporten driver udstrømning og trykbølger. Blast-værktøjet bogføres
  fortsat som ekstern energi. Reaktionsparametre er dokumenterede tilnærmelser.
- **Faste materialer:** trykbelastning overføres gennem materialeforbindelser med
  elasticitet, skade og brud. Geometri, spænding og materialedata bestemmer svigt;
  absolut gastryk alene er ikke et brudkriterium. Start med metal, glas, sten og træ.
  Fragmenter bevarer masse og kobles tilbage til gassen gennem bevægelse og arbejde.
- **Regnskab:** mål indre, kemisk, kinetisk, potentiel og elastisk energi samt
  brudenergi, dissipation, randflux og understøtningsreaktioner. CPU/GPU-paritet
  måles ved samme fysiske tid med tolerancer tilpasset præcisionen.
- **Betjening:** `materialBreakage` er som standard til i understøttede scener.
  Fra stopper nye brud uden at hele skader eller fryse fragmenter. Clear/reset
  bevarer valget. Langsom gengivelse og deltrinsinspektion gør hurtige bølger
  synlige; tryk-/densitetsvisning læser fysiktilstanden.

Første kompressible scener omfatter gas, reaktive korn og faste materialer.
Kompressibel flerfasekobling med væske er en senere valideringsopgave. Promotion
kræver numeriske referenceforsøg, bevaringskontrol, scenarie- og browsertests,
GPU-recovery og målt ydelse ved 480×270. Planen er ikke en påstand om leveret
realtidshastighed eller valideret højtrykskemi. Den aktive backlog findes i
[remaining-work.md](remaining-work.md).
