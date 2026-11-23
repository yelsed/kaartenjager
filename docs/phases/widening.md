# Fase: verbreden

Kaartenjager zoekt niet langer naar één groeiende lijst losse dingen, maar naar **producten**:
videokaarten, voedingen, risers, beeldbuistelevisies, en straks mini-pc's. Elk product heeft zijn
eigen zoektermen, zijn eigen regels en prijzen, en in de app zijn eigen overzicht.

Tickets in deze fase (Paperclip project Kaartenjager, backlog in de servercheckout op paperclip):

| Taak | Ticket | Wat |
|---|---|---|
| TASK-1 | YEL-503 | Zoeken per product, en per product zien wat er gevonden is |
| TASK-2 | YEL-504 | Snel blijven, en echte meldingen op de telefoon |
| TASK-3 | YEL-501 | Hosting naast Hermes herzien |
| TASK-4 | YEL-502 | Mini-pc's als product; onderzoek staat in `docs/onderzoek/` |

Release-branch: `release/widening`. Elke taak een eigen `feature/widening-<n>`.

## Widening 1 (TASK-1): producten

### Wat het bouwt

- `[[product]]` in de configuratie: een id, een naam, de zoektermen waarmee hij begint, en of
  zijn topvondsten een melding opleveren. Elke `[[card]]` en `[[part]]` hoort bij één product.
- Een configuratie zonder `[[product]]` blijft werken: de kaarten worden samen het product
  "Videokaarten", elk onderdeel wordt een eigen product (ADR 0001).
- Twee niveaus per vondst: **topvondst** en **goede deal** (ADR 0002). Topvondsten van een product
  dat meldt gaan naar Discord; de rest staat alleen in de app.
- Beeldbuistelevisies: binnen 45 km en onder € 150 is een topvondst die meldt. Van Vinted, waar
  geen afstand bekend is, verschijnt hij als goede deal met de waarschuwing dat de afstand
  onbekend is. Op Marktplaats verder dan 45 km blijft hij weg.
- Voedingen en risers zijn elk een eigen product (antwoord van de eigenaar, 22 november 2026).
- Producten staan in de database met een aan/uit-stand. Uit betekent: zijn zoektermen worden niet
  afgezocht, hij meldt niets en zijn vondsten staan niet in de inbox (ADR 0003).
- De app: in de inbox een keuzebalk per product met het aantal vondsten, een pagina Producten om
  producten aan en uit te zetten, en zoektermen die per product gegroepeerd zijn en bij het
  toevoegen een product krijgen.
- Schema 3 (`docs/database.md`).

### Checklist (elke regel heeft een controle in `kaartenjager selftest` of in `app/smoke.sh`)

- [x] Een configuratie zonder `[[product]]` levert Videokaarten plus één product per onderdeel
- [x] Een configuratie met `[[product]]` koppelt elke regel aan zijn product, en `check` weigert
      een regel die naar een onbekend product wijst
- [x] Een beeldbuis van Marktplaats binnen bereik is een topvondst
- [x] Een beeldbuis van Vinted zonder afstand is een goede deal met een waarschuwing
- [x] Een beeldbuis van Marktplaats buiten bereik valt af
- [x] Een topvondst van een product dat meldt gaat naar Discord; van een product dat niet meldt niet
- [x] Kaarten melden zoals voorheen (percentage onder de markt)
- [x] Een database van schema 2 migreert naar 3; bestaande zoektermen en vondsten krijgen hun product
- [x] Een nieuw product in de configuratie krijgt zijn zoektermen één keer in de database
- [x] Een uitgezet product wordt niet afgezocht en meldt niets
- [x] De app toont de inbox per product en zet producten aan en uit
- [x] De bestaande 70 zelftestcontroles blijven groen (77 met de nieuwe)

### Wat het niet doet

- Geen nieuwe meldkanalen; dat is Widening 2. Topvondsten gaan via de bestaande Discord-route.
- Geen mini-pc-regels; dat is Widening 4, op deze productlaag.
- Volglijst en archief krijgen nog geen productfilter; de inbox is waar je focust.

## Widening 2 (TASK-2): snel blijven, en meldingen op de telefoon

### Wat het bouwt

- Vondsten gaan via een eigen Discord-webhook, niet meer via de cron-uitvoer en Hermes (ADR 0004).
  Discord blijft het enige kanaal (antwoord van de eigenaar, 23 november 2026).
  - Een topvondst is luid: het bericht noemt de eigenaar (`mention_user_id`), dus de telefoon
    trilt ook als het kanaal op "alleen vermeldingen" staat.
  - Een goede deal is stil: hij staat in het kanaal met Discords stille vlag, zonder melding.
  - Zonder webhook in de configuratie werkt alles zoals nu: de ronde drukt de topvondsten af en
    Hermes post ze.
- Elk product kiest zijn eigen drempel (ADR 0005):
  - `notify`: topvondsten melden, luid (bestond al).
  - `notify_deals`: goede deals stil in het kanaal.
  - `push_below_market_percent`: hoeveel onder de markt een kaart van dit product een topvondst
    is; zonder waarde geldt `[notify]`.
- Snelheid: elke ronde schrijft hoe lang hij duurde (`last_round_seconds`). Het budget: een ronde
  duurt hooguit de helft van het gat tussen twee rondes, dus 2,5 minuut bij rondes van vijf
  minuten. Duurt hij langer, dan zegt de app dat bij de hartslag.

### Checklist (elke regel heeft een controle in `kaartenjager selftest` of in `app/smoke.sh`)

- [ ] Met een webhook gaat een topvondst als bericht met vermelding naar de webhook, en de ronde
      drukt hem niet meer af
- [ ] Een goede deal van een product met `notify_deals` gaat stil naar de webhook (vlag 4096),
      zonder vermelding
- [ ] Een goede deal van een product zonder `notify_deals` gaat nergens heen
- [ ] Zonder webhook drukt de ronde de topvondsten af zoals voorheen
- [ ] Een mislukte webhook laat de vondst ongemeld, zodat de volgende ronde het opnieuw probeert
- [ ] `push_below_market_percent` op een product gaat voor op die in `[notify]`
- [ ] `check` toont per product of en hoe het meldt
- [ ] Een ronde schrijft `last_round_seconds`, en de app waarschuwt als die over het budget gaat
- [ ] Rondeduur gemeten voor en na Widening 1 en 2, met de uitkomst in dit document
- [ ] Op de telefoon van de eigenaar getest: een topvondst trilt, een deal niet

### Wat het niet doet

- Geen ntfy, Pushover of mail: de eigenaar koos Discord.
- Hermes blijft voor de knop "Hermes laten kijken" en de prijsherziening; of Hermes blijft is
  Widening 3.
- Geen stille uren; Discord regelt dat op de telefoon zelf.
