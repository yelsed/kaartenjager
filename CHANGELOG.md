# Wat er veranderde

## Nog niet uitgegeven: producten

- Zoeken per product (`[[product]]` in de configuratie). Elke regel hoort bij een product; een
  bestand zonder producten blijft werken (`docs/decisions/0001`).
- Elke vondst is een topvondst of een goede deal. Topvondsten van een product met `notify =
  true` gaan naar Discord, nu ook voor onderdelen (`docs/decisions/0002`).
- Beeldbuistelevisies van Vinted, waar geen afstand bekend is, verschijnen met
  `show_without_distance` als goede deal met het label "afstand onbekend".
- De app: een keuzebalk per product in de inbox, een pagina Producten om producten aan en uit
  te zetten, zoektermen per product (`docs/decisions/0003`).
- Schema 3, en met de meldingen schema 4 (`docs/database.md`). Het programma migreert zelf; **werk de app tegelijk bij**,
  want de app weigert een schema dat hij niet kent.
- Meldingen via een eigen Discord-webhook (`[notify] discord_webhook`): topvondsten noemen je
  (`mention_user_id`) zodat je telefoon trilt, goede deals van een product met `notify_deals`
  komen stil. Een vondst die Discord niet aannam wordt de volgende ronde opnieuw geprobeerd.
  Zonder webhook blijft de route via Hermes (`docs/decisions/0004`).
- Een product kan een eigen `push_below_market_percent` hebben (`docs/decisions/0005`), en
  `check` zegt per product hoe het meldt.
- Elke ronde onthoudt hoe lang hij duurde. De app toont dat onderaan en waarschuwt als een ronde
  langer duurt dan de helft van de tijd tussen twee rondes.

## 2.0.0 — 18 september 2026

### Waarom

Vinted beantwoordt `/api/v2/catalog/items` sinds september 2026 met HTTP 403. Dat eindpunt was de
enige manier waarop kaartenjager daar zocht, dus elke ronde leverde vanaf dat moment nul
Vinted-advertenties op — zonder dat er iets rood werd. Marktplaats had er geen last van.

### Wat er nu gebeurt

Vinted wordt bezocht via de gewone cataloguspagina `/catalog?search_text=...`, geladen in een echte
Chromium. Die pagina staat door de server ingevuld, dus er valt gewoon te lezen wat er staat — maar
pas nadat hij echt geladen is, en daar is een browser voor nodig.

Eén browser en één tabblad per ronde, zoektermen na elkaar, nooit parallel. Aansturing gaat over de
DevTools-pijp (`--remote-debugging-pipe`), dus zonder open netwerkpoort en zonder extra
bibliotheek.

**Geen stealth, geen CAPTCHA-omzeiling, geen vermomming.** Houdt Vinted ons tegen, dan stopt de
ronde daar en treedt de bestaande terugvalregeling in werking: 15, 30, 60, 120 minuten. De overige
bronnen draaien wel gewoon af.

De artikelpagina van Vinted gaat nog steeds gewoon over HTTP. Daar staat nog een
`application/ld+json`-blok met prijs, beschrijving en of hij verkocht is, en daar draait de
hercontrole op. Die kant is niet aangeraakt.

### Wat je moet installeren

Chromium. `pacman -S chromium`, `apt install chromium`, `dnf install chromium`, of
`apk add chromium` (zet dan ook `[browser] no_sandbox = true`). Google Chrome werkt ook.

Staat er geen browser, dan levert Vinted niets op, meldt de app dat, en draait Marktplaats gewoon
door. Er valt dan géén blokkade-strike: een ontbrekende browser is iets anders dan een bron die ons
tegenhoudt, en die twee vragen om een ander antwoord.

Controleren met `kaartenjager doctor`, uitproberen met `kaartenjager probe "rtx 3090"`.

### Nieuw

- `kaartenjager probe <zoekterm>` — één Vinted-zoekopdracht via de browser, met de uitkomstsoort,
  de HTTP-status, hoeveel rijen er stonden en hoeveel er te lezen waren. Schrijft niets weg, meldt
  niets, en raakt de database niet aan.
- `kaartenjager doctor` kijkt nu ook of er een browser is, of hij start, of de cataloguspagina
  laadt en of de ontleder er wat mee kan. Dat is de enige controle in dit programma die het netwerk
  aanraakt; hij kost een seconde of zes.
- `kaartenjager check` noemt of er een browser gevonden is. Alleen kijken, niet starten — `check`
  blijft snel en offline, en de exitcode verandert er niet door.
- `[browser]`-sectie in de configuratie: `enabled`, `executable`, `max_pages`, `page_timeout_ms`,
  `delay_between_pages_ms`, `round_budget_seconds`, `no_sandbox`, `profile_dir`. Alle acht hebben
  een standaard, dus een bestaande `kaartenjager.toml` blijft ongewijzigd laden.

### Wat je kwijtraakt

De cataloguspagina geeft minder mee dan de oude JSON-API:

| Veld | Hoe het nu zit |
|---|---|
| `posted_at` | Vervalt. Kwam uit de fotostempel; niets in het programma leest dit veld, en de app toont dan de eerste keer dat wíj hem zagen. Bestaande waarden blijven staan. |
| `view_count` | Vervalt. Stond in zoekresultaten altijd al op nul, en de app filterde die nullen er toch al uit. |
| `reserved` | Niet meer zichtbaar tijdens het zoeken. Komt bij de eerstvolgende hercontrole alsnog boven water. |
| `photo_count` | Het raster toont er één per rij, wat niets zegt. Staat daarom op nul (= onbekend) en wordt aangevuld zodra de beschrijving wordt opgehaald. |
| `seller` | Nieuw uit de artikelpagina; die wordt voor de beschrijving toch al opgehaald, dus het kost geen extra verzoek. |

De waarschuwing "geen foto's" is weg. Nul betekende nooit betrouwbaar "geen foto's" — ook
Marktplaats leest dat veld met een terugval op nul — dus die waarschuwing was niet waar te maken.
"Maar één foto" blijft, en klopt nu wel.

### Verder veranderd

- Het verzoekbudget heet nu *paginabezoeken*. Vinted telt per zoekterm mee voor `max_pages`
  pagina's in plaats van één verzoek. `check` en de ronde rekenen dat allebei met dezelfde functie
  uit, in plaats van met twee kopieën die uiteen kunnen lopen. De grens blijft 60.
- Een ronde duurt langer: ongeveer twee minuten bij vijftien zoektermen, tegen een paar seconden
  hiervoor. Ruim binnen het rondeslot van een kwartier en de cron van vijf minuten, maar het is een
  echte verandering. `round_budget_seconds` is de noodrem eronder.
- Het browserprofiel blijft tussen rondes staan. Driehonderd keer per dag als gloednieuwe bezoeker
  langskomen vanaf hetzelfde adres valt meer op dan één die terugkomt. Na een controlepagina wordt
  het profiel wél weggegooid — een vergiftigd koekje is het enige geval waarin schoon beginnen
  helpt.
- `kaartenjager.example.toml`: RTX 3090 meldt onder €1.000 (marktbereik 1.000–1.200) en RTX 3090 Ti
  onder €1.100 (marktbereik 1.100–1.300). De marktbereiken schoven mee omdat `alert_below` nooit
  boven `used_price_low` mag liggen. Dat is een fors wijder net dan de 700/950 hiervoor, dus reken
  op meer in de inbox. **Je eigen `~/.config/kaartenjager/kaartenjager.toml` wordt niet
  overschreven** — die krijgt hooguit een `kaartenjager.toml.new` ernaast.

### Wat niet verandert

- Het databaseschema blijft 2. Geen migratie, geen dataverlies, en de app hoeft niet mee.
- De opdrachten die de app gebruikt (`run`, `config path`, `check --config`) en die de Hermes-skill
  gebruikt (`reviews take|answer|fail`, `dossier`, `check`, `config apply`) zijn ongewijzigd.
- Zoektermen blijven door de app en de database beheerd; er wordt niets opnieuw uit TOML gezet.
- De Marktplaats-bron is niet aangeraakt.
- De CRT-regel (`crt`, `beeldbuis`, `trinitron`, binnen 45 km) is niet aangeraakt.

### Bijwerken vanaf 1.8 of 1.9

```sh
# 1. Chromium erbij, als hij er nog niet staat
apt install chromium        # of pacman -S chromium / dnf install chromium

# 2. Het programma bijwerken
curl -fsSL https://raw.githubusercontent.com/yelsed/kaartenjager/main/install.sh | sh

# 3. Nalopen
kaartenjager --version      # kaartenjager 2.0.0
kaartenjager doctor         # de regels browser, browser start en cataloguspagina
kaartenjager probe "rtx 3090"
```

Verder niets. De database blijft staan, je configuratie blijft staan, je zoektermen blijven aan.
De app hoeft niet opnieuw gebouwd te worden — het schema is niet veranderd.

Draai je `deploy.sh`, dan waarschuwt die voortaan als er geen Chromium staat, maar breekt hij er
niet op af.
