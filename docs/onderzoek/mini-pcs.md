# Onderzoek: mini-pc's als nieuw product

Onderzoek voor TASK-4 / YEL-502, gedaan op 22 november 2026, vóór er iets gebouwd wordt. Dit
document zegt **waar de zoekopdracht op moet letten in een advertentie**. Welke machine er gekocht
wordt en wat hij in het cluster gaat doen staat in de vault:
`Projecten/Home/Homelab/Mini-pc voor cluster en CI.md`.

## Wat we zoeken

Een gebruikte zakelijke 1-liter-pc (Tiny / Micro / Mini, geen small form factor of toren).

- **16 GB geheugen of meer, in de advertentie zelf. Harde eis.** Een machine met 8 GB plus een
  reep bijkopen telt niet. Een advertentie met keuzeopties ("8/16 GB") telt als de laagste.
- **Het budget is een richtlijn, geen grens.** Wat meer betalen voor veel meer rekenkracht mag.
- **Processor: rekenkracht per euro**, niet een vaste ondergrens. De app rangschikt op CPU Mark
  tegenover de prijs.

### Processor: rekenkracht per euro

PassMark CPU Mark, gemiddelden van cpubenchmark.net (2026, afgerond). De ZenBook (pve2) staat er
ter vergelijking bij, niet als grens.

| Processor | Kernen / threads | CPU Mark |
|---|---|---|
| i7-8565U (ZenBook, pve2, ter vergelijking) | 4 / 8 | 5.940 |
| i5-6500T | 4 / 4 | ongeveer 4.700 |
| i5-7500T | 4 / 4 | ongeveer 5.200 |
| i5-8500T | 6 / 6 | 7.700 |
| Ryzen 5 PRO 3400GE | 4 / 8 | 7.760 |
| Ryzen 5 PRO 2400GE | 4 / 8 | 7.200 tot 7.800 |
| i5-9500T | 6 / 6 | 8.000 tot 8.200 |
| i5-8500 (65 W) | 6 / 6 | 9.500 |
| i5-10500T | 6 / 12 | 10.200 |
| i7-9700T | 8 / 8 | 10.500 |
| Ryzen 3 PRO 5350GE | 4 / 8 | 12.600 |
| Ryzen 5 PRO 4650GE | 6 / 12 | 15.300 |

De cijfers voor i5-6500T en i5-7500T zijn bij benadering. Een processor die niet in deze tabel
staat, krijgt in de app "nakijken" tot de score is opgezocht.

### Doelmodellen

- Lenovo ThinkCentre M720q, M920q, M70q (en M920x, M75q, M90q)
- Dell OptiPlex 3060, 3070, 5060, 7060, 7070 Micro (en 3080 / 5080 / 7080 Micro)
- HP EliteDesk 800 G4 / G5 Mini, ProDesk 400 / 600 G4 / G5 Mini (en G6)

Een modelnaam zegt niets over de processor: dezelfde M720q bestaat met een i3-8100T en met een
i5-9500T. De processor moet dus uit de advertentie zelf komen.

## Wat de markt laat zien

Steekproef op Marktplaats, 22 november 2026: 13 zoekopdrachten (modelnamen, "mini pc 16gb",
"i5-8500t", "i5-9500t") via hetzelfde zoek-endpoint dat de app al gebruikt. 450 unieke
advertenties, waarvan 347 bij de doelmodellen horen. Vraagprijzen, geen verkoopprijzen.

| Processor, geheugen | Aantal | Mediaan | Laagste |
|---|---|---|---|
| i5-8500T, 8 GB | 15 | € 179 | € 110 |
| i5-8500T, 16 GB | 5 | € 250 | € 210 |
| i5-9500T, 8 GB | 14 | € 198 | € 125 |
| i5-9500T, 16 GB | 4 | € 198 | € 175 |
| i5-10500T, 16 GB | 12 | € 300 | € 280 |
| i5-6500T, 8 GB (te zwak) | 5 | € 125 | € 80 |

Wat dat betekent:

- **Het budget van € 80 tot € 150 uit het Gemini-onderzoek haal je alleen met een Ryzen 5 PRO
  2400GE of 3400GE** (HP EliteDesk 705 G4 / G5 Mini, vanaf € 129 met 16 GB). Een Intel 6-kerner met
  16 GB begint rond € 200, een i5-10500T rond € 275, een Ryzen 5 PRO 4650GE rond € 315.
- **De meeste rekenkracht per euro** gaf op 22 november de Ryzen 5 PRO 4650GE (HP EliteDesk 805 G6
  Mini, € 315): twee keer de score van een 3400GE en anderhalf keer die van een i5-10500T.
- 8 GB is veruit het meest aangeboden (139 van de advertenties waar het geheugen in staat), 16 GB
  komt 86 keer voor, 32 GB of meer 6 keer.

## Waar de zoekopdracht op moet letten

### Velden die Marktplaats zelf meegeeft

Bijna alles valt in categorie **328** (desktop-pc's: 407 van de 450). Naast titel, beschrijving en
prijs geeft het endpoint `extendedAttributes`:

| Veld | Gevuld bij | Bruikbaar |
|---|---|---|
| `memoryRAM` (bijvoorbeeld "8 GB", "16 GB") | 148 van 347 (43%) | Ja: waar het veld en de tekst allebei iets zeggen, kloppen ze in 122 van de 128 gevallen. De afwijkingen zijn advertenties met keuzeopties ("8GB/16GB"). |
| `storagetype` ("SSD") | 83 | Alleen het soort, niet de grootte |
| `Processor`, `processorSpeed` | ongeveer 70 | Te vaag (merk of kloksnelheid), de tekst is beter |
| `condition`, `delivery` | bijna altijd | Ja, zoals bij de andere producten |

`priceInfo.priceType` is `FIXED` (238), `MIN_BID` (83), `FAST_BID` (22) of `SEE_DESCRIPTION` (4),
net als bij de videokaarten.

### Wat uit de tekst moet komen

| Eigenschap | Hoe | Gevonden in |
|---|---|---|
| Processor | patroon `i[3579]-\d{4,5}[a-z]*` (i5-9500T, i5 8500t) | 159 van 347 (46%) |
| Geheugen | `memoryRAM` eerst, anders "16GB", "16 GB DDR4", "16GB RAM" in titel of tekst | samen ruim 70% |
| Opslag | "256GB", "512GB", "NVMe", "M.2", "SSD" | NVMe of M.2 genoemd in 43 |
| Voedingsadapter | "adapter", "voeding", "lader", "oplader", "incl." | genoemd in 36 (10%) |
| Model en generatie | modelnaam plus G-nummer of jaartal (G4, G5, 3070, M920q) | bijna altijd in de titel |
| Keuzeopties | "8GB/16GB", "16/32GB": de verkoper heeft meerdere versies | een handvol, telt als het laagste |

Niet zoeken op: BIOS-wachtwoord en automatisch aangaan na stroomuitval. Geen enkele advertentie
noemt het; dat vraag je de verkoper.

### Hoe de classificatie moet werken

1. **Model herkennen** (familie Tiny / Micro / Mini). Geen model, dan geen mini-pc.
2. **Processor vinden.** Niet gevonden (54% van de advertenties) is geen afwijzing maar
   "onbekend": tonen in de app, niet melden.
3. **Geheugen vinden.** Minder dan 16 GB, of onbekend, valt af. Keuzeopties tellen als de
   laagste.
4. **Prijs tegen de mediaan van die processor en dat geheugen**, zoals de videokaarten het al doen,
   en daarnaast rangschikken op CPU Mark per euro. Met de cijfers van nu: een i5-9500T met 16 GB
   onder ongeveer € 175 of een 4650GE onder ongeveer € 280 is een goede vondst.
5. **Adapter en NVMe** verhogen de score een beetje, maar ontbreken is geen afwijzing: 90% noemt de
   adapter niet.

### Bronnen

- **Marktplaats**: werkt met het bestaande endpoint, geen extra werk.
- **Vinted**: bemonsterd op 22 november via een browser (12 zoekopdrachten, 139 relevante
  advertenties). Anders dan Marktplaats:
  - **Europees aanbod.** vinted.nl toont vooral Franse, Italiaanse, Spaanse en Duitse verkopers.
    Verzending uit het buitenland, dus land en verzendkosten tellen mee.
  - **Talen in de tekst:** "16 Go" en "16Go RAM" (Frans), "16 GB di RAM" (Italiaans), "Netzteil"
    en "Alimentation" voor de adapter. Het geheugenpatroon moet `Go` naast `GB` kennen.
  - **Geen geheugenveld.** De tegel geeft titel, merk, staat, prijs en prijs inclusief Vinted-kosten
    (ongeveer 5% plus € 0,70). Geheugen en processor staan alleen in titel of beschrijving, en vaak
    alleen in de beschrijving: "Lenovo ThinkCentre M75q Gen 2" zonder meer in de titel had een
    Ryzen 3 PRO 5350GE met 16 GB. De artikelpagina lezen is dus nodig.
  - **Veel onderdelen.** Zoeken op "thinkcentre tiny" geeft ook VESA-beugels, adapters, ventilatoren
    en kappen. Die moeten eruit (prijs onder ongeveer € 40, of woorden als support, capot,
    ventilateur, alimentation zonder pc).
  - Prijzen liggen in dezelfde orde als Marktplaats; de goedkoopste 16 GB-machines zaten tussen
    € 195 en € 240.
- **Tweakers Vraag & Aanbod**: veel van dit soort machines, maar een nieuwe bron. Niet in deze
  fase.

## Bronnen voor dit document

- Steekproef: Marktplaats `lrp/api/search`, 22 november 2026.
- [PassMark i7-8565U](https://www.cpubenchmark.net/cpu.php?id=3308),
  [i5-8500T](https://www.cpubenchmark.net/cpu.php?id=3231),
  [i5-9500T](https://www.cpubenchmark.net/cpu.php?id=3454),
  [i5-10500T](https://www.cpubenchmark.net/cpu.php?id=3768).
