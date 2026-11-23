# feature/widening-4

Voor: TASK-4 / YEL-502, mini-pc's. Gaat naar `release/widening`; staat op `feature/widening-3`
(pull request #3), want het bouwt op de producten (#1) en de meldingen (#2).

Wat het bouwt staat in `docs/phases/widening.md` (Widening 4), de keuze in `docs/decisions/0007`,
het marktonderzoek en de marktcheck in `docs/onderzoek/mini-pcs.md`.

## Stand

- Docs, plan en ontwerp (`docs/phases/widening/design/mini-pcs.png`): klaar en goedgekeurd.
- Programma: klaar. `kaartenjager selftest`: 95 controles groen, waarvan 6 nieuw.
- App: klaar. `npm run check` zonder fouten, `npm run smoke` groen, ook de volgorde op score.
- Echte ronde op Marktplaats met een losse testdatabase: 31 vondsten, 3 topvondsten.
- Twee reviewrondes. Alle punten opgelost op drie na (hieronder); de punten van de tweede ronde
  zijn niet opnieuw nagekeken.

## Reviewpunten die bewust blijven

- `stated_ram_gb` en `stated_memory_gb` lezen allebei getallen met een eenheid. Ze verschillen
  in wat ze zoeken (videogeheugen in een titel tegenover werkgeheugen in een hele advertentie,
  met keuzes, repen en opslag); één gedeelde lezer zou de regels van beide moeten dragen.
- `find_processor` maakt de tabel per advertentie opnieuw klaar. Dat is een paar dozijn korte
  strings voor de handvol computers per ronde; voorberekenen loont pas bij veel meer.
- Herbeoordelen na de beschrijving is een uitzondering voor voorlopige Vinted-computers. Pas als
  een tweede soort regel de beschrijving nodig heeft, loont het om alle Vinted-vondsten pas na
  hun beschrijving te beoordelen.

## Browsertest van alle vier de verbredingen (24 november 2026)

Op omarchy via henk (`https://kaartenjager.test`), met een echte Marktplaats-ronde als data.
Gelopen: inbox per product, mini-pc's op score, uitklappen, product uit en aan, zoekterm
toevoegen, dubbel, leeg, uitzetten en verwijderen, volgen, archiveren en terug, Instellingen,
een onbekend `?product=`, en nu zoeken. Screenshots in `docs/phases/widening/screenshots/`.

Gevonden en hersteld, elk met een controle in selftest of smoke:

- Een losse handleiding en een zwart-wit-tv werden topvondst bij de beeldbuizen: de
  voorbeeldregel sluit ze nu uit en `suspicious_below` staat op 5.
- Een processor die de advertentie noemt maar de tabel niet kent, heet nu bij naam
  ("Mini-pc, i7-6700T niet in de tabel") in plaats van "processor onbekend".
- De CPU Mark in de reden heeft een punt per duizendtal.
- Het gekozen product schuift op een smal scherm de rij in, in plaats van rechts buiten beeld.
- De melding bij aan- of uitzetten noemt het product.

Vondsten die al in een database staan, worden niet opnieuw beoordeeld; de twee valse
beeldbuisvondsten uit de demo blijven dus staan tot ze gearchiveerd worden.

## Bij het uitrollen

- Schema 5. Programma en app samen bijwerken.
- In het serverbestand een `[[product]]` Mini-pc's en de `[[computer]]`-regel uit
  `kaartenjager.example.toml`. Let op de grens van 60 paginabezoeken: de voorbeeldconfiguratie
  zit er precies op.

## Video

Geen: dit project heeft geen `video/`-map.
