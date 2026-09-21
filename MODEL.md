# Fysisk modelspecifikation

Denne fil er den autoritative beskrivelse af den fysiske model. Simulationen er en
reproducerbar, grovkornet hybridmodel; den er ikke CFD eller en sikker ingeniørmodel.

## Skala og enheder

| Størrelse | Enhed og standardværdi | Ejer |
| --- | --- | --- |
| Cellebredde | 0,01 m | `physical-scale.ts` |
| Repræsenteret dybde | 0,01 m | `physical-scale.ts` |
| Cellevolumen | 0,000001 m³ | afledt i `physical-scale.ts` |
| Fysiktrin | 1/60 s | `physical-scale.ts` og `simulation-clock.ts` |
| Masse | kg | `world.massKg` |
| Volumen | m³ | `world.volumeM3` |
| Hastighed | m/s | `world.velocityX/Y` |
| Tryk | Pa | `world.pressurePa` |
| Kemisk, kinetisk og potentiel energi | kJ | verdensfelter/`diagnostics.ts` |
| Termisk celleentalpi | J-lignende kalibrerede celleenheder | `world.energy` |

Den termiske entalpi beholder projektets hidtidige fasekurver. Diagnostikken omregner
1.000 termiske enheder til 1 kJ. Det er en bevidst kalibrering, ikke en påstand om, at
alle materialedata er laboratorieværdier. Kataloget skelner mellem SI-felter som
`densityKgPerM3` og `viscosityPas` og modellens kalibrerede egenskaber:

- `displacementDensity`: dimensionsløs rækkefølge for cellebytning, ikke SI-densitet.
- `heatTransferCoefficient`: celleentalpi pr. temperaturforskel pr. 1/60 s trin,
  yderligere begrænset af solverens stabilitetsregel; ikke W/(m K).
- `cellHeatCapacity`: celleentalpi pr. grad C, ikke varmekapacitet pr. kg.
- `lowerCellEnthalpy` / `upperCellEnthalpy`: latente endepunkter i samme celleenheder.
  Familiens koldeste fase har nul entalpi ved 0 °C; øvrige faser tilføjer latente
  bidrag. Ingen af disse størrelser omdøbes til specifikke SI-entalpier.
- `flow.interval`, levetider og reaktionsrater er kalibreret til faste 1/60 s trin.

Omdøbningen ændrer ingen værdier eller beregninger.

## Tilstand og ejerskab

`world.ts` ejer alle autoritative arrays. Et materialeparcels masse, volumen, termiske
entalpi, kemiske energi, ilt, hastighed og øvrige metadata flyttes samlet ved `swap()`.
Solvers må have scratch-arrays, men må ikke spejle en anden autoritativ væskemængde.

En nymalet celle starter som ét fuldt cellevolumen. Ved faseændring bevares masse og
entalpi; volumen genberegnes fra den nye fases densitet. Den cellulære geometri kan kun
placere ét parcel pr. celle, så kraftig ekspansion repræsenteres som komprimeret volumen
og gastryk frem for at skabe skjulte celler. Væskesøjler i `fluid-solver.ts` er afledte
målinger af parcelvolumen og ejer ikke væske.

## Bevarelse og åbne systemer

- Passive swaps, hydrostatisk transport og fase-ID-skift flytter hele tilstanden og
  må ikke skabe eller slette masse eller termisk entalpi.
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
- Røg, der forlader modellen ved udløb, registreres som massetab. Verdens kant er åben
  atmosfære for ilt; indvendige, lukkede hulrum har kun deres lagrede ilt.
- `diagnostics.ts` opgør masse samt termisk, kemisk, kinetisk og potentiel energi uden
  at ændre verden. Absolut tolerance er 1e-9 og relativ tolerance 1e-10.

## Solvergrænser

Alle materialedata samles i `material-definitions.ts`. Solvere kan kompilere
afledte opslagstabeller for hastighed, men må ikke eje egne materialekonstanter.
Fasefamilier deles mellem deres medlemmer. Tryk flytter begge entalpiendepunkter
for fordampning sammen; en ufuldstændig latent overgang beholder sin fase.
Temperaturkurven er fortsat kalibreret ved normaltryk, så trykkoblingen er en
tilnærmelse og ikke en komplet trykafhængig entalpitabel.

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

Faste vægge er forankrede. Kun celler, der udtrykkeligt markeres dynamiske, behandles af
`solid-mechanics.ts`; det forhindrer malede kar i at falde fra hinanden. Dynamiske faste
parceller har tyngde, densitetsbaseret opdrift, kollision og dissipativt energitab. De er
endnu ikke deformerbare eller brudbare sammenhængende legemer.

Gas bruger den ideelle gaslov og udleder tryk af masse, molarmasse, temperatur og
volumen. Væskecellers tryk øges med densitet, tyngde og celledybde langs en lodret
sammenhængende søjle; tilstødende gas kan sætte randtrykket. Det er en lokal
hydrostatisk tilnærmelse, ikke en fuld trykløsning for forbundne kar. Vandets kogepunkt
kobles til trykket med en Clausius-Clapeyron-tilnærmelse omkring normalpunktet.
Trykgradienter giver vandrette og lodrette impulser mellem gas, væske og dynamiske
faste parceller; forankrede vægge blokerer koblingen. Dynamiske faste parceller
bruger atmosfæretrykket som reference på den faste side af grænsen.
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

Elektricitet i `electricity.ts` forbinder kun ortogonalt tilstødende solide ledere.
Battery har en fast 12 V-terminal, Ground 0 V, og Wire, Metal og Lamp har positive,
kalibrerede resistanser. En iterativ løsning estimerer potentialet i hver leder.
Kantstrøm følger Ohms lov med middelmodstanden fra de to celler; varme er
`(ΔV)² / R · (1/60 s)` og trækkes fra batteriets kemiske energi. Ved tomt batteri
stopper strømmen. Grænser: potentialerne er kvasistatiske, iterationerne er
begrænsede, og modellen har ikke kapacitans, induktans, gnister eller realistisk
elektrokemi. En uforbundet komponent trækker ingen energi.

De 118 grundstoffer i `element-reference.ts` er kun navn, symbol og atomnummer.
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
`tests/physical-model.test.ts`, `tests/reactions.test.ts`, `tests/thermal.test.ts`,
`tests/hydrostatics.test.ts`, `tests/physics.test.ts`, `tests/explosions.test.ts`
`tests/electricity.test.ts` og `tests/neutralization.test.ts`.
De dækker regnskaber, geometri, viskositetsrækkefølge, idealgastryk, kollision,
forbrænding, eksplosioner, diffusion og forbundne kar. En bestået suite viser, at
modellen følger disse kontrollerede regler; den beviser ikke eksperimentel nøjagtighed.
