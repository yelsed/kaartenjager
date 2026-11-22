# feature/widening-1

Voor: TASK-1 / YEL-503, zoeken per product. Gaat naar `release/widening`.

Wat deze branch bouwt staat in `docs/phases/widening.md` (Widening 1), de keuzes in
`docs/decisions/0001` tot en met `0003`. Het onderzoek voor Widening 4 (mini-pc's) staat ook op
deze branch, in `docs/onderzoek/`.

## Stand

- Docs en plan: klaar.
- Programma: klaar. `kaartenjager selftest`: 77 controles groen, waarvan 7 nieuw voor producten.
- App: klaar. `npm run check` zonder fouten, `npm run smoke` groen, ook voor producten aan en uit
  en de inbox per product. Op telefoonbreedte bekeken: inbox met keuzebalk, Producten,
  Zoektermen per product.
- Voorbeeldconfiguratie, README, CHANGELOG en `docs/database.md` bijgewerkt.

## Bij het uitrollen

- Programma en app samen bijwerken: het programma migreert naar schema 3, en de oude app
  weigert dat schema.
- Het bestand op de server heeft geen `[[product]]`. Dat blijft werken (Videokaarten plus een
  product per onderdeel), maar de beeldbuis-regel heeft dan geen `show_without_distance`; zet
  die erbij om Vinted-beeldbuizen te zien, en `notify = true` op een `[[product]]`-blok om
  beeldbuis-topvondsten te melden. Zonder blok meldt alleen Videokaarten.
