# Kaartenjager

Houdt Vinted en Marktplaats in de gaten op videokaarten, voedingen en riserkabels die te
goedkoop staan, en meldt dat in Discord via [Hermes Agent](https://hermes-agent.nousresearch.com).

Eén statisch binair bestand. Geen runtime, geen database, geen container.

## Waarom

Marktplaats en Tweakers zitten vol verkopers die hun kaart eerst hebben opgezocht. Vinted is
van oorsprong een kledingplatform; verkopers zijn er vaker niet-technisch en prijzen
slordiger.

| Bron | RTX 3090 Ti, tweedehands, augustus 2026 |
|---|---|
| Duits gemiddelde | ± € 1.029 |
| Vinted Nederland, eerste twee advertenties | € 945,70 |

Acht procent onder de markt, zonder gericht zoeken. Het aanbod is alleen dun en verloopt
binnen dagen — als je met de hand kijkt staat de goede advertentie er al drie dagen.

## Hoe het werkt

Drie taken op de server.

| Wanneer | Wat | Kosten |
|---|---|---|
| Elk uur, 08:00–22:00 | Het programma zoekt, rekent en meldt duidelijke vondsten | nul tokens |
| 11:00 en 19:00 | Hermes beoordeelt de twijfelgevallen met verstand | een paar aanroepen |
| Zondag 09:00 | Hermes herziet de marktprijzen; het programma keurt het voorstel | één aanroep |

Het dragende principe: **het programma keurt, de agent stelt voor.** Een verzinsel van het
model wordt een geweigerd voorstel met uitleg, nooit een wachter die stilletjes niets meer
meldt.

Geen vondsten betekent geen bericht. Zonder die regel krijg je vijftien meldingen per dag en
zet je het na een week uit.

## Installeren

Op de server, zonder root en zonder GitHub-account:

```sh
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/install.sh | sh
```

Het script kiest het juiste binaire bestand voor de architectuur, controleert de SHA256,
plaatst het programma in `~/.local/bin`, zet de Hermes-skill klaar en schrijft een
voorbeeldconfiguratie.

Daarna:

```sh
nano ~/.config/kaartenjager/kaartenjager.toml   # postcode invullen
kaartenjager check
kaartenjager run --dry-run
```

En de drie cronjobs, vanuit Discord tegen Hermes:

```
kaartenjager-scan      "0 8-22 * * *"   no_agent, script ~/.local/bin/kaartenjager run
kaartenjager-oordeel   "0 11,19 * * *"  skill kaartenjager
kaartenjager-prijzen   "0 9 * * 0"      skill kaartenjager
```

Taken die je vanuit Discord aanmaakt melden vanzelf terug in datzelfde kanaal.

## Opdrachten

| Opdracht | Wat het doet |
|---|---|
| `kaartenjager run` | Eén ronde: zoeken, melden, onthouden |
| `kaartenjager run --dry-run` | Zelfde ronde, niets onthouden of melden |
| `kaartenjager check` | Configuratie controleren |
| `kaartenjager selftest` | 24 ingebouwde controles, zonder netwerk |
| `kaartenjager queue peek` | De stapel voor laag twee bekijken |
| `kaartenjager queue take` | De stapel oppakken en apart zetten |
| `kaartenjager queue done` | De opgepakte stapel als afgehandeld melden |
| `kaartenjager dossier <sleutel>` | Plakblok voor één advertentie |
| `kaartenjager config apply --from <bestand>` | Voorstel keuren en toepassen |
| `kaartenjager config rollback [--to DATUM]` | Terug naar een eerdere tabel |

`--config <pad>` wijst een andere configuratie aan, `--verbose` toont ook wat er geweerd werd
en waarom.

## Configuratie

Twee bestanden in `~/.config/kaartenjager/`:

| Bestand | Wie schrijft | Wint |
|---|---|---|
| `kaartenjager.toml` | alleen jij | ja |
| `cards.auto.toml` | de wekelijkse herziening, na keuring | nee |

Zet je een drempel met de hand, dan blijft die staan wat de herziening ook voorstelt.

Per kaart twee getallen die ertoe doen:

- `alert_below` — hieronder wil je het horen
- `suspicious_below` — hieronder is het vaker oplichterij dan een buitenkans

Een regel past als één patroon in de **titel** voorkomt en géén enkel `exclude_pattern`. De
volgorde in het bestand is daarmee niet bepalend.

### De 3060-val

De RTX 3060 bestaat met 8 en met 12 GB en heet in advertentietitels allebei "RTX 3060";
hetzelfde geldt voor de 4060 Ti. Daarvoor is `require_memory_in_title`: staat de maat er en
klopt hij niet, dan overslaan; staat hij er niet, dan melden met de vlag dat er gekeken moet
worden.

## Wat er in een echte ronde uit kwam

Eerste ronde over 1.103 advertenties, 27 verzoeken, 24 augustus 2026:

- Een **RTX 4090 voor € 1.260,70** waar de markt op € 1.600–2.000 staat
- Een **Cooler Master PCIe 4.0 x16 riserkabel van 300 mm voor € 14,99**
- Vier voedingen van 850 W tussen € 74 en € 90
- Twee duidelijke oplichtingspogingen, correct als zodanig gemarkeerd

Die 27 vondsten zijn het koudestartgetal. Elk uur daarna zijn het alleen de nieuwe
advertenties.

## Hoe het aan de gegevens komt

Beide bronnen hebben een JSON-eindpunt dat hun eigen zoekpagina gebruikt:

- Vinted: `/api/v2/catalog/items`, met een sessiekoekje dat het programma haalt door eerst de
  voorpagina te laden. Bij een afgewezen verzoek wordt de sessie eenmaal vernieuwd.
- Marktplaats: `/lrp/api/search`, zonder sessie. De volledige beschrijving zit al in het
  zoekresultaat, dus laag twee hoeft daar geen enkele pagina op te halen.

Geen van beide is gedocumenteerd en beide kunnen zonder aankondiging veranderen. Elke parser
slaat een rij over in plaats van de ronde te laten klappen, en een bron die wegvalt laat de
andere doorlopen.

Anderhalve seconde tussen twee verzoeken, 27 verzoeken per ronde, vijftien rondes per dag.
Ruim onder wat beide sites verdragen.

## Als er iets langskomt

De melding is het begin, niet het eind:

1. Vraag een foto van de kaart zelf, met een briefje met de datum erbij. Alleen
   fabrikantfoto's is de sterkste aanwijzing voor oplichterij die er is
2. Vraag hoe lang in gebruik, waarvoor, en of er gemined is
3. Vraag het serienummer en controleer de garantie bij de fabrikant
4. Betaal via het platform. Kopersbescherming dekt "niet ontvangen" en "wijkt sterk af van de
   beschrijving" — niet "doet het na drie weken niet meer"
5. Vraag of hij in een doos met opvulling gaat. Videokaarten breken bij de PCIe-connector

## Zelf bouwen

```sh
cargo build --release
./target/release/kaartenjager selftest
```

Rust 1.85 of nieuwer. Vier afhankelijkheden: `ureq`, `serde`, `toml`, `time`.

## Wat er bewust niet in zit

Geen database, geen webinterface, geen prijsgeschiedenis, geen andere platforms. Geen
automatisch bieden of kopen — nooit.

## Licentie

MIT
