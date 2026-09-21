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
volumen. Vandets kogepunkt kobles til trykket med en Clausius-Clapeyron-tilnærmelse
omkring normalpunktet. Trykgradienter giver impulser til gas, væske og dynamiske faste
celler. Den cellulære gastransport følger åbne naboceller og kan ikke krydse en fast væg.
Gasplumer genbruger frigivet luft også efter diagonale skridt. Opdrift kan fortrænge
tungere gas, der allerede er skubbet ned i samme trin. Opstigningen har en lille seedet
sideværts drift, og vandret diffusion er intermitterende; sandsynlighederne står i
materialekataloget. Dette bryder gitterlåste striber, men er ikke en turbulenssolver.
Reaktionernes markering af ny gas nulstilles før transport, så nye flammer og røg kan
stige straks uden at reagere to gange i samme trin.

## Reproducerbarhed og referenceforsøg

Fysisk tilfældighed og kosmetisk farvevariation har separate seedede strømme. Samme seed
og samme inputsekvens giver samme forløb. Referenceforsøgene ligger i
`tests/physical-model.test.ts`, `tests/reactions.test.ts`, `tests/thermal.test.ts`,
`tests/hydrostatics.test.ts` og `tests/physics.test.ts`. De dækker regnskaber, geometri,
viskositetsrækkefølge, idealgastryk, kollision, forbrænding, diffusion og forbundne kar.
