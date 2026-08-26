# Kaartenjager, deel twee: van meldingen naar een werkbank

**Datum:** 25 augustus 2026, dezelfde dag herzien na de kritische controle
**Status:** goedgekeurd, klaar voor implementatieplan
**Bouwt voort op:** `2026-08-24-kaartenjager-design.md`

---

## 1. Waarom dit er komt

De eerste versie meldt in Discord. Dat werkt technisch, maar niet in de praktijk:

**De berichten zijn te lang.** Een vondst is een kop, drie tot zes redenen, een
waarschuwingenblok, een link en een dossierblok van dertig regels. Twintig daarvan is een
muur tekst waar je niets mee doet.

**Er is geen dagelijkse blik.** Discord is een stroom. Je kunt niet zien wat er sinds gisteren
bij is gekomen, welke prijs gezakt is, of welke advertentie verdwenen is. En je kunt niets
wegleggen: elke melding blijft even zichtbaar als de rest.

**Er is geen terugweg.** De configuratie staat in een TOML-bestand op de server. Een zoekterm
toevoegen betekent inloggen en een bestand bewerken, en dus gebeurt het niet. De dekking
blijft daardoor te klein.

**Bij het bouwen vastgesteld:** het wordt een eigen SvelteKit-project met server-routes, in
`app/` binnen deze repository. Niet een route in iets bestaands: dan is de app los uit te
rollen en te herstarten zonder er iets anders bij te betrekken.

De database wordt gelezen met **`node:sqlite`**, dat sinds Node 22 in Node zelf zit, en niet
met `better-sqlite3` zoals eerst aangenomen. Dat scheelt een native module, en daarmee een
compiler op de server — precies het soort afhankelijkheid dat pas opvalt als je een keer wilt
bijwerken.

## 2. Wat er blijft en wat er verandert

| | Nu | Straks |
|---|---|---|
| Zoeken en beoordelen | Rust, elk uur, cron | **zelfde cron; de kernlus gaat om (§3)** |
| Opslag | `seen.json`, `queue.jsonl` | **SQLite** |
| Waar je kijkt | Discord | **Svelte-app** |
| Discord | elke vondst | **alleen echte uitschieters** |
| Wegleggen | kan niet | archiveren en volgen |
| Oordeel van Hermes | tweemaal daags automatisch | **een knop per advertentie** |
| Zoektermen aanpassen | bestand op de server | **in de app** |
| Prijsherziening | zondag, automatisch | onveranderd |

Het rekenwerk blijft gratis. Hermes kost per beoordeling, en daarom is dat nu een knop in
plaats van een automatisme: je betaalt voor de drie advertenties die je aanklikt, niet voor de
twintig die je overslaat.

## 3. Architectuur

```
   ┌──────────────────────────────────────────────────────────┐
   │  kaartenjager run    elk uur, 08:00-22:00, no_agent      │
   │    Vinted + Marktplaats → filteren → beoordelen          │
   │    + gerichte hercontrole van actieve vondsten (§4)      │
   └───────────────┬─────────────────────────┬────────────────┘
                   │ schrijft                │ bij >35% onder de markt
                   ▼                         ▼
        ┌──────────────────────┐      ┌──────────────┐
        │   kaartenjager.db    │      │   Discord    │
        │      SQLite, WAL     │      │  kort bericht│
        └───┬──────────────┬───┘      └──────────────┘
            │ leest/schrijft│ leest/schrijft
            ▼               ▼
   ┌─────────────────┐   ┌────────────────────────────────┐
   │  Svelte-app     │   │  Hermes                        │
   │  server-routes  │   │    gewekt door het wekbericht  │
   │  better-sqlite3 │   │    van de app in Discord       │
   └────────┬────────┘   │    reviews take → oordelen →   │
            │ wekbericht  │    reviews answer              │
            │ (webhook)   └────────────────────────────────┘
            ▼                            ▲
      Discord-kanaal ────────────────────┘
```

De database staat op `~/.local/share/kaartenjager/kaartenjager.db`, en dat pad is instelbaar
met `KAARTENJAGER_DB` zodat de Svelte-app hem kan vinden zonder te gokken.

**Eén database, drie schrijvers.** SQLite in WAL-modus verdraagt dat: meerdere lezers naast
één schrijver tegelijk. De rondes duren seconden en de app schrijft alleen als je klikt, dus
er is geen wedloop van betekenis.

Wel moet elke verbinding `PRAGMA busy_timeout = 5000` zetten. Zonder dat geeft een klik die
precies samenvalt met het wegschrijven van een ronde meteen "database is locked" in plaats van
even te wachten. Vijf seconden is ruim: een ronde schrijft in een fractie daarvan.

Het programma schrijft bovendien per ronde **één transactie**, niet per advertentie. Anders
staat de database duizend keer kort op slot in plaats van één keer.

**De app wacht nooit op Hermes.** De knop zet een regel in `review_request` en stuurt daarna
via een webhook een kort wekbericht naar het Discord-kanaal — "review gevraagd:
vinted:9758884187". Hermes reageert op dat bericht zoals op elk bericht in het kanaal, haalt
de openstaande verzoeken op, beoordeelt ze en schrijft het antwoord terug. Er is géén
poll-cron: een dag zonder klikken kost nul agent-aanroepen.

Waarom een wachtrij mét wekbericht en geen rechtstreekse aanroep: een app die op een agent
wacht loopt vast als de agent traag is, verliest het verzoek bij een herstart, en moet een
time-out afhandelen die niemand wil bedenken. De wachtrij in de database blijft daarom de
waarheid; het Discord-bericht is alleen het belletje. Gaat het verloren (webhook faalt,
Hermes ligt eruit), dan blijft het verzoek gewoon staan: de app toont hoe lang het al wacht,
en de uurlijkse scan print een vangnetregel naar Discord zodra een verzoek langer dan een
uur onbeantwoord openstaat — zichtbaar voor jou, zodat je Hermes alsnog kunt porren.

**Bij inrichting te controleren:** dat Hermes op webhook-berichten in het kanaal reageert.
Zo niet, dan moet het wekbericht als gewone gebruiker verstuurd worden (bot-token) — zelfde
ontwerp, ander verzendmechanisme.

**De database is de enige koppeling.** Er is geen API, geen poort, geen dienst die moet
blijven leven. Valt de app om, dan blijft het zoeken doorgaan. Valt het zoeken om, dan blijft
de app tonen wat er al was.

**De kernlus van het programma gaat om.** De huidige code beoordeelt een advertentie precies
één keer: wat in `seen.json` staat wordt overgeslagen (`if !history.is_new(&key) continue`
in `hunt.rs`). Dit ontwerp eist het omgekeerde — elke ronde wordt élk zoekresultaat opnieuw
gezeefd en beoordeeld, want daar komen `last_seen`, de prijsgeschiedenis en `still_a_find`
vandaan. De al-gezien-poort en `seen.json` vervallen dus; herbeoordelen is
tekenreeksvergelijking en kost niets. Afgewezen en vondstloze advertenties laten geen spoor
na in de database — de zeef doet zijn werk gewoon elke ronde opnieuw. Dit is de grootste
wijziging aan de Rust-kant en hoort niet tussen de regels door te gebeuren.

## 4. Het gegevensmodel

```sql
PRAGMA journal_mode = WAL;
PRAGMA user_version = 1;   -- opgehoogd door het Rust-programma bij elke schemawijziging;
                           -- de app controleert dit bij het starten en weigert met een
                           -- duidelijke melding bij een versie die hij niet kent

-- Elke advertentie die ooit een vondst werd. Advertenties zonder vondst laten geen
-- spoor na: die worden elke ronde opnieuw gezeefd en beoordeeld, en dat is goedkoop.
-- Zo blijft de database klein zonder opruimbeleid.
CREATE TABLE listing (
  key            TEXT PRIMARY KEY,           -- "vinted:9758884187"
  source         TEXT NOT NULL,
  listing_id     TEXT NOT NULL,
  title          TEXT NOT NULL,
  url            TEXT NOT NULL,
  description    TEXT NOT NULL DEFAULT '',
  location       TEXT NOT NULL DEFAULT '',
  seller         TEXT NOT NULL DEFAULT '',
  condition      TEXT NOT NULL DEFAULT '',
  delivery       TEXT NOT NULL DEFAULT 'unknown',  -- shipping | pickup | unknown
  photo_count    INTEGER NOT NULL DEFAULT 0,
  first_seen     INTEGER NOT NULL,
  last_seen      INTEGER NOT NULL,
  found_by_terms TEXT NOT NULL DEFAULT '[]', -- JSON-lijst van termen die hem vonden
  last_checked   INTEGER,                    -- laatste gerichte hercontrole (§4)
  gone_checks    INTEGER NOT NULL DEFAULT 0, -- opeenvolgende "bestaat niet meer" op rij
  gone_since     INTEGER                     -- pas na twee zulke hercontroles
);

-- Eén regel per prijswijziging, niet per ronde. Hiermee werkt "gezakt" en
-- "terug uit het archief".
CREATE TABLE price_point (
  key            TEXT NOT NULL REFERENCES listing(key) ON DELETE CASCADE,
  seen_at        INTEGER NOT NULL,
  price_cents    INTEGER NOT NULL,           -- wat je werkelijk betaalt
  asking_cents   INTEGER NOT NULL,           -- vraagprijs, zonder kosten
  PRIMARY KEY (key, seen_at)
);

-- Wat het programma concludeerde. Elke ronde bijgewerkt met een UPDATE die de eenmalige
-- velden (became_a_find_at, pushed_at, pushed_at_price) ongemoeid laat — een INSERT OR
-- REPLACE zou ze wissen, en dan meldt Discord dezelfde vondst elke ronde opnieuw.
CREATE TABLE finding (
  key                  TEXT PRIMARY KEY REFERENCES listing(key) ON DELETE CASCADE,
  matched_as           TEXT NOT NULL,        -- "RTX 3090 Ti" of "Onbekend model"
  kind                 TEXT NOT NULL,        -- card | part | unknown
  confidence           TEXT NOT NULL,        -- clear | review
  percent_under_market REAL,                 -- NULL bij onderdelen
  euros_under_market   REAL,
  reasons              TEXT NOT NULL,        -- JSON-lijst
  warnings             TEXT NOT NULL,        -- JSON-lijst
  queue_note           TEXT,
  became_a_find_at     INTEGER NOT NULL,   -- wanneer dit vóór het eerst een vondst werd
  judged_at            INTEGER NOT NULL,   -- wanneer het laatst herbeoordeeld is
  still_a_find         INTEGER NOT NULL DEFAULT 1,  -- 0 zodra de prijs boven de drempel gaat
  left_find_at_price   INTEGER,                     -- de prijs toen still_a_find op 0 ging
  pushed_at            INTEGER,                     -- wanneer het naar Discord ging
  pushed_at_price      INTEGER                      -- de prijs van dat moment
);

-- Wat jij ermee deed.
CREATE TABLE decision (
  key                 TEXT PRIMARY KEY REFERENCES listing(key) ON DELETE CASCADE,
  state               TEXT NOT NULL,         -- inbox | archived | watching
  changed_at          INTEGER NOT NULL,
  price_when_archived INTEGER,               -- om een daling te kunnen zien
  note                TEXT
);

-- De wachtrij naar Hermes.
CREATE TABLE review_request (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  key            TEXT NOT NULL REFERENCES listing(key) ON DELETE CASCADE,
  requested_at   INTEGER NOT NULL,
  taken_at       INTEGER,
  answered_at    INTEGER,
  attempts       INTEGER NOT NULL DEFAULT 0, -- hoe vaak dit verzoek is opgepakt
  verdict        TEXT,                       -- de tekst van Hermes
  recommendation TEXT,                       -- kijken | overslaan | oplichterij
  failed_reason  TEXT                        -- gevuld door `reviews fail`, dat óók
                                             -- answered_at zet: mislukt is een eindtoestand
);

-- Zoektermen, beheerd vanuit de app.
CREATE TABLE search_term (
  term      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL,                   -- card | part
  enabled   INTEGER NOT NULL DEFAULT 1,
  added_at  INTEGER NOT NULL,
  added_by  TEXT NOT NULL DEFAULT 'app'      -- app | config | hermes
);

-- Los-vaste waarden: last_visit, previous_visit, terms_seeded, en de hartslag —
-- last_round_at plus last_round_problems (JSON), elke ronde geschreven door de scan.
CREATE TABLE app_state (
  name   TEXT PRIMARY KEY,
  value  TEXT NOT NULL
);

CREATE INDEX listing_last_seen ON listing(last_seen);
CREATE INDEX listing_checked   ON listing(last_checked);
CREATE INDEX finding_became    ON finding(became_a_find_at);
CREATE INDEX decision_state    ON decision(state);
CREATE INDEX review_pending    ON review_request(answered_at) WHERE answered_at IS NULL;

-- Eén openstaand verzoek per advertentie. Tweemaal op de knop drukken hoort niet
-- tweemaal te kosten.
CREATE UNIQUE INDEX review_one_open ON review_request(key) WHERE answered_at IS NULL;
```

### Wat er uit volgt

| Vraag | Hoe |
|---|---|
| Nieuw sinds mijn laatste bezoek | `finding.became_a_find_at > app_state['last_visit']` |
| Prijs gezakt | Laatste twee `price_point`-regels van dezelfde advertentie |
| Terug uit het archief | `state = 'archived'` en laatste prijs onder 90% van `price_when_archived` |
| Verdwenen | `gone_since IS NOT NULL` |
| Niet langer interessant | `still_a_find = 0` — prijs ging omhoog of de tabel veranderde |
| Volglijst | `state = 'watching'`, gesorteerd op prijsverandering |
| Wacht op Hermes | `review_request` met `answered_at IS NULL` |
| Draait de wachter nog | `app_state['last_round_at']`; binnen 08:00–22:00 ouder dan twee uur → rode balk in de app |

Prijsgeschiedenis wordt **alleen bij verandering** weggeschreven. Vijftien rondes per dag maal
duizend advertenties zou anders vijftienduizend regels per dag opleveren voor niets.

### Waarom er twee tijdstempels op een vondst staan

Het programma beoordeelt elke advertentie opnieuw zodra hij in de resultaten voorkomt, dus
vijftien keer per dag. Met één tijdstempel zou dat betekenen dat **elke vondst elke ronde weer
"nieuw" is** en de inbox nooit leegloopt.

Daarom: `became_a_find_at` wordt één keer gezet, bij de eerste keer dat iets onder de drempel
uitkomt. `judged_at` schuift wel mee, zodat je kunt zien of een oordeel vers is. Alleen de
eerste bepaalt wat er in het nieuw-blok staat.

Zakt een advertentie later opnieuw onder de drempel nadat hij eruit was gelopen, dan telt dat
als nieuw en wordt `became_a_find_at` vernieuwd — maar alleen als de prijs werkelijk lager is
dan `left_find_at_price`, de prijs waarop hij eruit liep. Zonder die voorwaarde zou de
wekelijkse prijsherziening, die drempels tot twintig procent mag verschuiven, vondsten heen
en weer laten wippen en als "nieuw" aanmerken terwijl er aan de advertentie niets veranderde.
Een echte prijsdaling is nieuws; een bewogen drempel niet.

Het spiegelbeeld is ook een schrijfactie: levert een geziene of hergecontroleerde
advertentie géén vondst meer op, dan zet de ronde uitdrukkelijk `still_a_find = 0` en
`left_find_at_price`. Wie dat overlaat aan "er komt geen nieuwe finding-rij", zet het nooit —
de beoordelingsfunctie geeft bij een te hoge prijs immers gewoon niets terug.

### Gerichte hercontrole: prijzen volgen en "weg" vaststellen

**Afwezigheid in de zoekresultaten zegt niets.** Beide bronnen leveren per term alleen de
zestig níéuwste resultaten (`order=newest_first`); elke advertentie schuift daar na enkele
dagen uit terwijl hij gewoon nog te koop staat. Wie afwezigheid als "verkocht" leest,
markeert op den duur álles als verdwenen, ziet geen enkele prijsdaling meer, en de
archief-terugkeer — de verkoper die na twee weken toegeeft — vuurt nooit. Dat is precies de
stille storing die dit systeem niet mag hebben.

Daarom worden actieve advertenties — vondsten in de inbox, op de volglijst of in het
archief, zonder `gone_since` — **gericht hergecontroleerd**: het programma haalt hun eigen
advertentiepagina of item-API op. Dat levert twee dingen op: de actuele prijs (naar
`price_point` bij verandering — daar draaien de volglijst en de archief-terugkeer op) en
het antwoord op de vraag of de advertentie nog bestaat.

De regels:

- Hooguit **dertig hercontroles per ronde**, roulerend op `last_checked` (oudste eerst),
  zodat elke actieve advertentie ongeveer dagelijks langskomt. Een advertentie die deze ronde
  toch in de zoekresultaten stond wordt overgeslagen: dan is al bewezen dat hij bestaat.
- **De grens van zestig gaat over zoekverzoeken, niet over de hele ronde.** Dat onderscheid
  is nodig sinds hercontroles bestaan: met de voorbeeldconfiguratie is een ronde 26 zoeken +
  hooguit 30 hercontroles + hooguit 12 beschrijvingen, dus rond de 68 verzoeken. Dat past bij
  anderhalve seconde ertussen, maar het hoort zichtbaar te zijn in plaats van pas op te
  vallen als een bron gaat weigeren. `kaartenjager check` drukt daarom beide getallen af.
- Alleen een ondubbelzinnig "bestaat niet meer" (HTTP 404/410, of de verwijderd-markering
  van het platform) verhoogt `gone_checks`. Een netwerkfout, een 429 of een kapotte bron
  telt níét mee: dan blijft `last_checked` staan en komt de advertentie de volgende ronde
  weer aan de beurt. Een storing bij Vinted mag nooit als "alles is verkocht" lezen.
- `gone_since` wordt pas gezet na **twee** opeenvolgende zulke hercontroles; één 404 kan een
  hik zijn. Een geslaagde hercontrole zet `gone_checks` terug op nul.
- Voor Vinted bestaat de detailroute al (de dossierfunctie gebruikt hem). Voor Marktplaats
  is de advertentiepagina (`vipUrl`) de route.

**Bij het bouwen vastgesteld (25 augustus 2026):** beide bronnen geven een schone HTTP 404 op
een advertentie die niet bestaat, dus de weg-detectie leunt op een ondubbelzinnig signaal en
niet op een gokje. Beide zetten ook een schema.org-`Product`-blok in de pagina, met de prijs
en een `availability`. Vinted schrijft de prijs als getal, Marktplaats als tekst, en bij
Marktplaats staat er een `BreadcrumbList`-blok vóór het Product-blok. Een `availability` die
uitverkocht zegt telt als verdwenen: kopen kun je hem toch niet meer. De blokken van beide
sites staan als testbestand in `tests/fixtures/`.

Wat de pagina noemt is de **vraagprijs**; op Vinted komt daar kopersbescherming bovenop. Die
opslag is ongeveer evenredig, dus de hercontrole houdt de verhouding van de vorige meting
aan in plaats van de kosten weg te laten — dat laatste zou een prijsdaling voorspiegelen die
er niet is.

Het uitzetten van een zoekterm heeft hierdoor géén effect op bestaande vondsten: die worden
via hun eigen pagina gevolgd, niet via de zoekresultaten. `found_by_terms` blijft bestaan
voor de weergave (welke termen vonden dit), niet meer voor de weg-detectie.

## 5. De dagelijkse blik

Wat je 's ochtends opent.

```
┌─ INBOX ─────────────────────────────────────────── 4 nieuw ─┐
│                                                              │
│  NIEUW SINDS GISTEREN                                        │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ RTX 4090          €1.260,70      30% onder de markt    │ │
│  │ Gigabyte Windforce 3X · Vinted · alleen ophalen        │ │
│  │ 24 GB · past na radiator verplaatsen · 3,5 sleuven     │ │
│  │                                                        │ │
│  │ [archiveren]  [volgen]  [Hermes laten kijken]  [→]     │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ Cooler Master PCIe 4.0 riser 300mm    €14,99           │ │
│  │ Marktplaats · verzenden                                │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  EERDER                                        14 stuks  ▾   │
└──────────────────────────────────────────────────────────────┘
```

Drie regels per kaart in de lijst: **wat, hoeveel, waarom.** De volledige redenen, de
waarschuwingen en het dossierblok zitten achter het uitklappen. Dat is het verschil met
Discord, waar alles altijd openstond.

**Vier weergaven:**

| Tabblad | Wat erin staat |
|---|---|
| **Inbox** | Nieuw bovenaan, daaronder wat je nog niet hebt weggelegd |
| **Volglijst** | Wat je volgt, met de prijsontwikkeling erbij |
| **Archief** | Weggelegd. Komt iets terug door een prijsdaling, dan staat dat er ook |
| **Zoektermen** | Toevoegen, uitzetten, verwijderen |

Elke weergave toont hooguit vijftig regels met een knop om meer te laden. Na een half jaar
staan er duizenden advertenties in de database, en een pagina die ze allemaal ophaalt wordt
traag zonder dat iemand er iets aan heeft.

**Wanneer "nieuw" ophoudt nieuw te zijn.** Het bij het verlaten van de pagina bijzetten klinkt
logisch maar werkt niet: `beforeunload` gaat niet af bij een tabblad dat gesloten wordt op een
telefoon, bij een herstart, of als je gewoon wegklikt.

In plaats daarvan een knop **"alles gezien"** bovenaan het nieuw-blok, plus een automatische
grens: vondsten ouder dan achtenveertig uur tellen niet meer als nieuw, ook zonder klik. Dan
loopt het niet vol als je een week niet kijkt, en beslis je zelf wanneer je iets wegstreept.

De vorige stand wordt bewaard in `app_state['previous_visit']`, zodat "alles gezien" met één
klik terug te draaien is.

**De inbox heeft een afvoer.** Een vondst waarvan de advertentie verdwenen is (`gone_since`)
of die geen vondst meer is (`still_a_find = 0`) verdwijnt vanzelf uit de inbox. Hij blijft
zichtbaar in het archief, met het waarom erbij — "verdwenen op 3 september" of "prijs steeg
boven de drempel". Zonder die regel staan er na een half jaar honderden dode regels onder
"EERDER" en leest niemand de lijst nog.

**En een hartslag.** Is `app_state['last_round_at']` binnen het dagvenster (08:00–22:00)
ouder dan twee uur, dan toont de app bovenaan een rode balk, met de laatste inhoud van
`app_state['last_round_problems']` erbij. Een dode wachter ziet er anders precies zo uit als
een stille markt — en dat is de gevaarlijkste storing die dit systeem kent.

### Archiveren en volgen

**Archiveren** legt de advertentie weg en onthoudt de prijs van dat moment. Zakt die later
meer dan tien procent — de hercontrole (§4) blijft gearchiveerde advertenties volgen, dus
die daling wordt ook echt gezien — dan toont de inbox hem opnieuw, met de vlag *"was
gearchiveerd op €1.150, staat nu op €980"*. Dat is een leesregel in de app, geen
statuswijziging: `state` blijft `archived` en `price_when_archived` blijft staan totdat jij
er iets mee doet, zodat de scanner en de app nooit allebei aan dezelfde beslissing
schrijven. Dit vangt precies de verkoper die na twee weken toegeeft.

**Volgen** houdt hem zichtbaar in een aparte lijst, met elke prijswijziging eronder. Voor de
twee of drie kaarten waar je serieus over nadenkt.

## 6. Discord: alleen nog uitschieters

Eén regel bepaalt of er iets gestuurd wordt:

```toml
[notify]
push_below_market_percent = 35   # meer dan dit onder de laagste marktprijs
```

Berekend als `(used_price_low - prijs) / used_price_low`. Alleen voor `[[card]]`-regels, want
onderdelen hebben geen marktbereik.

**Er is geen aparte verbinding met Discord.** De cronjob draait met `no_agent` en Hermes levert
de standaarduitvoer van het programma af in het ingestelde kanaal. Print het programma niets,
dan is er geen bericht. Alles wat naar Discord moet is dus simpelweg wat er op stdout komt, en
alles wat niet naar Discord moet gaat alleen de database in.

Dat betekent ook: **waarschuwingen horen niet op stdout.** De huidige code print een
PROBLEMEN-blok mee in het rondebericht; in dit regime zou "vijf vondsten kregen geen
beschrijving" elke ronde een Discord-bericht opleveren. Problemen gaan voortaan naar stderr
én naar `app_state['last_round_problems']`, waar de app ze bij de hartslag toont. Op stdout
staan alleen uitschieters — plus één vangnetregel: staat er een reviewverzoek langer dan een
uur onbeantwoord open, dan print de scan dat, zodat een verloren wekbericht zichtbaar wordt.

Het bericht is kort. Geen redenen, geen dossier, geen waarschuwingen:

```
RTX 4090 — €1.260,70
30% onder de markt (€1.800–2.500)
Gigabyte Windforce 3X · alleen ophalen · Vinted
https://www.vinted.nl/items/9758884187-rtx-4090
```

Vier regels. De rest staat in de app.

**Eén bericht per advertentie, niet per ronde.** Zonder die regel krijg je vijftien keer per
dag dezelfde 4090 zolang hij online staat. Er komt een kolom `pushed_at` op `finding`; een
advertentie die al gemeld is blijft stil, tenzij de prijs sindsdien nog eens tien procent
zakt — dan is het nieuws. Omdat de ronde de finding-rij elke keer bijwerkt, moet die
bijwerking `pushed_at` en `pushed_at_price` uitdrukkelijk laten staan (§4); wist hij ze, dan
is dat precies fout drie uit het eerste ontwerp opnieuw.

**Onder de bodem gaat niets naar Discord.** Dit kwam uit de eerste echte rondes: de hoogste
kortingspercentages zijn vrijwel altijd oplichting. Een RTX 5090 voor €105,70 is 96% onder de
markt en dus per definitie de luidste melding van de dag. Het programma wéét dat al — die
prijs ligt onder `suspicious_below`, dat er juist voor is — maar de drempelregel hierboven
keek daar niet naar, en dan bestaat het kanaal dat zeldzaam en betrouwbaar hoort te zijn
vooral uit nepadvertenties.

Dus: een vondst onder `suspicious_below` van zijn eigen kaartregel wordt niet gemeld. Hij
staat gewoon in de app, met de waarschuwing erbij en de Hermes-knop eronder. Zo'n vondst
krijgt wél meteen zijn `pushed_at`-stempel, anders komt hij elke ronde opnieuw langs en zou
één prijsstijging hem alsnog het kanaal in duwen.

Bij de huidige tabel zou dat ongeveer één bericht per paar dagen zijn — dat is het punt.

## 7. Hermes aan de knop

De app zet een regel in `review_request` en stuurt het wekbericht (§3). Hermes werkt de
wachtrij af zodra dat bericht binnenkomt — er is geen poll-cron, dus een dag zonder klikken
kost nul agent-aanroepen.

```
kaartenjager reviews pending       openstaande verzoeken als JSON, zonder ze op te pakken
kaartenjager reviews take          idem, en markeert ze als opgepakt
kaartenjager reviews answer <id> --recommendation <kijken|overslaan|oplichterij>
                                   het oordeel (de tekst) gaat via stdin — meerregelige
                                   tekst door een shell-argument persen vraagt om
                                   aanhaalfouten
kaartenjager reviews fail <id> --reason <tekst>
kaartenjager reviews request <sleutel>   een verzoek in de wachtrij zetten
```

`request` staat er zodat de app het verzoek via het programma kan aanmaken in plaats van
zelf de tabel te vullen; welke van de twee de app gebruikt is een keuze bij het bouwen.

Hermes praat nooit rechtstreeks met SQLite; alles gaat via deze opdrachten. Let wel: het
schema staat daarmee níét op één plek — de Svelte-app leest en schrijft óók rechtstreeks.
Daarvoor is `PRAGMA user_version` er (§4): het programma hoogt hem op bij elke
schemawijziging, de app controleert hem bij het starten en weigert met een duidelijke
melding in plaats van half te werken op een schema dat hij niet kent. `kaartenjager update`
vernieuwt het binaire bestand; de databasemigratie doet het programma zelf bij de
eerstvolgende start.

**Wat er met twijfelgevallen gebeurt.** In de vorige opzet gingen advertenties met
`confidence = 'review'` automatisch naar Hermes. Nu Hermes een knop is, worden ze een
zichtbaar merkteken in de app: een oranje label *uitzoeken* met de reden erbij, en de
Hermes-knop eronder uitgelicht. Het programma beslist nog steeds wat twijfelachtig is; alleen
gaat er niets meer vanzelf naar de agent.

De skill is daarop meegegaan: `references/oordelen.md` beschreef `queue take` en
`queue done` en beschrijft nu de weg langs het wekbericht — bericht gezien, `reviews take`,
beoordelen, `reviews answer`, en bij een lege lijst niets melden. De tweedaagse cronjob
vervalt zonder opvolger.

**Wat Hermes beoordeelt** staat al in `references/oordelen.md`: zijn de foto's van de kaart
zelf of van de fabrikant, klinkt de beschrijving als iemand die weet wat hij verkoopt, staat
er iets over mining of reparatie, en bij een onbekend model wat het is en wat het waard is.

Het antwoord komt in de app onder de advertentie te staan, met de aanbeveling als kleurmerk.

**Wat er misgaat als niemand oplet:** een verzoek dat opgepakt is maar nooit beantwoord blijft
hangen. Daarom zet `reviews take` `taken_at` en hoogt het `attempts` op. Een verzoek waarvan
`taken_at` — niet `requested_at` — ouder is dan een uur komt terug in de wachtrij en wordt
bij de eerstvolgende `reviews take` opnieuw opgepakt. Na **drie** pogingen zet het programma
het zelf op mislukt, met als reden "drie pogingen mislukt"; zonder die grens blijft een
advertentie waar de agent op stukloopt eeuwig terugkeren, en elke terugkeer kost geld.

Mislukt is een **eindtoestand**: `reviews fail` zet naast `failed_reason` ook `answered_at`.
Zo blijft "open = `answered_at IS NULL`" kloppen, en — belangrijker — laat de unieke index
`review_one_open` daarna een nieuw verzoek voor dezelfde advertentie toe. De app toont het
mislukte verzoek met de reden en een knop om het opnieuw te proberen.

### De cronjobs na deze wijziging

| Naam | Wanneer | Agent | Wat |
|---|---|---|---|
| `kaartenjager-scan` | `0 8-22 * * *` | nee | Zoeken, beoordelen, hercontroleren, wegschrijven. Print alleen uitschieters en het wachtrij-vangnet |
| `kaartenjager-prijzen` | `0 9 * * 0` | ja | Wekelijkse prijsherziening, ongewijzigd |

`kaartenjager-oordeel` van 11:00 en 19:00 **vervalt**. Die werkte de stapel automatisch af,
en dat is nu de knop. Er komt géén poll-cron voor in de plaats: een agent-cron van elke tien
minuten zou 144 agent-aanroepen per dag kosten, vrijwel allemaal voor een lege wachtrij —
"stopt meteen als de lijst leeg is" verandert daar niets aan, want de agent is dan al
gestart en dat starten ís de kostenpost. Het wekbericht uit §3 vervangt het poll-mechanisme
volledig.

## 8. Zoektermen in de app

De enige configuratie die de app schrijft. Toevoegen, uitzetten, verwijderen.

`kaartenjager run` leest de termen uit de database. Bij de allereerste start worden de termen
uit `kaartenjager.toml` overgenomen, en dat wordt vastgelegd in `app_state['terms_seeded']`.

Zonder die markering zou het weghalen van je laatste zoekterm de hele lijst uit TOML
terugzetten, en dat is precies het tegenovergestelde van wat je bedoelde.

De grens van zestig zoekverzoeken per ronde wordt **in de app afgedwongen, op het moment van
toevoegen of aanzetten**: komt het aantal actieve termen maal het aantal bronnen erboven,
dan weigert de app de wijziging en zegt hij welke term er eerst uit moet. De controle in het
programma blijft bestaan, maar als vangnet: weigert een ronde alsnog, dan schrijft hij dat
naar `app_state['last_round_problems']` en kleurt de hartslag rood. Alleen bij het draaien
controleren zou betekenen dat één extra term de wachter elk uur stilletjes laat weigeren —
de fout hoort te vallen waar hij gemaakt wordt, in het formulier.

Bij het toevoegen kiest de app ook het soort (`kind`): kaart of onderdeel. Eén keuzerondje
in het formulier; het programma kan het niet raden.

De rest van de configuratie — modellen, prijzen, filters, machine, kast — blijft in TOML. Dat
is bewust: een verkeerde drempel legt de wachter stil, en dat wil je niet per ongeluk in een
webformulier doen. Komt de behoefte, dan volgt het later.

## 9. De borstvoedingsfout

Een boek over borstvoeding kwam in de resultaten. De oorzaak is de regel voor voedingen:

```toml
patterns = ["voeding", "psu", "power supply", "netzteil"]
```

Dat wordt getoetst met "komt voor in", en **"borstvoeding" bevat "voeding"**. Geen wattage in
de titel, dus het ging als twijfelgeval de stapel op. Hetzelfde zou gebeuren met kattenvoeding
en voedingssupplementen.

**De oplossing: woordgrenzen, maar alleen waar dat kan.** Een patroon dat uitsluitend uit
letters bestaat wordt op woordgrens getoetst — het teken ervoor en erna mag geen letter of
cijfer zijn. Een patroon met cijfers erin blijft een deelstring.

| Patroon | Toets | Gevolg |
|---|---|---|
| `voeding` | woordgrens | "borstvoeding" valt af, "voeding 850w" niet |
| `psu` | woordgrens | "PSU" blijft, "psufan" valt af |
| `riser` | woordgrens | "riser cable" blijft |
| `3090 ti` | deelstring | "rtx3090 ti" blijft werken |
| `4090` | deelstring | "RTX4090" blijft werken |

Waarom niet overal woordgrenzen: modelnummers lopen in advertentietitels vast tegen letters
aan. `rtx3090ti` zou met een woordgrens niet meer matchen, en dat is een veelvoorkomende
schrijfwijze. De regel "letters krijgen grenzen, cijfers niet" lost beide gevallen op zonder
dat er iets ingesteld hoeft te worden.

**Bijgesteld tijdens het bouwen: de grens is streng vóór het woord en soepel erachter.** Een
grens aan beide kanten leest "voedingen" — het gewone meervoud, dat volop in titels staat —
niet meer als een voeding, en veel erger: dan vuurt `kabel` niet meer op "kabels", en gaat een
zak voedingskabels alsnog als voeding door. Wat vóór het woord geplakt zit verandert de
betekenis ("borstvoeding" is geen voeding); wat erachter geplakt zit is meestal een meervoud.
Daarom mag een patroon gevolgd worden door `en`, `s` of `e`, mits daarna het woord ophoudt.

| Titel | Patroon | Uitkomst |
|---|---|---|
| "borstvoeding" | `voeding` | valt af — er zit iets vóór |
| "twee voedingen" | `voeding` | past — meervoud |
| "voedingssupplement" | `voeding` | valt af — `ssupplement` is geen uitgang |
| "psu kabels set" | `kabel` | uitgesloten, zoals bedoeld |
| "psufan" | `psu` | valt af — `fan` is geen uitgang |

## 10. Overgang

De huidige bestanden worden bij de eerste start ingelezen en daarna genegeerd:

| Van | Naar |
|---|---|
| `seen.json` | **niets** — zie hieronder |
| `queue.jsonl`, `queue.taken.jsonl` | `listing` plus `finding` met `confidence = 'review'` (de regels bevatten de volledige advertentie) |
| `recent.jsonl` | `listing`, `price_point` en `finding` |

`seen.json` wordt niet gemigreerd, want het kán niet en het hóéft niet. Kán niet: het
bestand bevat alleen sleutel en tijdstempel, terwijl `listing` titel en URL eist — migreren
zou duizenden spookrijen met lege titels opleveren. Hoeft niet: het bestand diende alleen de
al-gezien-poort, en die bestaat in het nieuwe model niet meer (§3) — elke ronde wordt alles
opnieuw gezeefd en beoordeeld.

Gemigreerde vondsten krijgen `became_a_find_at` én `pushed_at` op het migratiemoment, en
`app_state['last_visit']` wordt op datzelfde moment gezet. Zo begint de inbox leeg in plaats
van met tweehonderd "nieuwe" oude bekenden, en herhaalt Discord niets dat al gemeld was.

Daarna blijven de bestanden staan als terugval maar worden ze niet meer geschreven. Het
overzetten gebeurt één keer en is te herhalen met `kaartenjager migrate --from-files`.

## 11. Wat er getest wordt

| Wat | Hoe |
|---|---|
| Woordgrenzen | "borstvoeding", "kattenvoeding", "psufan" tegenover "voeding 850w", "rtx3090ti" |
| Schema | Aanmaken op een lege database, en tweemaal draaien mag niets kapotmaken |
| Prijsgeschiedenis | Dezelfde prijs tweemaal geeft één regel; een andere prijs geeft er twee |
| Nieuw sinds bezoek | Tijdstempel zetten, ronde draaien, telling controleren |
| Terug uit archief | Archiveren op €1.150, prijs naar €980, hoort weer in de inbox |
| Verdwenen | Twee hercontroles met "bestaat niet meer" zetten `gone_since`; een netwerkfout of 429 telt niet mee |
| Hercontrole | Een prijswijziging op de eigen pagina levert een `price_point`-regel, ook voor gearchiveerde advertenties |
| Wachtrij | Oppakken markeert en telt een poging; beantwoorden sluit af; een uur na `taken_at` komt hij terug |
| Uitschieterdrempel | 30% stuurt niets bij een grens van 35%, 40% wel |
| Overgang | Bestaande bestanden inlezen levert het verwachte aantal regels |
| Eén melding per advertentie | Tweemaal dezelfde vondst stuurt één bericht; tien procent lager stuurt een tweede |
| Uitgezette zoekterm | Bestaande vondsten blijven via hercontrole gevolgd; er verdwijnt en bevriest niets |
| Zoektermen leeghalen | Alles verwijderen laadt de lijst niet opnieuw uit TOML |
| Nieuw verloopt vanzelf | Een vondst van drie dagen oud telt niet meer als nieuw zonder klik |
| Nieuw blijft niet eeuwig nieuw | Vijf rondes over dezelfde advertentie laat hem één keer als nieuw tellen |
| Tweemaal klikken op Hermes | Levert één verzoek in de wachtrij op, geen twee |
| Tweemaal oppakken | Een tweede `reviews take` geeft niets zolang het eerste nog loopt, zodat de pogingengrens niet in een seconde opgaat |
| Meervouden | "voedingen" past op `voeding`, "kabels" sluit uit, "borstvoeding" niet |
| Oplichterij blijft uit Discord | Een vondst onder `suspicious_below` haalt de drempel wel maar wordt niet gemeld |
| Overgang herhalen | Een tweede `migrate --from-files` streept niet weg wat je nog niet gezien hebt, en telt onleesbare regels |
| Mislukt verzoek | Drie keer oppakken zonder antwoord zet hem op mislukt; daarna kan er een nieuw verzoek voor dezelfde advertentie |
| Eenmalige velden | Een ronde die de finding-rij bijwerkt laat `pushed_at`, `pushed_at_price` en `became_a_find_at` staan |
| Niet langer interessant | Prijs boven de drempel zet `still_a_find = 0` en haalt hem uit de inbox; een drempelverschuiving zonder prijsdaling maakt hem daarna niet opnieuw "nieuw" |
| Hartslag | Elke ronde schrijft `last_round_at`; de app toont de rode balk bij een verouderde stempel |
| Migratie zonder herrie | Na `migrate --from-files` is de inbox leeg en stuurt Discord niets opnieuw |
| Termgrens in de app | De term die over de grens gaat wordt in het formulier geweigerd, niet pas in de ronde |
| Gelijktijdig schrijven | Een klik tijdens het wegschrijven van een ronde wacht en faalt niet |

Alle controles blijven zonder netwerk draaien, met een database in een tijdelijke map.

## 12. Bewust niet

**Geen inlog op de app.** Hij draait op `openbinker`, bereikbaar via het tailnet. Wie daar
binnen is mag alles. Een gebruikersnaam en wachtwoord toevoegen voor één gebruiker is werk
zonder opbrengst.

**Geen volledige configuratie in de app.** Alleen zoektermen. Drempels en filters blijven in
TOML, waar een fout niet met één klik gemaakt is.

**Geen grafieken.** De prijsontwikkeling wordt een regel tekst — "€1.150 → €980 in twaalf
dagen" — niet een diagram. Bij drie prijspunten valt er niets te tekenen.

**Geen automatische beoordeling meer.** Dat was de vorige opzet; het is nu een knop. De
tweedaagse cronjob voor de stapel vervalt, de wekelijkse prijsherziening blijft.

**Geen bieden of kopen.** Nooit.
