# Kritische controle op de kaartenjager-spec

Ik wil dat je een ontwerpdocument aanvalt, niet goedkeurt. Een controle die eindigt met "ziet
er goed uit" is waardeloos. Ga ervan uit dat er fouten in zitten en zoek ze.

## Wat je moet lezen

Alles staat in `/home/yelsed/Projects/vinted-kaartenjager`, ook op
`github.com/yelsed/kaartenjager`.

| Bestand | Wat |
|---|---|
| `docs/superpowers/specs/2026-08-25-kaartenjager-app-design.md` | **Het ontwerp dat je moet controleren** |
| `docs/superpowers/specs/2026-08-24-kaartenjager-design.md` | Het eerdere ontwerp waar dit op voortbouwt |
| `src/` | De Rust-code zoals hij nu draait |
| `kaartenjager.example.toml` | De configuratie |
| `hermes-skill/` | De instructies voor de agent |

Lees het eerdere ontwerp ook. Een deel van de fouten zal zitten in tegenspraak tussen die twee.

## Wat dit systeem is

Een programma in Rust dat elk uur Vinted en Marktplaats afzoekt op ondergeprijsde
videokaarten, voedingen en riserkabels. Het draait als cronjob op een thuisserver via Hermes
Agent, een agent-omgeving met Discord als gateway.

De eerste versie meldde alles in Discord. Dat werkte niet: te lang, geen dagelijkse blik, niets
weg te leggen. Het nieuwe ontwerp verlegt dat naar een SQLite-database die door drie partijen
gedeeld wordt — het programma schrijft, een Svelte-app leest en schrijft, en een agent werkt
een wachtrij af. Discord houdt alleen echte uitschieters over.

De gebruiker is één persoon op één machine. Het draait al maanden zonder toezicht, dus fouten
die pas na weken zichtbaar worden zijn erger dan fouten die meteen opvallen.

## Waar je vooral naar moet kijken

**Stille fouten.** Wat in dit ontwerp kan stoppen met werken zonder dat iemand het merkt? Een
wachter die niets meldt ziet er precies zo uit als een markt zonder koopjes. Dat is de
gevaarlijkste storing die dit systeem kent.

**Fouten die pas na weken bijten.** Tabellen die volgroeien, tellers die niet aflopen, lijsten
die traag worden, tijdstempels die de verkeerde kant op schuiven. Redeneer expliciet: hoe ziet
dit eruit na een half jaar en tienduizend advertenties?

**Onbedoelde kosten.** Een van de cronjobs draait 144 keer per dag en mag vrijwel nooit een
agent-aanroep kosten. Klopt dat, en zijn er andere plekken waar per ongeluk een taalmodel
aangeroepen wordt?

**Gelijktijdigheid.** Drie schrijvers op één SQLite-bestand. Wat gebeurt er als twee dingen
tegelijk gebeuren, en wat als er eentje halverwege omvalt?

**Dingen die niet te bouwen zijn zoals beschreven.** Verzin niets erbij — wijs aan waar het
ontwerp iets aanneemt dat niet klopt of onvoldoende bepaald is om naar te handelen.

**Tegenspraak met het eerdere ontwerp of met de code.** Het ontwerp beschrijft een wijziging op
iets dat al draait. Is elk raakvlak benoemd?

## Wat ik al gevonden heb

Deze zijn opgelost. Noem ze niet opnieuw; ze staan hier zodat je weet welk soort fout ik zoek
en waar je dus verder moet kijken.

1. Het tijdstempel van een vondst werd elke ronde bijgewerkt, waardoor alles eeuwig "nieuw"
   zou blijven en de inbox nooit zou leeglopen
2. Er werd één zoekterm per advertentie onthouden, terwijl meerdere termen dezelfde advertentie
   vinden — het uitzetten van de ene zou hem onterecht bevriezen
3. Discord zou vijftien keer per dag hetzelfde bericht sturen zolang een advertentie online
   stond
4. Tweemaal op een knop drukken zou twee betaalde beoordelingen in de wachtrij zetten
5. Een klik die samenviel met het wegschrijven van een ronde zou "database is locked" geven
6. Het weghalen van de laatste zoekterm zou de hele lijst uit het configuratiebestand
   terugzetten
7. Een patroon werd met "komt voor in" getoetst, waardoor een boek over borstvoeding als
   voeding werd gemeld

Alle zeven zijn van dezelfde soort: iets dat op papier logisch leest en in bedrijf het
tegenovergestelde doet. Zoek meer daarvan.

## Hoe ik het antwoord wil

Per bevinding:

- **Wat er misgaat**, in één zin
- **Wanneer het misgaat** — concrete situatie, geen "zou kunnen"
- **Waarom het ontwerp het niet vangt**
- **Wat eraan te doen is**, kort

Gesorteerd op ernst. Twee gevonden fouten die echt kloppen zijn meer waard dan tien
opmerkingen over stijl.

Vind je iets waarvan je niet zeker bent, zeg dat er dan bij in plaats van het weg te laten of
zeker te brengen. En vind je echt niets in een onderdeel, zeg dan welk onderdeel je hebt
nagelopen — dan weet ik wat er gedekt is.

## Wat je niet hoeft te doen

Geen code schrijven. Geen alternatief ontwerp voorstellen. Geen mening over de taalkeuze, de
mappenindeling of het feit dat dit in het Nederlands staat. Het gaat om of dit ontwerp doet wat
het belooft, en of het dat over een half jaar nog steeds doet.
