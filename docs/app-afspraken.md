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

## Wat de app niet doet

Geen inlog — hij hangt achter het tailnet. Geen drempels, filters of kaartregels: die staan
in TOML, waar een fout niet met één klik gemaakt is. Geen grafieken; de prijsontwikkeling is
een regel tekst. En nooit bieden of kopen.
