# Kaartenjager, deel twee: van meldingen naar een werkbank

**Datum:** 25 augustus 2026
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

Er draait al een Svelte-app op `openbinker` voor een transfermarkt-schraper. Kaartenjager
krijgt daar een plek naast.

**Nog vast te stellen bij het bouwen:** of dat een SvelteKit-project is met server-routes, en
of kaartenjager er een route in krijgt of een eigen app naast wordt. Het ontwerp hieronder gaat
uit van SvelteKit met server-routes en `better-sqlite3`; is het een losse Svelte-app zonder
server-kant, dan verschuift alleen waar de databasecode staat, niet wat hij doet.

## 2. Wat er blijft en wat er verandert

| | Nu | Straks |
|---|---|---|
| Zoeken en beoordelen | Rust, elk uur, cron | **onveranderd** |
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
   │  Svelte-app     │   │  kaartenjager reviews          │
   │  server-routes  │   │    ← Hermes-cron elke 10 min   │
   │  better-sqlite3 │   │    verzoeken ophalen,          │
   └─────────────────┘   │    oordelen, terugschrijven    │
                         └────────────────────────────────┘
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

**De app roept Hermes nooit rechtstreeks aan.** De knop zet een regel in `review_request`.
Een cronjob haalt openstaande verzoeken op, beoordeelt ze en schrijft het antwoord terug.

Waarom een wachtrij en geen aanroep: een app die op een agent wacht loopt vast als de agent
traag is, verliest het verzoek bij een herstart, en moet een time-out afhandelen die niemand
wil bedenken. Een regel in een tabel overleeft dat allemaal. De prijs is wachttijd — met een
cron van tien minuten zie je het antwoord binnen tien minuten, en dat kan naar twee als het
te traag voelt.

**De database is de enige koppeling.** Er is geen API, geen poort, geen dienst die moet
blijven leven. Valt de app om, dan blijft het zoeken doorgaan. Valt het zoeken om, dan blijft
de app tonen wat er al was.

## 4. Het gegevensmodel

```sql
PRAGMA journal_mode = WAL;

-- Elke advertentie die ooit langskwam, ook als hij later verdwijnt.
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
  missed_rounds  INTEGER NOT NULL DEFAULT 0,
  gone_since     INTEGER                     -- pas na twee gemiste rondes
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

-- Wat het programma concludeerde. Overschreven bij elke nieuwe ronde.
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
  verdict        TEXT,                       -- de tekst van Hermes
  recommendation TEXT,                       -- kijken | overslaan | oplichterij
  failed_reason  TEXT
);

-- Zoektermen, beheerd vanuit de app.
CREATE TABLE search_term (
  term      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL,                   -- card | part
  enabled   INTEGER NOT NULL DEFAULT 1,
  added_at  INTEGER NOT NULL,
  added_by  TEXT NOT NULL DEFAULT 'app'      -- app | config | hermes
);

-- Los-vaste waarden, waaronder wanneer je voor het laatst keek.
CREATE TABLE app_state (
  name   TEXT PRIMARY KEY,
  value  TEXT NOT NULL
);

CREATE INDEX listing_last_seen ON listing(last_seen);
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
als nieuw en wordt `became_a_find_at` vernieuwd. Dat is namelijk echt nieuws.

### Wanneer iets "weg" is, en wanneer niet

Een advertentie die niet meer in de resultaten voorkomt is meestal verkocht. Maar hij kan ook
ontbreken omdat **jij de zoekterm hebt uitgezet die hem vond** — en dan zou de hele lijst in
één keer als verkocht gemarkeerd worden.

Daarom onthoudt `listing` **alle** termen die hem ooit opleverden, en telt een afwezigheid
alleen als er nog minstens één van die termen aanstaat. Eén term onthouden zou niet werken:
"rtx 4090" en "geforce rtx" vinden dezelfde kaart, en dan zou het uitzetten van de ene de
advertentie bevriezen terwijl de andere hem gewoon nog vindt.

Om dezelfde reden zijn er **twee** rondes nodig voordat `gone_since` gevuld wordt: Vinted
levert niet elke ronde exact dezelfde selectie, dus één keer ontbreken zegt niets.

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

### Archiveren en volgen

**Archiveren** legt de advertentie weg en onthoudt de prijs van dat moment. Zakt die later
meer dan tien procent, dan komt hij terug in de inbox met de vlag *"was gearchiveerd op
€1.150, staat nu op €980"*. Dat vangt precies de verkoper die na twee weken toegeeft.

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
zakt — dan is het nieuws.

Bij de huidige tabel zou dat ongeveer één bericht per paar dagen zijn — dat is het punt.

## 7. Hermes aan de knop

De app zet een regel in `review_request`. Een cronjob van elke tien minuten werkt hem af.

```
kaartenjager reviews take          openstaande verzoeken als JSON, markeert ze als opgepakt
kaartenjager reviews answer <id> --verdict <tekst> --recommendation <kijken|overslaan|oplichterij>
kaartenjager reviews fail <id> --reason <tekst>
```

De database blijft zo achter het Rust-programma; Hermes praat nooit rechtstreeks met SQLite,
zodat het schema op één plek gedefinieerd is.

**Wat er met twijfelgevallen gebeurt.** In de vorige opzet gingen advertenties met
`confidence = 'review'` automatisch naar Hermes. Nu Hermes een knop is, worden ze een
zichtbaar merkteken in de app: een oranje label *uitzoeken* met de reden erbij, en de
Hermes-knop eronder uitgelicht. Het programma beslist nog steeds wat twijfelachtig is; alleen
gaat er niets meer vanzelf naar de agent.

De skill moet daarop mee: `references/oordelen.md` beschrijft nu `queue take` en `queue done`,
en dat wordt `reviews take` en `reviews answer`. De tweedaagse cronjob vervalt en er komt een
van elke tien minuten voor in de plaats.

**Wat Hermes beoordeelt** staat al in `references/oordelen.md`: zijn de foto's van de kaart
zelf of van de fabrikant, klinkt de beschrijving als iemand die weet wat hij verkoopt, staat
er iets over mining of reparatie, en bij een onbekend model wat het is en wat het waard is.

Het antwoord komt in de app onder de advertentie te staan, met de aanbeveling als kleurmerk.

**Wat er misgaat als niemand oplet:** een verzoek dat opgepakt is maar nooit beantwoord blijft
hangen. Daarom zet `reviews take` een tijdstempel, en geldt een verzoek dat een uur oud is als
mislukt en komt het weer in de wachtrij.

### De cronjobs na deze wijziging

| Naam | Wanneer | Agent | Wat |
|---|---|---|---|
| `kaartenjager-scan` | `0 8-22 * * *` | nee | Zoeken, beoordelen, wegschrijven. Print alleen uitschieters |
| `kaartenjager-reviews` | `*/10 * * * *` | **ja** | Wachtrij afwerken. Doet niets als hij leeg is |
| `kaartenjager-prijzen` | `0 9 * * 0` | ja | Wekelijkse prijsherziening, ongewijzigd |

`kaartenjager-oordeel` van 11:00 en 19:00 **vervalt**. Die werkte de stapel automatisch af, en
dat is nu de knop.

Die wachtrij-cronjob draait 144 keer per dag en doet vrijwel altijd niets. Dat mag geen
agent-aanroep kosten: hij begint met `kaartenjager reviews take`, en als daar een lege lijst
uit komt stopt hij zonder verder te denken. De skill moet dat expliciet als eerste stap
voorschrijven.

## 8. Zoektermen in de app

De enige configuratie die de app schrijft. Toevoegen, uitzetten, verwijderen.

`kaartenjager run` leest de termen uit de database. Bij de allereerste start worden de termen
uit `kaartenjager.toml` overgenomen, en dat wordt vastgelegd in `app_state['terms_seeded']`.

Zonder die markering zou het weghalen van je laatste zoekterm de hele lijst uit TOML
terugzetten, en dat is precies het tegenovergestelde van wat je bedoelde. De grens van zestig verzoeken per ronde blijft gelden: staan
er te veel termen aan, dan weigert de ronde te starten en zegt de app welke je uit moet zetten.

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

## 10. Overgang

De huidige bestanden worden bij de eerste start ingelezen en daarna genegeerd:

| Van | Naar |
|---|---|
| `seen.json` | `listing` met `first_seen`, zonder verdere gegevens |
| `queue.jsonl`, `queue.taken.jsonl` | `finding` met `confidence = 'review'` |
| `recent.jsonl` | `listing`, `price_point` en `finding` |

Daarna blijven ze staan als terugval maar worden ze niet meer geschreven. Het overzetten
gebeurt één keer en is te herhalen met `kaartenjager migrate --from-files`.

## 11. Wat er getest wordt

| Wat | Hoe |
|---|---|
| Woordgrenzen | "borstvoeding", "kattenvoeding", "psufan" tegenover "voeding 850w", "rtx3090ti" |
| Schema | Aanmaken op een lege database, en tweemaal draaien mag niets kapotmaken |
| Prijsgeschiedenis | Dezelfde prijs tweemaal geeft één regel; een andere prijs geeft er twee |
| Nieuw sinds bezoek | Tijdstempel zetten, ronde draaien, telling controleren |
| Terug uit archief | Archiveren op €1.150, prijs naar €980, hoort weer in de inbox |
| Verdwenen | Twee rondes zonder de advertentie zet `gone_since`, maar alleen als de zoekterm die hem vond nog aanstaat |
| Wachtrij | Oppakken markeert, beantwoorden sluit af, een uur oud komt terug |
| Uitschieterdrempel | 30% stuurt niets bij een grens van 35%, 40% wel |
| Overgang | Bestaande bestanden inlezen levert het verwachte aantal regels |
| Eén melding per advertentie | Tweemaal dezelfde vondst stuurt één bericht; tien procent lager stuurt een tweede |
| Uitgezette zoekterm | Advertenties eraan verdwijnen niet, ze bevriezen |
| Zoektermen leeghalen | Alles verwijderen laadt de lijst niet opnieuw uit TOML |
| Nieuw verloopt vanzelf | Een vondst van drie dagen oud telt niet meer als nieuw zonder klik |
| Nieuw blijft niet eeuwig nieuw | Vijf rondes over dezelfde advertentie laat hem één keer als nieuw tellen |
| Tweemaal klikken op Hermes | Levert één verzoek in de wachtrij op, geen twee |
| Twee zoektermen, één advertentie | De ene uitzetten bevriest hem niet zolang de andere aanstaat |
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
