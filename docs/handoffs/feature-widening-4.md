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

## Bij het uitrollen

- Schema 5. Programma en app samen bijwerken.
- In het serverbestand een `[[product]]` Mini-pc's en de `[[computer]]`-regel uit
  `kaartenjager.example.toml`. Let op de grens van 60 paginabezoeken: de voorbeeldconfiguratie
  zit er precies op.

## Video

Geen: dit project heeft geen `video/`-map.
