# 0003 De app zet producten aan en uit in de database

Datum: 23 november 2026. Taak: TASK-1 / YEL-503.

## Besluit

Producten staan in een tabel `product` (id, naam, volgorde, aan/uit). Het programma schrijft id,
naam en volgorde uit de configuratie bij elke start; de aan/uit-stand is van de app en het
programma raakt hem niet aan. Een uitgezet product wordt niet afgezocht, meldt niets en staat
niet in de inbox. Zijn vondsten blijven bewaard.

Zoektermen krijgen een kolom `product`. Nieuwe producten krijgen hun `search_terms` één keer in
de database; daarna beheert de app ze, net als de zoektermen nu.

## Waarom

De eigenaar vroeg een eenvoudige pagina om producten aan en uit te zetten. De app schrijft de
TOML niet per knop, en de zoektermen staan al in de database met de app als baas. Dezelfde
verdeling voor producten houdt één regel: de configuratie zegt wat er bestaat, de database wat er
aan staat.
