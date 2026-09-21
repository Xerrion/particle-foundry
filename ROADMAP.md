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

## Fase 4: syrer og opløsninger

- Vælg konkrete syre-presets, fx saltsyre og svovlsyre, som opløsninger med
  koncentration og opløsningsmiddel; brugeren kan placere dem direkte.
- Tilføj fortynding, neutralisation, korrosion og afgrænsede reaktionsprodukter
  med stof- og energiregnskab. Test kendte støkiometriske forhold og diffusion
  gennem åbne forbindelser.

## Fase 5: grundstoffer og flere Powder Toy-materialer

- Registrér alle 118 grundstoffer med atomnummer, navn, symbol og dokumenterede
  egenskaber. Angiv eksplicit når en fysisk egenskab er ukendt eller ikke modelleret.
- Implementér udvalgte fysiske former og interaktioner som selvstændige,
  verificerede materialer; udvid iterativt til flere grundstoffer og spilmaterialer.
- Udbyg materialekategorier, værktøjer, gemte scener og en testmatrix for
  materialemøder. Antallet af katalogposter må ikke forveksles med antallet af
  realistisk modellerede reaktioner.

## Validering for hver fase

En bestået testpakke kontrollerer kun de påstande, den faktisk måler. Hver ny
mekanisme skal derfor have mindst én uafhængig reference: en analytisk ligning,
et kendt forsøgsresultat eller et dokumenteret stofregnskab. Supplér med tests
for bevarelse, væg-/kantgeometri, tærskler, seedet replay og ydelse i en fyldt
verden. Skriv modellens gyldighedsområde og kendte afvigelser i `MODEL.md`.
