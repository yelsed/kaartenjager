# Kaartenjager — ontwerp

**Datum:** 24 augustus 2026
**Status:** goedgekeurd, klaar voor implementatieplan
**Doel:** Vinted en Marktplaats afzoeken op ondergeprijsde videokaarten, voedingen en
riserkabels, en dat melden in Discord via Hermes Agent.

---

## 1. Waarom

Marktplaats en Tweakers zitten vol verkopers die hun kaart eerst hebben opgezocht. Vinted
is van oorsprong een kledingplatform; verkopers daar zijn vaker niet-technisch en prijzen
slordiger.

Waarneming van 24 augustus 2026, zonder gericht zoeken:

| Bron | RTX 3090 Ti, tweedehands |
|---|---|
| Duits gemiddelde | ± € 1.029 |
| Vinted Nederland, eerste twee advertenties | € 945,70 |

Acht procent onder de markt. Het aanbod is dun en verloopt binnen dagen, dus handmatig
kijken werkt niet — als je kijkt staat de goede advertentie er al drie dagen.

Tweede reden: tweedehandsprijzen zakken continu. Een drempeltabel van augustus klopt in
december niet meer, en het faalt stil. Daarom werkt de tabel zichzelf bij (§7).

---

## 2. Architectuur

Drie Hermes-cronjobs op `openbinker`. Verder niets.

```
┌─ Laag 1 ── elk uur, 08:00–22:00 ── no_agent ── nul tokens ──────────────┐
│                                                                          │
│  kaartenjager run                                                        │
│    Vinted API + Marktplaats API                                          │
│    → filter: al gezien / verkeerde categorie / vraagadvertentie          │
│    → oordeel uit de kaartentabel: onder de markt? per GB? past hij?      │
│    → stdout  ──▶ Hermes ──▶ Discord                                      │
│    → twijfelgevallen ──▶ queue.jsonl                                     │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
┌─ Laag 2 ── 11:00 en 19:00 ── met agent ─────────────────────────────────┐
│                                                                          │
│  kaartenjager queue --take   (leest en leegt de stapel)                  │
│    → per advertentie: beschrijving, foto's, verkoper beoordelen          │
│    → onbekend model: wat is dit en is die prijs scherp                   │
│    → oordeel met reden ──▶ Discord                                       │
└──────────────────────────────────────────────────────────────────────────┘

┌─ Laag 3 ── zondag 09:00 ── met agent ───────────────────────────────────┐
│                                                                          │
│  marktonderzoek per model                                                │
│    → voorstel naar cards.proposed.toml                                   │
│    → kaartenjager config apply --from cards.proposed.toml                │
│         keurt: syntax · logica · max 20% stap · absolute grenzen         │
│    → diff of weigering ──▶ Discord                                       │
└──────────────────────────────────────────────────────────────────────────┘
```

**Het dragende principe: het programma keurt, de agent stelt voor.** Een verzinsel van het
model wordt een geweigerd voorstel met uitleg, nooit een kapotte wachter.

### Cronjobs, letterlijk

```python
cronjob(action="create", name="kaartenjager-scan",
        schedule="0 8-22 * * *", no_agent=True,
        script="~/.local/bin/kaartenjager run",
        deliver="discord:#<kanaal>")

cronjob(action="create", name="kaartenjager-oordeel",
        schedule="0 11,19 * * *", skill="kaartenjager",
        prompt="Werk de stapel van kaartenjager af.",
        deliver="discord:#<kanaal>")

cronjob(action="create", name="kaartenjager-prijzen",
        schedule="0 9 * * 0", skill="kaartenjager",
        prompt="Wekelijkse prijsherziening volgens de skill.",
        deliver="discord:#<kanaal>")
```

`no_agent=True` bestaat expliciet voor watchdogs en heartbeats — precies laag 1. De
uitvoer van een cronjob gaat automatisch naar het `deliver`-doel; er is geen apart
verzendcommando nodig, en dus ook geen ntfy of Discord-webhook.

`<kanaal>` is de enige waarde die bij installatie ingevuld moet worden. Wordt de cronjob
vanuit Discord aangemaakt, dan kan `deliver="origin"` en gaat het antwoord vanzelf terug
naar diezelfde chat; een expliciet kanaal is duidelijker als je het later wilt verplaatsen.

---

## 3. Modules

Rust, één binair bestand.

| Module | Verantwoordelijkheid |
|---|---|
| `main.rs` | Opdrachtregel: `run`, `dry-run`, `queue`, `dossier`, `config`, `check`, `update`, `selftest` |
| `config.rs` | TOML laden, samenvoegen van handmatig en automatisch bestand, valideren |
| `http.rs` | `ureq` + `rustls`, koekjes, wachttijd tussen verzoeken, herhaalpogingen |
| `listing.rs` | `Listing` — de ene vorm die elke bron oplevert |
| `sources/vinted.rs` | Vinted-API naar `Listing` |
| `sources/marktplaats.rs` | Marktplaats-API naar `Listing` |
| `filter.rs` | Categorie, vraagadvertenties, gereserveerd, bezorgwijze |
| `pricing.rs` | Modelherkenning, drempels, het waarom-blok |
| `state.rs` | Wat is al gemeld, met vervaltermijn |
| `queue.rs` | De overdracht naar laag 2 |
| `dossier.rs` | Het plakblok |
| `report.rs` | Uitvoer voor Discord |
| `selfupdate.rs` | `config apply` met alle keuringen |

### Afhankelijkheden

| Kist | Waarvoor |
|---|---|
| `ureq` (rustls-functie) | HTTP. Blokkerend, geen async-runtime |
| `serde`, `serde_json` | Antwoorden van beide bronnen |
| `toml` | Configuratie lezen en schrijven |
| `time` | Vervaltermijnen en tijdstempels |

Geen `regex`: modelherkenning gaat met `to_lowercase()` en `contains()`. Geen `tokio`.
Geen BoringSSL — zie §4.

---

## 4. Gegevensbronnen

Beide eindpunten zijn op 24 augustus 2026 met de hand geverifieerd vanaf deze machine.

### Vinted

```
GET https://www.vinted.nl/                       (koekjes ophalen, 10 stuks)
GET https://www.vinted.nl/api/v2/catalog/items
      ?search_text=<term>&order=newest_first&per_page=<n>&page=1
```

Headers: browser-`User-Agent`, `Accept: application/json`,
`Referer: https://www.vinted.nl/catalog`.

**Cloudflare blokkeert ons niet.** De literatuur waarschuwt voor TLS-vingerafdrukherkenning,
maar dat geldt bij opschalen. Met een gewone curl-vingerafdruk en een browser-`User-Agent`
kwam er een 200 met 70 KB aan echte advertenties. Op vijftien rondes per dag met negen
zoektermen zitten we ver onder de gemelde grens van 10–30 verzoeken per minuut.

Daarom **geen `rquest`/BoringSSL**, en blijft het statische musl-bestand haalbaar. Zie §10
voor wat er gebeurt als dat ooit verandert.

Velden, zoals waargenomen:

| Veld | Vorm | Gebruik |
|---|---|---|
| `id` | getal | identiteit |
| `title` | tekst | modelherkenning |
| `price` | `{amount:"900.0"}` | vraagprijs |
| `total_item_price` | `{amount:"945.7"}` | **wat je werkelijk betaalt** |
| `service_fee` | `{amount:"45.7"}` | kopersbescherming |
| `url` | volledige URL | link |
| `user` | `{id, login, profile_url}` | verkoper |
| `status` | `"Nieuw zonder prijskaartje"` | staat |
| `photos` | lijst | aantal foto's |
| `favourite_count`, `view_count` | getal | belangstelling |
| `brand_title` | `"NVIDIA"` | zwakke categoriehint |

Bovenin: `pagination{current_page, total_pages, total_entries}`.

**Beslissing: oordelen op `total_item_price`, tonen als "€900 (€945,70 incl.)".** Anders
vergelijk je een Vinted-vraagprijs met een Marktplaats-eindprijs, en dat scheelt vijf
procent de verkeerde kant op.

### Marktplaats

```
GET https://www.marktplaats.nl/lrp/api/search
      ?query=<term>&limit=<n>&offset=0
```

Geen sessie, geen koekjes, geen waargenomen snelheidslimiet.

| Veld | Vorm | Gebruik |
|---|---|---|
| `itemId` | `"m2434539849"` | identiteit |
| `title` | tekst | modelherkenning |
| `description` | volledige tekst | **staat al in het zoekresultaat** |
| `priceInfo` | `{priceCents: 119900, priceType: "FIXED"}` | prijs |
| `location` | `{cityName, countryName, distanceMeters}` | plaats |
| `date` | `"Vandaag"` | ouderdom |
| `sellerInformation` | `{sellerId, sellerName}` | verkoper |
| `categoryId` | `353` | **videokaarten** |
| `verticals` | `["graphic_cards", ...]` | **categoriefilter** |
| `extendedAttributes` | `[{key:"delivery", value:"Ophalen of Verzenden"}, ...]` | bezorgen, staat, geheugentype |
| `reserved` | boolean | overslaan indien waar |
| `vipUrl` | relatief pad | link |
| `imageUrls` | lijst | aantal foto's |

`priceType` is een enum: `FIXED`, `BID`, `FAST_BID`, `MIN_BID`, `FREE`, `ASKING`, `TRADE`,
`SEE_DESCRIPTION`, `RESERVED`. Alleen `FIXED` en `SEE_DESCRIPTION` leveren een bruikbare
prijs; de rest wordt overgeslagen.

**Dat `description` al meekomt is belangrijk:** laag 2 hoeft voor Marktplaats geen enkele
detailpagina op te halen. Dat scheelt de helft van zijn werk en de bijbehorende tokens.

### Verzoekbudget per ronde

Dit bepaalt of we onder de snelheidsgrens van Vinted blijven, dus het staat vast in plaats
van dat het per configuratie uit de hand loopt.

| | Aantal |
|---|---|
| Zoektermen voor kaarten | 9 |
| Zoektermen voor voedingen en risers | 4 |
| Bronnen | 2 |
| Verzoeken per ronde | 26, plus 1 voor de Vinted-sessie |
| Wachttijd ertussen | 1,5 s |
| Duur van een ronde | ± 45 s |
| Rondes per dag | 15 |
| **Verzoeken per dag per bron** | **± 200** |

Vinted meldt een grens van 10–30 verzoeken per minuut. Wij doen er 40 per uur verdeeld over
45 seconden, dus ruim eronder. Het programma weigert te starten als het aantal zoektermen
maal het aantal bronnen boven de 60 uitkomt; dan is de configuratie te gulzig geworden en
is een waarschuwing beter dan een blokkade.

---

## 5. Filteren

Voordat er iets beoordeeld wordt, gaan deze eruit:

| Filter | Regel | Waarom |
|---|---|---|
| Al gemeld | staat in `seen.json` | geen herhaling |
| Verkeerde categorie | Marktplaats: `verticals` bevat niet de doelcategorie | in de proef zat een resultaat op `categoryId 341` tussen de videokaarten |
| **Vraagadvertentie** | titel of beschrijving bevat `gezocht`, `ruilen`, `wtb`, `ter overname gevraagd`, `zoek ik` | in de proef stond *"RTX4070 Super ruilen tegen RTX3090"* tussen de resultaten — iemand die er één wíl |
| Gereserveerd | `reserved == true`, of `priceType == RESERVED` | niet meer te koop |
| Onbruikbare prijs | `priceType` niet in `{FIXED, SEE_DESCRIPTION}`, of `priceCents <= 0` | bieden en "op aanvraag" hebben geen prijs |
| Alleen ophalen, te ver | `extendedAttributes.delivery == "Ophalen"` én `location.distanceMeters > max_pickup_km` | Alkmaar heeft nauwelijks aanbod; verzenden is de eis |

```toml
[filters]
postcode = ""                # INVULLEN. Zonder postcode geeft Marktplaats geen afstand,
                             # en dan kan het ophaal-filter niet werken
max_pickup_km = 30           # verder weg en alleen ophalen: overslaan
skip_pickup_only = false     # true = ophalen altijd overslaan
wanted_words = ["gezocht", "ruilen", "wtb", "ter overname gevraagd", "zoek ik", "wie heeft"]
```

Die vraagadvertentie-filter is niet optioneel. Zonder hem is een deel van de Marktplaats-
resultaten op een populair model iemand die er zelf één zoekt — in de proef van 24 augustus
stond er één tussen de eerste vijf.

**Vinted kent geen categorieveld in het zoekresultaat.** Daar valt dus niet op categorie te
filteren; `brand_title` is een zwakke hint en `catalog_ids` zou een vaste categoriecode per
markt vergen die niet gedocumenteerd is. Gevolg: Vinted leunt volledig op modelherkenning
uit de titel, en levert daardoor meer twijfelgevallen op dan Marktplaats. Dat is
acceptabel — die gaan naar de stapel, en laag 2 lost ze op.

---

## 6. Beslissen en melden

### De kaartentabel

Per model in de configuratie:

```toml
[[card]]
name = "RTX 3090 Ti"
patterns = ["3090 ti", "3090ti"]
vram_gb = 24
bandwidth_gbs = 1008
tdp_watt = 450
used_price_low = 950
used_price_high = 1050
alert_below = 850
suspicious_below = 550
require_memory_in_title = false

[[card]]
name = "RTX 3090"
patterns = ["3090"]
exclude_patterns = ["3090 ti", "3090ti"]     # anders vangt deze regel de Ti
vram_gb = 24
bandwidth_gbs = 936
tdp_watt = 350
used_price_low = 750
used_price_high = 925
alert_below = 700
suspicious_below = 450
```

Herkennen gaat zonder reguliere expressies: titel naar kleine letters, dan `contains()` per
patroon. Een regel past als **één** patroon voorkomt en **geen enkel** `exclude_patterns`
voorkomt. Daarmee is de volgorde in het bestand niet meer bepalend, wat een hele klasse
fouten wegneemt — al blijft de eerste passende regel winnen als er toch overlap is.

### Voedingen en risers

Videokaarten krijgen een rekenkundig oordeel omdat er per GB en per watt iets te zeggen
valt. Voor de andere twee categorieën bestaat die maatstaf niet, dus krijgen ze een
eenvoudiger regelsoort:

```toml
[[part]]
name = "Voeding 750-1200 W"
patterns = ["voeding", "psu", "power supply", "netzteil"]
require_all = ["w"]                       # ergens een wattage in de titel
min_watts = 700                           # uit de titel gelezen, anders op de stapel
alert_below = 90
suspicious_below = 30
note = "Let op 80 PLUS Gold of beter, en op de lengte: max 180 mm in de 4000D."

[[part]]
name = "PCIe riser"
patterns = ["riser", "pcie verleng", "extender"]
alert_below = 25
suspicious_below = 6
note = "Alleen PCIe 4.0 x16 is bruikbaar. 3.0 halveert de bandbreedte."
```

Verschillen met `[[card]]`:

| | `[[card]]` | `[[part]]` |
|---|---|---|
| Waarom-blok | berekend uit VRAM, bandbreedte, verbruik | vaste `note` uit de configuratie |
| Marktvergelijking | ja, `used_price_low/high` | nee |
| Categoriefilter | `graphic_cards` | breder, dus meer ruis en vaker op de stapel |

Voedingen zonder herkenbaar wattage in de titel gaan naar de stapel in plaats van
weggegooid te worden — een advertentie "Corsair voeding modulair" kan best een RM1000x zijn.

### Vier uitkomsten

| Situatie | Naar Discord | Ook op de stapel |
|---|---|---|
| Bekend model, onder de drempel, maat klopt | direct | nee |
| Geheugengrootte ontbreekt in de titel | direct, met vlag | ja |
| Onbekend model, opvallend goedkoop | direct, met vlag | ja |
| Onder `suspicious_below` | direct, met waarschuwing | ja |
| Boven de drempel, of geen kaart | — | — |

De derde regel vangt "kleinere kaarten die goedkoop opstaan". Laag 1 weet niet wat een
6700 XT waard is en gaat dat niet raden; hij ziet "videokaart, en goedkoop" en legt hem neer
voor laag 2.

**De 3060-val.** De RTX 3060 bestaat met 8 en met 12 GB en heet in advertentietitels
allebei "RTX 3060"; hetzelfde geldt voor de 4060 Ti. Bij `require_memory_in_title = true`
geldt: staat de maat er en klopt hij niet → overslaan; staat hij er niet → melden met de
vlag dat er gekeken moet worden.

### Het waarom-blok

Volledig rekenwerk, geen taalmodel:

```
RTX 3090 Ti — €820,00
MSI Gaming X Trio 24GB · Nederland · Vinted

WAAROM INTERESSANT
· €130 tot €230 onder de markt (normaal €950–1.050)
· €34,17 per GB videogeheugen — markt zit op €39,58
· 24 GB en 1008 GB/s: draait een 27B-model op Q4 met ruimte over
· 450 W — je RM850 trekt dit als enige kaart

LET OP
· 450 W betekent 3× 8-pins via de 12VHPWR-verloopstekker

https://www.vinted.nl/items/...
```

Regel drie en vier rekenen met een `[system]`-blok dat de machine van de gebruiker
beschrijft. Zonder dat blok worden die twee regels weggelaten in plaats van gegokt:

```toml
[system]
psu_watts = 850              # de voeding die er nu in zit
other_draw_watts = 155       # processor, schijven, ventilatoren
model_bits_per_weight = 4.8  # Q4_K_M, voor de "past dit model erin"-regel
kv_overhead_gb = 3.0         # KV-cache plus rekenbuffers
```

Regel drie ("draait een 27B-model op Q4") volgt uit
`(vram_gb - kv_overhead_gb) * 8 / model_bits_per_weight` = het aantal miljard parameters
dat past. Regel vier vergelijkt `tdp_watt + other_draw_watts` met `psu_watts`.

### Stilte

**Geen treffers betekent geen uitvoer.** Zonder die regel krijg je vijftien berichten per
dag en zet je het na een week uit.

---

## 7. De configuratie die zichzelf bijhoudt

### Twee bestanden

| Bestand | Wie schrijft | Wint |
|---|---|---|
| `kaartenjager.toml` | alleen de gebruiker | ja |
| `cards.auto.toml` | `config apply` | nee |

Een handmatig gezette drempel blijft staan, wat de agent ook voorstelt.

### De keuring

`kaartenjager config apply --from <bestand>` weigert bij:

| Controle | Regel |
|---|---|
| Syntax | moet als TOML laden; alle verplichte velden aanwezig en van het juiste type |
| Innerlijke logica | `suspicious_below < alert_below < used_price_low <= used_price_high` |
| **Stapgrootte** | geen waarde mag meer dan 20% verschuiven ten opzichte van de huidige |
| Absolute grenzen | een kaartregel kan niet onder €20 of boven €5.000 |
| Herkomst | een nieuw model heeft een `source`-veld met bronvermelding nodig |
| Volledigheid | een bestaand model mag niet zomaar verdwijnen |

De stapgrens is de kern. Marktprijzen bewegen geen 50% per week; een voorstel dat dat doet
is een verzinsel.

**De prijs:** bij een echte instorting duurt het twee weken voor de tabel is bijgetrokken.
Bewuste ruil tegen het risico van één verkeerde week.

### Versies en terugdraaien

Vier weken aan versies in `config-history/`. `kaartenjager config rollback [--naar DATUM]`.

### Wat de gebruiker zondag ziet

Toegepaste wijzigingen met percentage en onderbouwing, geweigerde met de reden, en een
opsomming van wat ongewijzigd bleef — dat laatste zodat stilte te onderscheiden is van
vastlopen. Een saaie week is drie regels.

---

## 8. Het dossier

### De stapel oppakken

`kaartenjager queue --take` verplaatst de regels naar `queue.taken.jsonl` en drukt ze af.
Rondt laag 2 af, dan roept hij `kaartenjager queue --done` en wordt dat bestand geleegd.
Klapt laag 2 halverwege, dan staat het werk er om 19:00 nog en gaat er niets verloren.

Staat er bij het oppakken al iets in `queue.taken.jsonl`, dan komt dat er weer bij — een
mislukte ronde levert een dubbele stapel op, geen lege.

### Het plakblok

```bash
kaartenjager dossier vinted:7005251780
```

Eén blok, klaar om in een ander model te plakken: advertentie, letterlijke beschrijving,
kaartgegevens uit de tabel, marktvergelijking, en een lijst vragen aan de verkoper. Bij
elke Discord-melding gaat hetzelfde blok mee, zodat het commando meestal niet nodig is.

Voor Marktplaats komt alles uit het zoekresultaat — beschrijving, staat, bezorgwijze en
verkoper zitten er al in. Voor Vinted moet de detailpagina opgehaald worden, want de
beschrijving ontbreekt in de zoekuitvoer.

Dat is een extra verzoek per advertentie, en dat telt mee in het budget van §4. Daarom haalt
**laag 1 nooit detailpagina's op**: die meldt met wat hij heeft. Alleen `dossier` en laag 2
doen dat, en die draaien twee keer per dag over een handvol advertenties. Levert een
detailpagina een 429 op, dan komt het blok zonder beschrijving met een notitie erbij.

---

## 9. Bestanden en uitrol

| Pad | Wat |
|---|---|
| `~/.local/bin/kaartenjager` | binair bestand |
| `~/.config/kaartenjager/kaartenjager.toml` | handmatige configuratie |
| `~/.config/kaartenjager/cards.auto.toml` | door de agent voorgesteld, door het programma gekeurd |
| `~/.config/kaartenjager/config-history/` | vier weken versies |
| `~/.local/share/kaartenjager/seen.json` | gemelde advertenties, 30 dagen |
| `~/.local/share/kaartenjager/queue.jsonl` | overdracht naar laag 2
| `~/.local/share/kaartenjager/queue.taken.jsonl` | door laag 2 opgepakt, nog niet afgerond |
| `~/.hermes/skills/kaartenjager/SKILL.md` | wat laag 2 en 3 moeten doen |

### Repo en release

`github.com/yelsed/kaartenjager`, **publiek** — alleen publieke releases zijn zonder token
te downloaden, en de server mag geen GitHub-inloggegevens nodig hebben. Alleen code en een
voorbeeldconfiguratie zijn openbaar; echte drempels blijven op de server.

Bij een tag `v*` bouwt GitHub Actions statische bestanden voor
`x86_64-unknown-linux-musl` en `aarch64-unknown-linux-musl`, plus `SHA256SUMS`. Musl omdat
het dan niet uitmaakt welke glibc of architectuur `openbinker` heeft.

```bash
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/install.sh | sh
```

Herkent de architectuur, haalt de nieuwste release, **controleert de SHA256**, plaatst het
binaire bestand en de skill, en schrijft een voorbeeldconfiguratie als er nog geen is.
Bijwerken: `kaartenjager update`.

### De skill

`~/.hermes/skills/kaartenjager/SKILL.md` met frontmatter volgens de Hermes-conventie:
`name`, `description` (≤60 tekens), `version`, `platforms: [linux]`,
`metadata.hermes.requires_toolsets: [terminal]`, en `metadata.hermes.config` voor het pad
naar de configuratie.

SKILL.md blijft kort; het werk staat in `references/` zodat Hermes met `skill_view(name, path)`
alleen laadt wat hij nodig heeft:

- `references/oordelen.md` — hoe laag 2 een advertentie beoordeelt
- `references/prijsherziening.md` — hoe laag 3 een voorstel opstelt en welke keuringen er zijn

---

## 10. Fouten

| Situatie | Gedrag |
|---|---|
| Eén bron faalt | Fout loggen, andere bron doorlopen. Niets van de kapotte bron als gezien markeren |
| Beide bronnen falen | Afsluiten met een foutcode, zodat de cron het als mislukt toont |
| Vinted-sessie verlopen | Koekjes weggooien, één keer opnieuw, daarna opgeven |
| HTTP 429 | Deze ronde afbreken, niet doorrammen. Volgende ronde over een uur |
| **Vinted gaat structureel blokkeren** | Zichtbaar als aanhoudende 403's. Uitwijk: `rquest` achter een cargo-functie, of Vinted overlaten aan `web_extract` van laag 2 |
| `seen.json` beschadigd | Waarschuwing, leeg beginnen. Eenmalig het hele aanbod, daarna weer stil |
| Onbekend veld in het antwoord | Rij overslaan, niet klappen. Het schema van beide bronnen is ongedocumenteerd en verandert |

Geschiedenis wordt pas weggeschreven als de hele ronde erdoorheen is. Een klapper halverwege
markeert nooit iets als gezien dat niet gemeld is.

---

## 11. Testen

| Wat | Hoe |
|---|---|
| Parsers | Opgeslagen JSON-antwoorden van beide bronnen als testbestanden, inclusief de rommelige randgevallen uit de proef |
| Filters | De vraagadvertentie, de verkeerde categorie, `priceType: MIN_BID`, `reserved` |
| Prijslogica | De 3060-val, `3090 ti` die niet in de `3090`-regel valt, drempels, bodems |
| Keuring | Elk van de zes weigergronden uit §7 |
| Geschiedenis | Heen en weer schrijven; een beschadigd bestand mag niet klappen |
| Dossier | Vaste invoer, vaste uitvoer |
| Echt netwerk | Eén `--live`-test, met de hand, niet in de pijplijn |
| Op de server | `kaartenjager selftest` draait dezelfde controles zonder netwerk, zodat een kapotte installatie meteen zichtbaar is |

De opgeslagen antwoorden komen uit de verificatie van 24 augustus 2026 en staan in
`tests/fixtures/`.

---

## 12. Bewust niet

**Terugkoppeling van laag 2 naar de tabel.** Hermes die zelf "een 6700 XT is €340 waard"
in de tabel schrijft. Waardevol, maar pas te bouwen als bekend is welke modellen er
werkelijk langskomen. Laag 3 doet het bewust alleen voor modellen die al in de tabel staan,
plus nieuwe met bronvermelding.

**Een database, een webinterface, prijsgeschiedenis, andere platforms.** Komt als dit
maanden heeft gedraaid en er iets blijkt te ontbreken.

**Automatisch bieden of kopen.** Nooit.
