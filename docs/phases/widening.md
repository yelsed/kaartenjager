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

- [ ] Een configuratie zonder `[[product]]` levert Videokaarten plus één product per onderdeel
- [ ] Een configuratie met `[[product]]` koppelt elke regel aan zijn product, en `check` weigert
      een regel die naar een onbekend product wijst
- [ ] Een beeldbuis van Marktplaats binnen bereik is een topvondst
- [ ] Een beeldbuis van Vinted zonder afstand is een goede deal met een waarschuwing
- [ ] Een beeldbuis van Marktplaats buiten bereik valt af
- [ ] Een topvondst van een product dat meldt gaat naar Discord; van een product dat niet meldt niet
- [ ] Kaarten melden zoals voorheen (percentage onder de markt)
- [ ] Een database van schema 2 migreert naar 3; bestaande zoektermen en vondsten krijgen hun product
- [ ] Een nieuw product in de configuratie krijgt zijn zoektermen één keer in de database
- [ ] Een uitgezet product wordt niet afgezocht en meldt niets
- [ ] De app toont de inbox per product en zet producten aan en uit
- [ ] De bestaande 70 zelftestcontroles blijven groen

### Wat het niet doet

- Geen nieuwe meldkanalen; dat is Widening 2. Topvondsten gaan via de bestaande Discord-route.
- Geen mini-pc-regels; dat is Widening 4, op deze productlaag.
- Volglijst en archief krijgen nog geen productfilter; de inbox is waar je focust.
