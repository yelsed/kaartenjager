# De stapel afwerken

Draait om 11:00 en 19:00. Duurt een paar minuten en kost weinig, want de stapel is meestal
kort of leeg.

## Werkwijze

**1. Pak de stapel op.**

```bash
kaartenjager queue take
```

Dat drukt per advertentie af: de vondst, een regel `UITZOEKEN` met wat er onduidelijk is, en
een dossierblok met alles wat bekend is.

Is de stapel leeg, dan meld je niets en ben je klaar. Stuur geen bericht om te zeggen dat er
niets was.

**2. Kijk per advertentie.**

Voor Marktplaats staat de volledige beschrijving al in het dossier; die hoef je niet op te
halen. Voor Vinted staat er `(niet opgehaald)` — gebruik dan `web_extract` op de URL om de
beschrijving en de foto's te zien.

**3. Beoordeel.**

Drie soorten twijfel komen langs, elk met een eigen vraag:

| `UITZOEKEN` zegt | Wat jij uitzoekt |
|---|---|
| Geheugengrootte staat niet in de titel | Welke uitvoering is dit? Een 3060 met 8 GB is een andere, goedkopere kaart dan die met 12 GB |
| Onbekend model | Wat is dit, hoeveel videogeheugen heeft het, en wat doet het tweedehands? |
| Prijs ligt onder de bodem | Is dit oplichterij of een echte buitenkans? |

Waar je bij alle drie naar kijkt:

- **Foto's.** Zijn het foto's van de kaart zelf, of productplaatjes van de fabrikant? Alleen
  fabrikantfoto's is de sterkste aanwijzing voor oplichterij die er is.
- **Beschrijving.** Klinkt dit als iemand die weet wat hij verkoopt, of als een tekst die
  overal op zou kunnen slaan? Staat er iets over mining, over reparatie, over "werkte nog
  prima toen ik hem eruit haalde"?
- **Verkoper.** Hoeveel beoordelingen, hoe lang actief, verkoopt hij vaker hardware of vooral
  kleding?
- **De prijs zelf.** Een kaart op een derde van de marktprijs is bijna nooit echt. Een kaart
  op tachtig procent kan heel goed een verhuizing zijn.

**4. Rapporteer.**

Twee lijstjes, geen essay:

```
Vier van de stapel bekeken.

WAARD OM NAAR TE KIJKEN
· Sapphire RX 6800 XT 16GB — €240
  Gaat normaal voor €320–380. Verkoper heeft 40 beoordelingen, foto's zijn
  van de kaart zelf met serienummer zichtbaar. Beschrijving noemt geen mining.

OVERGESLAGEN
· "videokaart nvidia" €90 — alleen fabrikantfoto's, geen serienummer,
  verkoper zonder beoordelingen. Vrijwel zeker oplichterij.
· RTX 3060 €130 — beschrijving noemt 8GB, dus de goedkope uitvoering.
· RTX 4090 €236 — "HS" staat voor hors service; kaart doet het niet.
```

Zet onder elke aanrader het dossierblok, zodat het meteen te plakken is.

**5. Sluit af.**

```bash
kaartenjager queue done
```

Alleen doen als je de hele stapel hebt gehad. Loop je vast, sla die stap dan over — dan staat
het werk er om 19:00 nog en gaat er niets verloren.

## Als je een model vaker ziet

Komt hetzelfde onbekende model meerdere keren langs, noteer dat dan in je rapport. Zondag
gebruik je die notities om er een regel voor voor te stellen, zodat het programma hem
voortaan zelf vangt.
