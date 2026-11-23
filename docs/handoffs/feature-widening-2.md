# feature/widening-2

Voor: TASK-2 / YEL-504, snel blijven en meldingen op de telefoon. Gaat naar `release/widening`.

Deze branch staat op `feature/widening-1` (pull request #1), want de meldrempel per product heeft
de producten nodig. Na de merge van #1 wordt hij op `release/widening` gezet.

Wat hij bouwt staat in `docs/phases/widening.md` (Widening 2), de keuzes in `docs/decisions/0004`
en `0005`.

## Stand

- Docs, plan en ontwerp (`docs/phases/widening/design/`): klaar en goedgekeurd.
- Programma: klaar. `kaartenjager selftest`: 86 controles groen, waarvan 9 nieuw.
- App: klaar. `npm run check` zonder fouten, `npm run smoke` groen, ook voor de rondeduur.
- Twee reviewrondes, alle punten opgelost op één na (hieronder).
- Nog niet gedaan: de test op de telefoon van de eigenaar. Daarvoor zijn de webhook-URL en zijn
  Discord-gebruikers-ID nodig.

## Bij het uitrollen

- Schema 4. Programma en app samen bijwerken. De overgang stempelt alle vondsten die nog geen
  melding hadden, zodat de eerste ronde niet de hele achterstand post.
- In het serverbestand onder `[notify]`: `discord_webhook` en `mention_user_id`. Zodra de webhook
  er staat drukt de ronde geen vondsten meer af, dus Hermes post ze niet meer dubbel.
- `notify_deals = true` op het product Beeldbuistelevisies, als de eigenaar daar stille deals wil.

## Reviewpunt dat bewust blijft

Een vondst houdt het niveau dat de prijstabel gaf bij de laatste beoordeling. Verhoog je de
drempel, dan kan een advertentie die deze ronde niet in de zoekresultaten staat en niet van prijs
veranderde nog één keer luid melden op de oude drempel. Elke advertentie wordt opnieuw beoordeeld
zodra hij in de zoekresultaten staat of van prijs verandert, dus dat duurt hooguit tot de
volgende keer dat hij langskomt. De drempel opnieuw in de query toepassen zou de regel op twee
plekken zetten, en dat vond de eerste review juist een fout.

## Video

Geen: dit project heeft geen `video/`-map.
