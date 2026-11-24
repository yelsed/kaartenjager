# 0001 Elke regel hoort bij een product

Datum: 23 november 2026. Taak: TASK-1 / YEL-503.

## Besluit

De configuratie krijgt `[[product]]`-blokken (`id`, `name`, `search_terms`, `notify`). Elke
`[[card]]` en `[[part]]` wijst met `product = "<id>"` naar zijn product. Een vondst krijgt het
product van de regel die hem herkende; een onbekende kaart hoort bij het product van de
kaartregels.

Een configuratie zonder `[[product]]` blijft geldig. Dan maakt het programma zelf de producten:
alle kaartregels samen worden `videokaarten` (met `card_search_terms`), elk onderdeel wordt een
eigen product met de naam van de regel en de zoektermen uit `part_search_terms` die op die regel
passen. Zo hoeft het bestand op de server niet in dezelfde stap mee te veranderen.

## Waarom

- De eigenaar wil per product kunnen focussen; dat vraagt een product als ding, niet een label.
- De prijsregels bestaan al en werken. Ze ophangen aan een product is de kleinste stap; een
  nieuwe regelsoort per product (zoals mini-pc's) komt later naast `card` en `part`.
- Een regel zonder product zou stil verdwijnen uit elk overzicht. Daarom weigert `check` een
  regel die naar een onbekend product wijst.
