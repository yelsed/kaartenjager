# 0006 Kaartenjager blijft op openbinker tot Vinted het adres weert

Datum: 23 november 2026. Taak: TASK-3 / YEL-501.

## Besluit

De app, het programma en de ronde blijven op openbinker, naast Hermes.

Verhuizen gebeurt pas als Vinted het datacenteradres weert: als de hartslag in de app **drie
dagen achter elkaar** een blokkade van Vinted laat zien (`Blocked`, met `block_strikes:vinted`
die niet terug naar nul gaat). Dan verhuist het programma met de app en de database naar een
machine met een thuisadres, in deze volgorde van voorkeur:

1. De mini-pc uit Widening 4, als die er is.
2. Een virtuele machine op ThuisCLuster.

Niet naar de virtuele machine paperclip: die is te vaak zwaar belast.

Bij die verhuizing roept Hermes het programma over het tailnet aan
(`ssh <machine> kaartenjager reviews take`), zodat Hermes op openbinker kan blijven.

## Waarom

De eigenaar houdt Hermes (23 november 2026), en Vinted werkend houden weegt voor hem het
zwaarst. Hermes en de database op één machine is het eenvoudigst dat werkt. Na Widening 2 hangen
de meldingen niet meer aan Hermes, dus de enige reden om te verhuizen is het adres dat Vinted
ziet. Die reden is te meten, dus het besluit wacht op de meting in plaats van op een gevoel.

Afgewogen in `docs/onderzoek/hosting.md`.
