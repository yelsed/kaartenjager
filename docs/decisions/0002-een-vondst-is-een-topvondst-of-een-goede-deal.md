# 0002 Een vondst is een topvondst of een goede deal

Datum: 23 november 2026. Taak: TASK-1 / YEL-503.

## Besluit

Elke vondst krijgt een niveau, opgeslagen in `finding.level`:

- **top**: het soort vondst waarvoor je gestoord wilt worden.
  - Kaart: minstens `push_below_market_percent` onder de markt en niet onder `suspicious_below`.
  - Onderdeel: onder `alert_below`, niet onder `suspicious_below`, en, als de regel op afstand
    zeeft, binnen `max_pickup_km`.
- **deal**: de rest van wat de prijstabel de moeite waard vindt.

Een product met `notify = true` meldt zijn topvondsten in Discord. Kaarten melden zoals
voorheen; onderdelen melden alleen als hun product dat zegt.

Een onderdeelregel met `show_without_distance = true` gooit een advertentie zonder afstand niet
meer weg, maar toont hem als deal met de waarschuwing dat de afstand onbekend is.

## Waarom

De beeldbuistelevisies leken leeg. Vinted geeft nooit een afstand, en de regel gooide alles
zonder afstand weg: elke Vinted-beeldbuis verdween. De eigenaar koos (22 november 2026): de grens
blijft € 150, binnen 45 km is een topvondst die meldt, en van Vinted wil hij ze wel zien, met een
waarschuwing. Twee niveaus maken dat te zeggen zonder de grens te verschuiven.
