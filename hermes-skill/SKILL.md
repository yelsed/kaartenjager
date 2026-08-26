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

Een programma houdt elk uur Vinted en Marktplaats in de gaten, schrijft alles wat het vindt
naar een database die de gebruiker in een app bekijkt, en meldt in Discord alleen de echte
uitschieters. Jij doet de twee dingen die het programma niet kan: oordelen over advertenties
waar de gebruiker om vraagt, en de prijstabel bijhouden.

## Wanneer welke taak

| Aanleiding | Wat jij doet | Instructies |
|---|---|---|
| Een bericht `review gevraagd: <sleutel>` in het kanaal | De wachtrij afwerken | `references/oordelen.md` |
| `kaartenjager-prijzen`, zondag 09:00 | Marktprijzen herzien | `references/prijsherziening.md` |
| Als er gevraagd wordt wat een kaart zou toevoegen | De machine en het plan erbij halen | `references/opstelling.md` |

Er is **geen cronjob** meer die de stapel automatisch afwerkt. Beoordelen gebeurt op verzoek:
de gebruiker drukt in de app op een knop, en dat stuurt het bericht hierboven.

Lees het bijbehorende bestand voordat je begint. Ze staan los zodat je alleen laadt wat je
nodig hebt.

**`references/opstelling.md` is de belangrijkste.** Daarin staat welke machine dit is, welk
model erop moet draaien en waarom, en waartegen een gevonden kaart afgezet hoort te worden.
Zonder dat bestand kun je alleen algemeenheden zeggen over hardware. Lees het ook bij het
beoordelen van de stapel, niet alleen bij een directe vraag.

## Het programma

```bash
kaartenjager reviews pending    # kijken zonder op te pakken
kaartenjager reviews take       # de wachtrij oppakken, als JSON
kaartenjager reviews answer <id> --recommendation <kijken|overslaan|oplichterij>
                                # het oordeel zelf gaat via stdin
kaartenjager reviews fail <id> --reason <tekst>
kaartenjager dossier <sleutel>
kaartenjager config apply --from <bestand> [--check]
kaartenjager config rollback [--to JJJJ-MM-DD]
kaartenjager check
```

Een sleutel ziet eruit als `vinted:7005251780` of `marktplaats:m2434539849`.

**Begin altijd met `reviews take`.** Komt daar `[]` uit, dan is er niets te doen en meld je
niets.

## Wat je nooit doet

**Je schrijft nooit rechtstreeks in `cards.auto.toml` of `kaartenjager.toml`.** Wijzigingen
gaan altijd via `kaartenjager config apply`, want dat controleert of een voorstel klopt.
Rechtstreeks schrijven omzeilt die controle en kan de wachter stilzwijgend uitschakelen.

**Je praat nooit rechtstreeks met de database.** Alles gaat via de opdrachten hierboven. De
app leest en schrijft hetzelfde bestand, en het schema hoort op één plek te veranderen.

**Je biedt en koopt nooit iets.** Je rapporteert, meer niet.
