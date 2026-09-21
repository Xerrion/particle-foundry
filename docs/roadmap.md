# Udvidelsesplan

Målet er en stor, brugbar sandkasse med både Powder Toy-lignende materialer og
værktøjer samt de 118 kemiske grundstoffer. Et grundstof i et opslagsregister er
ikke automatisk en valideret fysik- eller reaktionsmodel: hvert simuleret materiale
skal have tydelig tilstand, kilder til data og afgrænsede regler.

## Fase 1: eksplosioner og forbrænding — første del leveret

- Gunpowder med begrænset kemisk energi, temperatur- og flammeantændelse.
- Blast-værktøj med registreret ekstern energi, flammefront, tryk og radial fart.
- Vægge blokerer blastens udbredelse. Tests måler energiregnskab, tærskel,
  flammekæde, tryk og geometri.
- Næste trin i samme område: flere brændstoffer og oxidanter, røg/ild-produkter,
  temperaturafhængige rater og scenarier til visuel sammenligning.
- Opløste chokbølger og materialebrud er planlagt som opfølgning på
  GPU-demonstratoren; de er ikke leveret af den nuværende blastmodel.

## Fase 2: katalog og betjening til mange materialer — første del leveret

- Udvid materiale-ID'er ud over `Uint8Array`-grænsen på 256 værdier og migrér
  eventuelle gemte scener. Hold værktøjs-ID'er adskilt fra stof-ID'er.
- Kategorier og søgning for malbare materialer og værktøjer er tilføjet. Favoritter,
  beskrivelser og flere materialer følger senere.
- En søgbar tabel med alle 118 navne, symboler og atomnumre er leveret som
  reference. Ingen af posterne tæller som en implementeret simulering endnu.
- Indfør maskinlæsbare datakilder og validering af enheder, manglende værdier,
  faseforbindelser og dublerede identiteter.

## Fase 3: elektricitet — første del leveret

- Battery, Ground, Wire, Lamp og Metal danner et resistivt netværk over fire
  naboretninger. Batteriets finite kemiske lager finansierer Joule-varmen.
  Et åbent kredsløb, isolering, batteritømning og et lukket kredsløb testes mod
  Ohms lov og et selvstændigt energiregnskab.
- Kontakter, gnister, polaritet, længerevarende dynamisk kredsløbsrespons og
  flere energikilder mangler endnu.

## Fase 4: syrer og opløsninger — første del leveret

- Saltsyre 1 M og svovlsyre 0,5 M kan males direkte som vandige opløsninger.
  Natriumhydroxid 1 M er et base-preset. Kontakt neutraliserer én celle af hver
  og danner to celler af en uspecificeret neutraliseret opløsning.
- Reaktionsvarmen følger syre/base-ækvivalenter og et dokumenteret ca. 57,2 kJ/mol
  niveau. Tests kontrollerer den uafhængige beregning, masse, energi, isolering
  og engangsreaktion.
- Fortynding, delvis koncentration, korrosion, specifikke salte og yderligere
  syre/base-par kræver en senere koncentrations- og stofmængdemodel.

## Fase 5: grundstoffer og flere Powder Toy-materialer

- Registrér alle 118 grundstoffer med atomnummer, navn, symbol og dokumenterede
  egenskaber. Angiv eksplicit når en fysisk egenskab er ukendt eller ikke modelleret.
- Implementér udvalgte fysiske former og interaktioner som selvstændige,
  verificerede materialer; udvid iterativt til flere grundstoffer og spilmaterialer.
- Udbyg materialekategorier, værktøjer, gemte scener og en testmatrix for
  materialemøder. Antallet af katalogposter må ikke forveksles med antallet af
  realistisk modellerede reaktioner.

## Fysik- og GPU-redesign — planlagt

Den [samlede implementeringsplan](plans/fluid-gpu-redesign/plan.md) beskriver
en konservativ CPU-reference efterfulgt af samme ligninger på WebGPU. Den
eksisterende sandkasse bevares, indtil understøttede scener består numeriske,
funktionelle og målte ydelseskriterier.

- M0–M1: reproducerbar baseline, backendgrænse, versioneret tilstand og kommandoer.
- M2–M3: konservativ væske/gas-reference og første GPU-demonstrator med direkte
  rendering. Demonstratoren omfatter én væske, bæregas og faste vægge; den omfatter
  ikke eksplosioner, chokbølger, faseændringer eller bevægelige faste legemer.
- M4–M6: termodynamik, endelig udluftning, delvise faser, korn, bevægelige faste
  materialer, reaktioner og produktintegration.
- M7: validering, måling på faktiske enheder, gendannelse og gradvis overgang.

GPU-portering alene ændrer ikke modellens fysiske gyldighedsområde. Den planlagte
low-Mach-model løser ikke chokbølger.

## Trykbølger og materialebrud — planlagt opfølgning

[Opfølgningsplanen](plans/pressure-waves-breakage/plan.md) starter efter M3
uden at udvide den første GPU-demonstrator. Bevægelige fragmenter afhænger også af
M5's kobling mellem faste materialer og fluid.

- Tilføj en særskilt kompressibel model med CPU-reference og GPU-paritet.
  Lokal kompression, ekspansion, refleksion og udstrømning skal følge konservative
  fluxer og stabile akustiske deltrin.
- Antændelse omsætter et begrænset brændselslager til varme og gasprodukter over
  tid. Gastryk skal drive bevægelsen gennem åbninger og videre ud i omgivelserne.
- Materialebrud afhænger af spændinger, geometri og materialedata. Start med metal,
  glas, sten og træ; fragmenter bevarer masse og indgår i energi-/impulsregnskabet.
- Indstillingen **Material breakage** er som standard slået til i understøttede
  scener. Deaktivering forhindrer nye brud uden at reparere tidligere skader;
  clear/reset bevarer indstillingen.
- Tilføj langsom gengivelse og inspektion af deltrin. Fysisk korrekt tidsintegration
  prioriteres over realtid; GPU-ydelse skal måles, ikke antages.
- Første kompressible scener omfatter gas, reaktive korn og faste materialer.
  Væskeholdige scener afventer særskilt valideret kompressibel flerfasekobling.

Aktuelle opgaver og leveret historik findes i [remaining-work.md](remaining-work.md).

## Validering for hver fase

En bestået testpakke kontrollerer kun de påstande, den faktisk måler. Hver ny
mekanisme skal derfor have mindst én uafhængig reference: en analytisk ligning,
et kendt forsøgsresultat eller et dokumenteret stofregnskab. Supplér med tests
for bevarelse, væg-/kantgeometri, tærskler, seedet replay og ydelse i en fyldt
verden. Skriv modellens gyldighedsområde og kendte afvigelser i `docs/model.md`.
