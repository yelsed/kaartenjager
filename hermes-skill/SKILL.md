---
name: kaartenjager
description: Beoordeelt gevonden videokaarten en houdt de prijstabel bij
version: 1.0.0
platforms: [linux]
metadata:
  hermes:
    tags: [hardware, marktplaats, vinted, inkoop]
    category: monitoring
    requires_toolsets: [terminal]
    config:
      - key: kaartenjager.binary
        description: Pad naar het kaartenjager-programma
        default: "~/.local/bin/kaartenjager"
        prompt: "Waar staat het kaartenjager-programma?"
---

# Kaartenjager

Een programma houdt elk uur Vinted en Marktplaats in de gaten en meldt zelf wat duidelijk
te goedkoop is. Jij doet de twee dingen die het programma niet kan: oordelen over
twijfelgevallen, en de prijstabel bijhouden.

## Wanneer welke taak

| Cronjob | Wat jij doet | Instructies |
|---|---|---|
| `kaartenjager-oordeel`, 11:00 en 19:00 | De stapel twijfelgevallen afwerken | `references/oordelen.md` |
| `kaartenjager-prijzen`, zondag 09:00 | Marktprijzen herzien | `references/prijsherziening.md` |
| Als er gevraagd wordt wat een kaart zou toevoegen | De machine en het plan erbij halen | `references/opstelling.md` |

Lees het bijbehorende bestand voordat je begint. Ze staan los zodat je alleen laadt wat je
nodig hebt.

**`references/opstelling.md` is de belangrijkste.** Daarin staat welke machine dit is, welk
model erop moet draaien en waarom, en waartegen een gevonden kaart afgezet hoort te worden.
Zonder dat bestand kun je alleen algemeenheden zeggen over hardware. Lees het ook bij het
beoordelen van de stapel, niet alleen bij een directe vraag.

## Het programma

```bash
kaartenjager queue peek     # kijken zonder op te pakken
kaartenjager queue take     # oppakken (en apart zetten)
kaartenjager queue done     # opgepakte stapel als afgehandeld melden
kaartenjager dossier <sleutel>
kaartenjager config apply --from <bestand> [--check]
kaartenjager config rollback [--to JJJJ-MM-DD]
kaartenjager check
```

Een sleutel ziet eruit als `vinted:7005251780` of `marktplaats:m2434539849`.

## Wat je nooit doet

**Je schrijft nooit rechtstreeks in `cards.auto.toml` of `kaartenjager.toml`.** Wijzigingen
gaan altijd via `kaartenjager config apply`, want dat controleert of een voorstel klopt.
Rechtstreeks schrijven omzeilt die controle en kan de wachter stilzwijgend uitschakelen.

**Je biedt en koopt nooit iets.** Je rapporteert, meer niet.
