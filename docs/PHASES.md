# Faser og milepæle

P1 flytter hele den nuværende sandbox til Rust/GPU.
Målet er 60 viste FPS, simulering i realtid, hurtige værktøjer og fysisk troværdig adfærd.
[Det fælles spilmål](plans/fluid-gpu-redesign/plan.md#final-gameplay-target-clarified-on-13-october-2026)
fastlægger de målinger, der skal bevise resultatet.

Se [STATUS.md](STATUS.md) for den valgte opgave og seneste fremdrift.
Brug [håndbogen](README.md), når du skal fortsætte arbejdet.
[Roadmap](roadmap.md) beskriver den tekniske rækkefølge og afhængighederne.

## Hvad betyder P, M og E?

| Navn | Betydning                                        | Eksempel                                           |
| ---- | ------------------------------------------------ | -------------------------------------------------- |
| P    | Fase: et større samlet resultat                  | P1 flytter den nuværende sandbox til den nye motor |
| M    | Milepæl: et afgrænset resultat i motorarbejdet   | M3 viser en lille GPU-scene i browseren            |
| E    | Engineering item: en konkret teknisk opgave i P1 | E08 forbinder GPU-scenen med browseren             |

`P1 / M3 / E08` angiver dermed fase, milepæl og opgave.
Det er projektets egne navne. De følger ikke en særlig Linear- eller Jira-standard.

Andre faser har egne bogstaver for milepæle: W for trykbølger, C for materialer og kemi,
N for atomkerner og Q for produktkontrol. De 35 milepæle forklares nedenfor.
En milepæl er først færdig, når dens krav og dokumenterede kontroller består.

## De tre checkpoints i P1

1. Bevis en lille GPU-scene i M2-M3. Væske, gas, fysik og tegning skal fungere i browseren.
2. Flyt hele den nuværende sandbox i M4-M6. Det omfatter materialer, varme, sand, reaktioner, kredsløb, scener og værktøjer.
3. Bevis standardskiftet i M7. Kontroller alle funktioner, ydelse på rigtige enheder, gemte tilstande og genopretning samlet.

Den gamle motor forbliver tilgængelig, mens erstatningen udvikles og testes.
En hurtig prøvescene afslutter kun sit eget checkpoint.
[Listen over nuværende funktioner](plans/rust-wasm-migration/current-sandbox-scope.md)
fastlægger alt, der skal fungere før standardskiftet.

## P1: Flyt den nuværende sandbox

Browserens interface og nyttige funktioner bevares.
Hver scene har én motor, der ejer dens tilstand.
Den nye fysik skal bevare stof og energi. Kendte fejl i den gamle motor skal rettes.
[Den tekniske migrationsplan](plans/rust-wasm-migration/plan.md) fastlægger E00-E14.

### Grundlag: M0-M1

- M0 registrerer udgangspunktet. Eksisterende adfærd og kontroller giver et grundlag for at vurdere ændringer.
- M1 forbinder Rust og browseren. Den fastlægger materialer, kommandoer, resultater og en selvstændig reference for kredsløb.

### Checkpoint 1: M2-M3

- M2 beviser væskefysikken på CPU'en. En præcis Rust-model skal håndtere tryk og transport uden at miste stof.
- M3 viser GPU-scenen i browseren. Den bruger en begrænset væske- og gasmodel og tegner direkte fra GPU'ens tilstand.

### Checkpoint 2: M4-M6

- M4 flytter varme og faseændringer. Vand, damp, luft, smeltning, frysning og kogning skal have korrekte stof- og energiregnskaber.
- M5 tilføjer sand og bevægelige faste stoffer. Korn, forhindringer og væsker skal kunne påvirke hinanden uden tab af stof.
- M6 samler de nuværende reaktioner, rettet brand, kredsløb, værktøjer, kontroller og scener.

M6 omfatter de otte nuværende grundstofmodeller: brint, helium, kulstof, kvælstof,
ilt, svovl, jern og kobber. Deres første migration til Rust/GPU hører til P1.
Den bredere materialemodel i P4 er en senere udvidelse.

### Checkpoint 3: M7

M7 kontrollerer hele den nuværende sandbox samlet.
Alle krævede funktioner, browserkontroller, gemte tilstande, genopretning og ydelsesmål skal bestå.
Resultatet skal vise 60 FPS under normalt spil, simulering i realtid og fysisk troværdig adfærd.

Efter M7 skal resultatet vurderes. De næste faser kræver en særskilt beslutning.
Læs [acceptkravene](validation/backend-migration.md) for de præcise krav til standardskiftet.

## P2-P9: Senere udvidelser

Disse faser er udskudt. Deres afhængigheder giver ikke tilladelse til at begynde arbejdet.
Små forudsætninger for at flytte en eksisterende funktion hører fortsat til P1.

### P2: Trykbølger og brud

P2 tilføjer en særskilt model for trykbølger, eksplosioner med begrænset brændstof og faste stoffer, der kan brydes.
[Planen for trykbølger og brud](plans/pressure-waves-breakage/plan.md) fastlægger kravene.

- W0 fastlægger kontroller og understøttede scener.
- W1 beviser gasbølger og refleksioner ved vægge i en Rust-model på CPU'en.
- W2 gentager modellen på GPU'en og i browseren.
- W3 tilføjer reaktioner med begrænsede ressourcer, skader og bevægelige fragmenter.
- W4 kontrollerer komplette scener, indstillinger, gemte tilstande og ydelse før udgivelse.

### P3: Et bredere datagrundlag for stof

Kataloget kan navngive 118 grundstoffer. Hvert stof kræver stadig data og en testet model, før sandboxen kan simulere det.
[Materialeplanen](plans/materials-and-chemistry/plan.md) udvider data og reaktionssystemer ud over P1's behov.

- C0 opbygger et kontrolleret katalog over grundstoffer, stofformer og egenskaber med kilder og gyldighedsområder.
- C1 udvider registreringen af sammensætning, så flere stoffer kan blandes, smelte, deles og gemmes korrekt.
- C2 udvider reaktionsmotoren med regler, der bruger tilgængelige ingredienser og gør rede for energien.

### P4: 40 grundstoffer og nødvendige forbindelser

Hver af de 40 identiteter skal have mindst én testet stofform inden for et angivet gyldighedsområde.
[Materialeplanen](plans/materials-and-chemistry/plan.md) fastlægger formerne og deres krav.

- C3 kontrollerer P1's otte grundstofmodeller i det bredere datasystem.
- C4 tilføjer de øvrige grundstoffer i kernegruppen og udvalgte forbindelser med vand, luft, mineraler og brændstof.
- C5 tilføjer udvalgte reaktioner og opløsninger, herunder begrænset oxidation og syre/base-adfærd.
- C6 kontrollerer alle 40 identiteter i scener med synlige begrænsninger og kilder.

### P5: 83 grundstoffer

P5 tilføjer præcis 43 identiteter til de første 40.
Fælles fysiske systemer skal understøtte dem, hvor modeller og data passer.
[Materialeplanen](plans/materials-and-chemistry/plan.md) ejer den præcise afgrænsning.

- C7 tilføjer den fastlagte gruppe med 43 grundstoffer.
- C8 tilføjer udvalgte legeringer, belægninger, halvledere og andre testede egenskaber.
- C9 kontrollerer de 83 identiteter, blandinger, ydelse og synlige modelgrænser.

### P6: Henfald og stråling

P6 tilføjer atomkerneadfærd for udvalgte isotoper og 20 nye grundstofidentiteter.
Det giver 103 identiteter. Nogle findes først som isotoper og har endnu ingen almindelig stofform i sandboxen.
[Planen for atomkernefysik](plans/nuclear-simulation/plan.md) fastlægger kravene.

- N0 importerer og kontrollerer data om isotoper og henfald.
- N1 beviser henfaldskæder og datterprodukter i en CPU-reference.
- N2 gør rede for stråling, der bevæger sig, afgiver energi eller forlader scenen.
- N3 tilføjer de 20 identiteter og brugbare kontroller for isotoper.

### P7: Udvalgt fusion og fission

P7 modellerer bestemte reaktioner med kendte udgangsstoffer, produkter og energiregnskaber.
[Planen for atomkernefysik](plans/nuclear-simulation/plan.md) beskriver det begrænsede anvendelsesområde.

- N4 kontrollerer et lille netværk af fusionsreaktioner.
- N5 tilføjer udvalgt fission og neutronreaktioner for bestemte isotoper.
- N6 forbinder godkendte reaktioner med understøttede scener og kontrollerer energi, genopretning og modelgrænser.

### P8: Alle 118 identiteter

P8 tilføjer de sidste 15 supertunge grundstoffer.
Ukendte egenskaber skal stå som ukendte.
[Planen for atomkernefysik](plans/nuclear-simulation/plan.md) fastlægger kravene.

- N7 tilføjer de sidste identiteter med synlig status for data og understøttelse.
- N8 tilføjer valgfri kreativ stofadfærd, som tydeligt skelnes fra målte egenskaber og gemmes med scenen.

### P9: Kontroller det udvidede produkt

P9 kontrollerer kombinationerne af de senere motortilstande, materialer, reaktioner og indstillinger.
[Acceptkravene](validation/acceptance.md) gælder for den udvidelse.
Den nuværende sandbox skal allerede bestå sine brugbarheds-, genopretnings- og ydelseskrav i P1.

- Q0 kontrollerer hvilke kombinationer af funktioner der kan understøttes sikkert og korrekt.
- Q1 færdiggør brugeroplevelsen, gemte tilstande, genopretning, forklaringer, kontroller og vejledende scener.
- Q2 samler målinger fra numeriske kontroller, browsere og enheder samt dokumentation, der svarer til resultaterne.

## Hvad tæller 40, 83, 103 og 118?

Tallene tæller grundstofidentiteter ved hvert mål.
De tæller ikke alle isotoper, alle reaktioner eller fuldt realistiske stoffer.
Hver fase skal opfylde sine egne acceptkrav for de scener, den understøtter.
[Grundstofoversigten](elements.md) ejer de præcise grupper.
