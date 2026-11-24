# 0005 Elk product kiest zijn eigen meldrempel

Datum: 23 november 2026. Taak: TASK-2 / YEL-504.

## Besluit

Een `[[product]]` krijgt naast `notify` twee instellingen:

- `notify_deals` (standaard uit): goede deals gaan stil naar Discord.
- `push_below_market_percent` (optioneel): voor kaarten van dit product, hoeveel onder de markt
  een topvondst is. Zonder waarde geldt die uit `[notify]`.

Onderdelen houden hun drempel per regel (`alert_below`), want daar hangt hij aan een prijs, niet
aan een percentage.

## Waarom

Een beeldbuis die 40 km verderop € 120 kost is iets anders dan een 3090 die 35% onder de markt
zit. Eén drempel voor alles maakte de beeldbuizen stil en de kaarten luid. De drempel staat in
de configuratie, waar de producten al staan (ADR 0001), en niet in de app.
