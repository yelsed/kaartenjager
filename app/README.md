# De werkbank

De app naast de wachter. Het Rust-programma zoekt en schrijft; hier kijk je, leg je weg, volg
je, en vraag je Hermes om een oordeel.

SvelteKit met server-routes. De database wordt gelezen met **`node:sqlite`**, dat sinds Node 22
in Node zelf zit — dus geen `better-sqlite3`, geen native module, geen compiler op de server.

## Draaien

```sh
npm install
KAARTENJAGER_DB=~/.local/share/kaartenjager/kaartenjager.db npm run dev
```

De database moet al bestaan: draai eerst één keer `kaartenjager run`. De app maakt hem
bewust niet aan — het schema hoort bij het programma, en een app die zelf tabellen verzint
is precies hoe twee versies stilletjes uit elkaar gaan lopen.

## Op de server

```sh
npm ci
npm run build
node build
```

`adapter-node` levert een gewoon Node-proces. `kaartenjager-app.service` ernaast zet hem als
gebruikersservice neer; lees de kop van dat bestand voor wat je moet aanpassen.

| Omgevingsvariabele | Waarvoor |
|---|---|
| `KAARTENJAGER_DB` | Pad naar de database. Zonder deze valt hij terug op `~/.local/share/kaartenjager/kaartenjager.db` |
| `KAARTENJAGER_DISCORD_WEBHOOK` | Waar het wekbericht naar Hermes heen gaat. Zonder deze blijft de knop werken, maar wordt Hermes niet gewekt |
| `PORT`, `HOST` | Waar hij luistert. Standaard 3000 op alle adressen |

## Geen inlog

Hij hangt achter het tailnet; wie daar binnen is mag alles. Een gebruikersnaam en wachtwoord
voor één gebruiker is werk zonder opbrengst. Dat betekent wel: **niet aan het open internet
hangen.**

## Wat hij met de database doet

Alles wat hij mag en moet staat in [`../docs/app-afspraken.md`](../docs/app-afspraken.md).
De korte versie:

- `PRAGMA busy_timeout = 5000` op de verbinding, anders faalt een klik die samenvalt met het
  wegschrijven van een ronde met `SQLITE_BUSY`.
- `PRAGMA user_version` controleren en weigeren bij een onbekend schema. Half werken op een
  schema dat je verkeerd begrijpt is erger dan niet werken.
- De hartslag bovenaan tonen. Een wachter die om is ziet er precies zo uit als een markt
  zonder koopjes, en dat is de gevaarlijkste storing die dit systeem kent.
- De grens op het aantal zoektermen in het formulier afdwingen, niet pas in de ronde.
