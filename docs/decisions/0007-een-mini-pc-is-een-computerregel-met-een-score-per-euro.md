# 0007 Een mini-pc is een computerregel met een score per euro

Datum: 23 november 2026. Taak: TASK-4 / YEL-502.

## Besluit

Een nieuwe soort regel, `[[computer]]`, naast `[[card]]` en `[[part]]`:

- `patterns`: de doelmodellen (m720q, optiplex micro, elitedesk mini, ...), gezocht in de titel.
- `min_memory_gb` (16): harde eis. Het geheugen komt uit het veld `memoryRAM` van Marktplaats,
  anders uit titel en beschrijving ("16GB", "16 Go", "8/16GB" telt als 8, "2x8GB" telt niet als
  8). Minder of onbekend valt af. Een Vinted-advertentie zonder beschrijving wacht op zijn
  beschrijving en wordt daarna opnieuw beoordeeld.
- `processors`: CPU Mark per processor (i5-9500T = 8100, ...). In de configuratie, want de tabel
  groeit en scores zijn geen code.
- `top_score_per_euro` (45): vanaf hier is het een topvondst. Daaronder een goede deal.
- Een processor die niet in de tabel staat of niet genoemd wordt: goede deal met "uitzoeken".
- `min_price` (40): daaronder is het een beugel, adapter of kap, geen pc.

De vondst bewaart zijn score per euro (`finding.score_per_euro`, schema 5). In de inbox van een
product met scores staan de vondsten op score per euro, hoogste eerst.

## Waarom

Een mini-pc past niet in de bestaande regels. Een kaart heeft een marktbereik per model, maar
dezelfde M720q bestaat met een i3 en een i5; de processor bepaalt de waarde, en die staat in de
advertentie, niet in de modelnaam. Een onderdeel heeft een vaste prijsgrens, maar de eigenaar
wil rekenkracht per euro, met het budget als richtlijn (22 november 2026). De grens van 45 komt
uit de marktcheck van 23 november (`docs/onderzoek/mini-pcs.md`).
