# Kaartenjager

Houdt Vinted en Marktplaats in de gaten op videokaarten, voedingen en riserkabels die te
goedkoop staan. Alles wat het vindt gaat naar een SQLite-database die je in een app bekijkt;
alleen echte uitschieters gaan óók naar Discord, via
[Hermes Agent](https://hermes-agent.nousresearch.com).

Eén statisch binair bestand en één databasebestand. Geen runtime, geen dienst, geen
container.

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

Twee taken op de server, plus beoordelen op verzoek.

| Wanneer | Wat | Kosten |
|---|---|---|
| Elke 5 minuten, 08:00–22:00 | Het programma zoekt, rekent en schrijft alles naar de database | nul tokens |
| Als je in de app op "Hermes laten kijken" drukt | Hermes beoordeelt die ene advertentie | één aanroep per keer |
| Zondag 09:00 | Hermes herziet de marktprijzen; het programma keurt het voorstel | één aanroep |

Het dragende principe: **het programma keurt, de agent stelt voor.** Een verzinsel van het
model wordt een geweigerd voorstel met uitleg, nooit een wachter die stilletjes niets meer
meldt.

Discord houdt alleen uitschieters over: meer dan `push_below_market_percent` onder de
onderkant van het marktbereik, en dan één bericht per advertentie in plaats van één per
ronde. De rest staat in de app. Zonder die regels krijg je vijftien meldingen per dag en zet
je het na een week uit.

**Hoe lang stond het er al?** Vinted noemt geen plaatsingstijd, maar de foto's dragen hun
uploadmoment mee — en verkopers maken die bij het plaatsen. Daarmee legt het programma per
advertentie vast wanneer hij geplaatst is, wanneer wij hem vonden, hoeveel mensen hem
bewaarden, en wanneer hij weg was en waarom (verkocht of weggehaald). In de app staat dat
onder "hoe het verliep", zodat je kunt zien hoe snel zo'n koopje werkelijk wegging.

**Prijzen volgen gaat via de advertenties zelf.** Beide bronnen geven alleen de zestig
nieuwste resultaten per zoekterm, dus een advertentie verdwijnt daar binnen dagen uit terwijl
hij gewoon nog te koop staat. Elke ronde haalt daarom hooguit dertig gevolgde advertenties op
bij hun eigen pagina — oudste eerst — en leest daar de huidige prijs en of ze nog bestaan.
Afwezigheid in de zoekresultaten betekent niets, en een storing bij een bron mag nooit als
"alles verkocht" lezen.

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

En de twee cronjobs, vanuit Discord tegen Hermes:

```
kaartenjager-scan      "*/5 8-22 * * *"  no_agent, script ~/.local/bin/kaartenjager run
kaartenjager-prijzen   "0 9 * * 0"       skill kaartenjager
```

**Waarom elke vijf minuten.** Een echt koopje op Vinted is soms binnen een half uur verkocht.
Elk uur kijken betekent dat je het gemiddeld een half uur te laat ziet, en dus meestal
misloopt. Elke vijf minuten brengt dat terug naar tweeënhalve minuut.

Dat kost niet twaalf keer zoveel verzoeken, want alleen het zóéken gaat sneller. Prijzen
volgen van advertenties die je al kent blijft op zijn eigen tempo (`[scan]` in de
configuratie), en beschrijvingen worden hergebruikt in plaats van elke ronde opnieuw
opgehaald. `kaartenjager check` rekent voor wat een cadans kost.

Taken die je vanuit Discord aanmaakt melden vanzelf terug in datzelfde kanaal.

Er is bewust **geen** cronjob voor het beoordelen. Die zou 144 keer per dag een agent-aanroep
kosten voor een wachtrij die vrijwel altijd leeg is. In plaats daarvan stuurt de app een kort
bericht in het kanaal zodra je op de knop drukt, en dat is het sein voor Hermes.

### Alles in één keer

```sh
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/deploy.sh | sh
```

Dat doet het programma, de configuratie, de database en de app achter elkaar, en het is
idempotent: elke stap kijkt eerst of hij nodig is, en niets wat van jou is wordt overschreven.
Wil je dat de Hermes-knop meteen werkt, geef de webhook dan mee:

```sh
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/deploy.sh \
  | KAARTENJAGER_DISCORD_WEBHOOK=https://discord.com/api/webhooks/... sh
```

Twee dingen doet het script bewust niet: de cronjob `kaartenjager-oordeel` weghalen (dat gaat
via Hermes) en je `kaartenjager.toml` aanpassen, op het toevoegen van `[notify]` en `[scan]` na.

De webhook mag je ook later invullen; dan zet je alleen die en blijft de rest van de service
zoals hij is:

```sh
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/deploy.sh \
  | KAARTENJAGER_DISCORD_WEBHOOK=https://discord.com/api/webhooks/... sh
systemctl --user restart kaartenjager-app
```

De losse stappen, als je liever ziet wat er gebeurt:

### En de app erbij

De app leest de database die het programma aanmaakt, dus **draai eerst één keer
`kaartenjager run`.** Hij maakt het bestand met opzet niet zelf aan: het schema hoort bij het
programma, en een app die zelf tabellen verzint is precies hoe twee versies stilletjes uit
elkaar gaan lopen.

```sh
git clone https://github.com/yelsed/kaartenjager.git ~/kaartenjager
cd ~/kaartenjager/app
npm ci
npm run build
```

Node 22 of nieuwer is genoeg: de database wordt gelezen met het ingebouwde `node:sqlite`, dus
er hoeft geen compiler op de server te staan.

Dan als gebruikersservice, zodat hij een herstart overleeft:

```sh
cp app/kaartenjager-app.service ~/.config/systemd/user/
nano ~/.config/systemd/user/kaartenjager-app.service   # pad en webhook invullen
systemctl --user daemon-reload
systemctl --user enable --now kaartenjager-app
loginctl enable-linger $USER
```

De webhook is die van je Discord-kanaal. Zonder die waarde werkt alles, alleen wordt Hermes
niet gewekt als je op de knop drukt — het verzoek staat dan wel in de wachtrij, en de scan
meldt het na een uur alsnog.

**Niet aan het open internet hangen.** De app kent geen inlog: wie op het tailnet binnen is
mag alles, en dat is precies waarom hij daar hoort te blijven.

### Bijwerken

```sh
# Het programma: hetzelfde script als bij installeren. Een bestaande
# kaartenjager.toml blijft staan; het nieuwe voorbeeld komt er als
# kaartenjager.toml.new naast.
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/install.sh | sh

# De app
cd ~/kaartenjager && git pull && cd app && npm ci && npm run build
systemctl --user restart kaartenjager-app
```

Verandert het schema, dan migreert het programma zichzelf bij de eerstvolgende start en
weigert de app tot hij bijgewerkt is. Dat is met opzet: half werken op een schema dat je
verkeerd begrijpt is erger dan niet werken.

## Opdrachten

| Opdracht | Wat het doet |
|---|---|
| `kaartenjager run` | Eén ronde: zoeken, melden, onthouden |
| `kaartenjager run --dry-run` | Zelfde ronde, niets onthouden of melden |
| `kaartenjager check` | Configuratie controleren |
| `kaartenjager selftest` | De ingebouwde controles, zonder netwerk |
| `kaartenjager reviews pending` | De wachtrij bekijken zonder hem op te pakken |
| `kaartenjager reviews take` | De wachtrij oppakken, als JSON |
| `kaartenjager reviews answer <id> --recommendation <...>` | Oordeel terugschrijven; de tekst gaat via stdin |
| `kaartenjager reviews fail <id> --reason <tekst>` | Verzoek als mislukt afsluiten |
| `kaartenjager reviews request <sleutel>` | Zelf een verzoek in de wachtrij zetten |
| `kaartenjager migrate --from-files` | De oude bestanden alsnog overzetten |
| `kaartenjager dossier <sleutel>` | Plakblok voor één advertentie |
| `kaartenjager config apply --from <bestand>` | Voorstel keuren en toepassen |
| `kaartenjager config rollback [--to DATUM]` | Terug naar een eerdere tabel |

`--config <pad>` wijst een andere configuratie aan, `--verbose` toont ook wat er geweerd werd
en waarom.

## Waar de gegevens staan

| Pad | Wat |
|---|---|
| `~/.local/share/kaartenjager/kaartenjager.db` | De database. Instelbaar met `KAARTENJAGER_DB`, zodat de app hem kan vinden zonder te gokken |
| `~/.config/kaartenjager/kaartenjager.toml` | Drempels, filters, machine, kast |

De database is de enige koppeling tussen het programma, de app en Hermes. Er is geen API en
geen poort: valt de app om, dan blijft het zoeken doorgaan, en andersom.

De app staat in [`app/`](app/): SvelteKit met server-routes, die de database leest met het
ingebouwde `node:sqlite` — geen native module, dus geen compiler op de server. Waar hij zich
aan moet houden staat in [`docs/app-afspraken.md`](docs/app-afspraken.md).

SQLite draait in WAL-modus en elke verbinding zet `PRAGMA busy_timeout = 5000`, zodat een
klik die samenvalt met het wegschrijven van een ronde wacht in plaats van te falen.
`PRAGMA user_version` zegt welk schema erin zit; de app hoort te weigeren bij een versie die
hij niet kent.

## Configuratie

Twee bestanden in `~/.config/kaartenjager/`:

| Bestand | Wie schrijft | Wint |
|---|---|---|
| `kaartenjager.toml` | alleen jij | ja |
| `cards.auto.toml` | de wekelijkse herziening, na keuring | nee |

Zet je een drempel met de hand, dan blijft die staan wat de herziening ook voorstelt.

**Zoektermen staan niet meer in TOML.** De lijst uit het bestand wordt bij de allereerste
start één keer in de database gezet; daarna beheer je hem in de app. Zo zet het weghalen van
je laatste zoekterm de hele lijst niet terug.

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
  zoekresultaat, dus een beoordeling hoeft daar geen enkele pagina voor op te halen.

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

## Uitgeven

```sh
git tag -a v1.6.0 -m 'Kaartenjager 1.6.0' && git push origin v1.6.0
gh run list --repo yelsed/kaartenjager --limit 1     # start er een build?
```

De workflow luistert op `push` van een tag `v*`, maar dat blijkt niet betrouwbaar te vuren:
sinds v1.4.0 kwam er geen enkele tag-push als event binnen terwijl de tags er wél staan.
Start hem dan met de hand — dat werkt wel, en levert dezelfde release op:

```sh
gh workflow run release.yml --repo yelsed/kaartenjager --ref v1.6.0
```

## Zelf bouwen

```sh
cargo build --release
./target/release/kaartenjager selftest

cd app && npm install && npm run build
```

Rust 1.85 of nieuwer. Vijf afhankelijkheden: `ureq`, `serde`, `toml`, `time` en `rusqlite`
met meegeleverde SQLite, zodat er op de server niets geïnstalleerd hoeft te zijn.

## Wat er bewust niet in zit

Geen inlog op de app — hij hangt achter het tailnet. Geen volledige configuratie in de app:
alleen zoektermen, want een verkeerde drempel legt de wachter stil. Geen grafieken, geen
andere platforms. Geen automatisch bieden of kopen — nooit.

## Licentie

MIT
