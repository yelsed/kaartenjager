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

## Bij het uitrollen

- Schema 5. Programma en app samen bijwerken.
- In het serverbestand een `[[product]]` Mini-pc's en de `[[computer]]`-regel uit
  `kaartenjager.example.toml`. Let op de grens van 60 paginabezoeken: de voorbeeldconfiguratie
  zit er precies op.

## Video

Geen: dit project heeft geen `video/`-map.
