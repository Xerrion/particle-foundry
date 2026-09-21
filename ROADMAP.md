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

## Fase 2: katalog og betjening til mange materialer

- Udvid materiale-ID'er ud over `Uint8Array`-grænsen på 256 værdier og migrér
  eventuelle gemte scener. Hold værktøjs-ID'er adskilt fra stof-ID'er.
- Tilføj kategorier, søgning, favoritter og beskrivelser, så hundredvis af poster
  kan vælges uden en uoverskuelig knapvæg.
- Indfør maskinlæsbare datakilder og validering af enheder, manglende værdier,
  faseforbindelser og dublerede identiteter.

## Fase 3: elektricitet

- Ledningsevne, isolatorer, energikilder, kontakter og en begrænset
  ladnings-/strømmodel. Elektrisk energi, varme og gnister skal stå i regnskabet.
- Test åbne og lukkede kredsløb, polaritet hvor relevant, isolering gennem vægge,
  energioverførsel og reproducerbarhed mod selvstændige referenceberegninger.

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
