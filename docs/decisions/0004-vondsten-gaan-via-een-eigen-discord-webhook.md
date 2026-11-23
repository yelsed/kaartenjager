# 0004 Vondsten gaan via een eigen Discord-webhook

Datum: 23 november 2026. Taak: TASK-2 / YEL-504.

## Besluit

Het programma post vondsten zelf naar een Discord-webhook (`[notify] discord_webhook`, of
`KAARTENJAGER_DISCORD_WEBHOOK` zoals de app hem al kent).

- Topvondst: een gewoon bericht dat de eigenaar noemt (`<@mention_user_id>`), zodat de telefoon
  trilt.
- Goede deal van een product met `notify_deals`: een bericht met de vlag
  `SUPPRESS_NOTIFICATIONS` (4096). Het staat in het kanaal, de telefoon blijft stil.

Is er een webhook, dan drukt de ronde geen vondsten meer af; alleen de waarschuwing over een
wachtende Hermes-beoordeling gaat nog via de cron. Zonder webhook blijft de oude route.

Een vondst krijgt pas zijn stempel als de webhook hem aannam. Een mislukte post wordt de
volgende ronde opnieuw geprobeerd.

## Waarom

De eigenaar wil echte meldingen op de telefoon en koos Discord als enige kanaal, met
topvondsten luid en deals stil. De Hermes-route post wat de cron afdrukt en kan geen stille
vlag zetten. Een eigen webhook kan dat wel, en maakt de vondsten los van Hermes, wat Widening 3
(hosting naast Hermes) ruimte geeft.

Overwogen: ntfy, Pushover en mail (afgewezen door de eigenaar: één kanaal), Hermes voor
topvondsten en een webhook voor deals (twee routes naar één kanaal), deals als dagoverzicht via
Hermes (of het trilt hangt dan af van de kanaalinstelling).
