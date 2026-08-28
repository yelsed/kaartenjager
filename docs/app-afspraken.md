# Wat de app moet doen

De app staat in [`../app/`](../app/). Hij deelt één databasebestand met het programma en met
Hermes, en dat maakt een paar afspraken bindend. Ze staan hier apart van de code omdat het
programma ze afdwingt of erop rekent — wie de app herschrijft of vervangt, houdt zich hieraan.

## De verbinding

`node:sqlite` zit sinds Node 22 in Node zelf, dus er is geen `better-sqlite3` nodig en dus ook
geen compiler op de server.

```js
import { DatabaseSync } from 'node:sqlite';

const db = new DatabaseSync(process.env.KAARTENJAGER_DB
  ?? `${homedir()}/.local/share/kaartenjager/kaartenjager.db`);

db.exec('PRAGMA busy_timeout = 5000');
```

`busy_timeout` is niet optioneel. Zonder die regel geeft een klik die precies samenvalt met
het wegschrijven van een ronde meteen `SQLITE_BUSY` in plaats van even te wachten. Vijf
seconden is ruim: een ronde schrijft in een fractie daarvan.

WAL staat al aan; die instelling zit in het bestand zelf en hoeft niet herhaald te worden.

## De schemaversie controleren

```js
const { user_version } = db.prepare('PRAGMA user_version').get();
if (user_version !== 1) {
  throw new Error(`kaartenjager.db heeft schema ${user_version}, deze app kent 1`);
}
```

Is de versie 0, dan bestaat de database nog niet of is hij leeg. De app maakt hem bewust niet
zelf aan: het schema hoort bij het programma, en een app die zelf tabellen verzint is precies
hoe twee versies stilletjes uit elkaar gaan lopen. Zeg dan dat er eerst `kaartenjager run`
moet draaien.

Het programma hoogt `user_version` op bij elke schemawijziging en migreert zichzelf bij de
eerstvolgende start. De app hoort te wéigeren bij een versie die hij niet kent, met een
zichtbare melding. Half werken op een schema dat je verkeerd begrijpt is erger dan niet
werken: dan lijkt het te werken.

## De hartslag tonen

Dit is de belangrijkste weergave in de hele app, want een dode wachter ziet er precies zo uit
als een markt zonder koopjes.

```sql
SELECT value FROM app_state WHERE name = 'last_round_at';
SELECT value FROM app_state WHERE name = 'last_round_problems';  -- JSON-lijst
```

Is `last_round_at` binnen het dagvenster (08:00–22:00) ouder dan twee uur, dan hoort er een
rode balk bovenaan te staan, met de inhoud van `last_round_problems` eronder. Ontbreekt
`last_round_at` helemaal, dan heeft er nog nooit een ronde gedraaid.

## De weergaven

| Tabblad | Waar het op neerkomt |
|---|---|
| Inbox | `decision.state = 'inbox'`, `finding.still_a_find = 1`, `listing.gone_since IS NULL`. Nieuw bovenaan: `finding.became_a_find_at > app_state['last_visit']` |
| Volglijst | `decision.state = 'watching'` |
| Archief | `decision.state = 'archived'`, plus alles met `gone_since IS NOT NULL` of `still_a_find = 0` |
| Zoektermen | `search_term` |

Hooguit vijftig regels per weergave, met een knop om meer te laden.

**De inbox heeft een afvoer.** Een vondst waarvan de advertentie verdwenen is (`gone_since`)
of die geen vondst meer is (`still_a_find = 0`) hoort uit de inbox te vallen en in het
archief te verschijnen, met het waarom erbij. Zonder die regel staan er na een half jaar
honderden dode regels onder "EERDER".

**Terug uit het archief is een leesregel, geen statuswijziging.** Staat iets op `archived` en
ligt de laatste prijs onder 90% van `price_when_archived`, dan toont de inbox hem opnieuw met
de vlag erbij. `decision.state` blijft `archived` tot jij er iets mee doet — zo schrijven de
app en de scanner nooit allebei aan dezelfde beslissing.

## "Alles gezien"

```sql
UPDATE app_state SET value = (SELECT value FROM app_state WHERE name = 'last_visit')
  WHERE name = 'previous_visit';
UPDATE app_state SET value = :now WHERE name = 'last_visit';
```

Eerst de oude stand naar `previous_visit`, zodat één klik terug te draaien is. Daarnaast
geldt een automatische grens: vondsten ouder dan achtenveertig uur tellen niet meer als
nieuw, ook zonder klik. Anders loopt het vol als je een week niet kijkt.

## De Hermes-knop

Twee dingen, in deze volgorde:

1. Het verzoek in de wachtrij zetten. Ofwel rechtstreeks:

   ```sql
   INSERT INTO review_request (key, requested_at) VALUES (?, ?) ON CONFLICT DO NOTHING;
   ```

   ofwel via het programma: `kaartenjager reviews request <sleutel>`. De unieke index
   `review_one_open` zorgt dat tweemaal drukken één verzoek oplevert; **vang die
   constraint-fout op** in plaats van hem als storing te tonen.

2. Het wekbericht sturen, via de Discord-webhook van het kanaal:

   ```
   review gevraagd: vinted:9758884187
   ```

Zonder stap 2 gebeurt er niets tot iemand het handmatig vraagt: er is geen poll-cron, want
die zou 144 agent-aanroepen per dag kosten voor een wachtrij die vrijwel altijd leeg is. Gaat
het bericht verloren, dan blijft het verzoek gewoon staan en print de uurlijkse scan na een
uur een regel in Discord.

Toon bij een openstaand verzoek hoe lang het al wacht, en bij `failed_reason` de reden met
een knop om het opnieuw te proberen.

## De zoektermen, en de grens

De app is de enige die `search_term` beheert. Bij toevoegen of aanzetten geldt:

```sql
SELECT value FROM app_state WHERE name = 'max_search_terms';
```

Het programma schrijft dat getal elke ronde weg, want het hangt af van het aantal bronnen en
dat staat in TOML — dat de app niet leest. Bij twee bronnen is het dertig.

**Weiger de wijziging in het formulier** zodra het aantal aanstaande termen daarboven
uitkomt, bij toevoegen én bij aanzetten, en zeg dat er eerst een uit moet. Het programma
weigert de ronde óók bij overschrijding, maar dat gebeurt dan elk uur op de server — de fout
hoort te vallen waar hij gemaakt wordt.

Bij toevoegen hoort ook `kind` gekozen te worden: `card` of `part`. Het programma kan dat
niet raden.

Een term uitzetten heeft geen gevolgen voor bestaande vondsten: die worden via hun eigen
pagina gevolgd, niet via de zoekresultaten.

## Zelf een ronde starten

De knop "nu zoeken" start `kaartenjager run` als los proces en wacht er niet op — een ronde
duurt ruim een minuut, en daar een formulierpost op laten wachten levert alleen een pagina op
die lijkt vast te zitten. Eén tegelijk: twee rondes naast elkaar leveren niets extra's op en
verdubbelen wel het aantal verzoeken aan Vinted en Marktplaats.

Het pad naar het programma komt uit `KAARTENJAGER_BIN`, met `~/.local/bin/kaartenjager` als
terugval.

Wat het programma op stdout zet gaat bij de cronjob naar Discord. Start jij de ronde vanuit de
app, dan is er geen cronjob die dat doet — dus stuurt de app die uitvoer zelf door naar de
webhook. Zonder die regel zou de melding verloren gaan terwijl `pushed_at` wél gestempeld is,
en dan krijg je hem ook later nooit meer.

## Formulierposts en herkomst

SvelteKit weigert standaard elke formulierpost waarvan de `Origin`-kop niet exact gelijk is aan
de herkomst van de server, schema en al. `adapter-node` kent zijn eigen schema niet en gokt
daarom `https` zolang `ORIGIN` niet gezet is, terwijl de app over gewoon HTTP draait. Gevolg:
403 op elke knop, en niets in het log dat daarop wijst.

Daarom staat die controle uit (`csrf: { trustedOrigins: ['*'] }` in `vite.config.ts`) en doet
`src/hooks.server.ts` hem zelf, vergelijkend op **host** in plaats van op host én schema. Dat
houdt precies de aanval tegen waar CSRF over gaat en blijft werken of je de app nu via het IP,
via `openbinker` of via de MagicDNS-naam opent.

**Let op bij het testen.** Die hele controle staat uit in ontwikkelmodus
(`if (!__SVELTEKIT_DEV__)` in `runtime/server/respond.js`). Een knop die het met `npm run dev`
doet, zegt dus niets over productie. `npm run smoke` start daarom de productiebouw en post elke
actie mét een `Origin`-kop; dat is de enige manier waarop deze fout zichtbaar wordt.

## De levensloop van een advertentie

Sinds schema 2 legt het programma vast hoe een advertentie zich gedroeg, want dat is de enige
manier om te beantwoorden hoe snel een koopje werkelijk wegging.

| Waar | Wat |
|---|---|
| `listing.posted_at` | Wanneer de verkoper hem plaatste. Vinted noemt dat niet, maar de foto's dragen hun uploadmoment mee en die maakt een verkoper bij het plaatsen. Marktplaats geeft alleen "Vandaag", te grof, dus daar blijft het leeg |
| `listing.first_seen` | Wanneer wij hem voor het eerst zagen. Het verschil met `posted_at` is precies hoeveel je te laat was |
| `listing.last_seen` / `last_checked` | Laatst gezien in de resultaten, en laatst nagekeken op zijn eigen pagina. Het laatste levensteken is de hoogste van die twee |
| `listing.gone_since` / `gone_reason` | Wanneer hij weg was, en of dat verkocht was of weggehaald |
| `sighting` | Eén regel per waarneming waarin iets veranderde: prijs, kijkers of favorieten |

`sighting` verving `price_point`, dat alleen prijzen bijhield. Er wordt alleen geschreven bij
verandering, dus de reeks wordt vanzelf dicht waar het spannend is — bij een echt koopje lopen
de favorieten binnen minuten op — en blijft leeg waar niets gebeurt. Zonder die regel zou een
ronde van vijf minuten honderden regels per advertentie per dag opleveren.

`view_count` bestaat wel in het antwoord van Vinted maar staat in zoekresultaten altijd op
nul; toon hem alleen als er echt iets in staat.

**Hoe "verkocht" herkend wordt.** Gemeten op 26 augustus 2026, op echte pagina's:

| Toestand | Wat de pagina doet |
|---|---|
| Te koop | HTTP 200 met een `application/ld+json`-blok, `availability: InStock` |
| Verkocht | HTTP 200 **zonder** dat blok |
| Weggehaald | HTTP 404 |

Het woord "Verkocht" staat wél in een verkochte pagina, maar alleen in de taalbestanden die op
élke pagina meekomen — daar valt dus niet op te toetsen. Een pagina zonder blok is daarom het
signaal, met één voorbehoud: zo ziet een opmaakwijziging er ook uit. Komen in één ronde meer
dan drie hercontroles onleesbaar terug én zijn dat er meer dan de leesbare, dan wordt er niets
als verdwenen gemarkeerd en komt het als probleem naar boven. Anders zou één wijziging bij
Vinted de hele inbox leegvegen.

De app zet dit onder "hoe het verliep" in de uitgeklapte kaart: geplaatst, gevonden, hoe lang
daarna, verkocht of weggehaald, en hoe lang hij online stond.

## Wat de app niet doet

Geen inlog — hij hangt achter het tailnet. Geen grafieken; de prijsontwikkeling is een regel
tekst. En nooit bieden of kopen.

Drempels, filters en kaartregels stonden hier lang bij, omdat een fout daarin niet met één
klik gemaakt moest zijn. Dat is omgedraaid: ze staan nu onder Instellingen, als het echte
TOML-bestand in een tekstvak. De reden is dat de helft die wél in de app stond — de
zoektermen — precies de helft is die niets bepaalt, en dat wie een regel voor een nieuw
kaartmodel wilde alsnog met een shell op de server moest zien te komen.

De angst was terecht, dus die is opgelost in plaats van weggewuifd. Bewaren schrijft de
nieuwe versie eerst naast het bestand, draait daar `kaartenjager check` op — hetzelfde
programma dat de wachter zelf gebruikt, geen nabouw in TypeScript — en vervangt pas als die
goedkeurt. De vorige versie blijft als `.vorige` staan. De ergste uitkomst is dus een
afgekeurde bewaarpoging met de reden erbij, in plaats van een wachter die vannacht stilvalt.
`app/smoke.sh` schrijft twee keer expres iets kapots weg en controleert dat het bestand niet
veranderde.
