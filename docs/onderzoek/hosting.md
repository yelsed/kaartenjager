# Waar Kaartenjager draait

Onderzoek voor Widening 3 (TASK-3 / YEL-501), 23 november 2026. Het besluit staat in
`docs/decisions/0006`.

De inventaris komt uit de vault en de repository, niet van de machine zelf: de eigenaar koos
(23 november 2026) om openbinker niet langs te lopen. Het opruimen en de precieze inventaris
horen bij YEL-409 in the-whole-system.

## Wat openbinker de app nu geeft

openbinker is een Hetzner-VPS: 2 vCPU, 4 GB geheugen, 40 GB schijf, niet te vergroten, op het
tailnet. Hermes draait er, en Kaartenjager is zijn belangrijkste taak.

| Wat | Hoe | Waar het staat |
|---|---|---|
| De ronde elke vijf minuten, 08:00 tot 22:00 | Hermes-cronjob `kaartenjager-scan`, `no_agent`, script `kaartenjager run` | README, "Hoe het werkt" |
| Topvondsten in Discord | De cron levert de uitvoer van de ronde aan Hermes, die post. Na Widening 2 met een webhook post het programma zelf. | `src/report.rs`, ADR 0004 |
| De knop "Hermes laten kijken" | De app post een bericht in het kanaal; Hermes pakt de wachtrij met `kaartenjager reviews take`, op dezelfde machine en dezelfde database | `app/src/lib/server/hermes.ts` |
| De prijsherziening op zondag | Hermes-cronjob `kaartenjager-prijzen` met de skill `kaartenjager`; het programma keurt het voorstel | README |
| De app | systemd-gebruikersservice `kaartenjager-app` op poort 5173, alleen op het tailnet, met linger | `app/kaartenjager-app.service` |
| De browser voor Vinted | Chromium op de server, als root met `no_sandbox` | `deploy.sh`, README "Chromium erbij" |
| Het IP-adres dat de marktplaatsen zien | Een datacenteradres van Hetzner | |
| Altijd aan | Een datacenter: stroom, netwerk en herstart regelt Hetzner | |
| Backups | Geen bekende backup van de database. Hetzner-snapshots alleen als die aan staan. | |

Wat de app nodig heeft: weinig. Een ronde met 18 zoektermen duurde vanaf omarchy 76 seconden
(Widening 2); Chromium vraagt tijdens de ronde een paar honderd MB. De database is enkele MB.

## De alternatieven

### Blijven op openbinker

- **Behouden:** alles hierboven. Hermes en de app delen machine en database, dus de knop en de
  prijsherziening werken zonder netwerk ertussen. Altijd aan.
- **Verliezen:** niets.
- **Risico:** Vinted ziet een datacenteradres. Sinds 2.0 zoekt Kaartenjager Vinted via een echte
  browser omdat het JSON-antwoord zonder dat niet meer kwam; een datacenteradres is het eerste
  wat zo'n bescherming strenger behandelt. Raakt Vinted daar dicht, dan is er geen knop om aan te
  draaien behalve een ander adres.
- **Kosten:** het VPS-abonnement, dat er voor Hermes toch al is.

### Een virtuele machine of container op ThuisCLuster

- **Gewonnen:** een thuisadres, dat marktplaatsen minder snel weren. Backups via de Proxmox
  Backup Server die er al draait. Geen extra abonnement.
- **Verliezen:** Hermes staat niet meer naast de database. De knop en de prijsherziening moeten
  dan over het tailnet (`ssh <machine> kaartenjager reviews take`), of Hermes verhuist mee.
- **Risico:** het cluster bestaat uit laptops met een quorum van 3 van 4. Zijn er twee tegelijk
  uit, dan start er niets meer; een draaiende machine blijft wel draaien. Een stroomstoring thuis
  legt de wachter stil, en dat ziet de app pas na drie gemiste rondes.
- **Kosten:** geen geld; een deel van het geheugen van een node.

### De virtuele machine paperclip

- **Gewonnen:** een thuisadres en Paperclip, de keten en dev-portal op dezelfde machine.
- **Verliezen:** zoals op het cluster, plus rust: paperclip draait vaak zwaar belast (de
  design-library koos er daarom ook niet voor), en een ronde met Chromium erbij concurreert dan
  met builds en agents.
- **Kosten:** geen geld.

### De mini-pc uit Widening 4

- **Gewonnen:** een thuisadres en een eigen machine die niets anders doet dan het cluster en de
  CI-wachtrij, met 16 GB of meer.
- **Verliezen:** zoals op het cluster.
- **Kosten:** de aankoop (Widening 4), en hij is er nog niet.

## Wat volgt

Zolang Vinted vanaf openbinker werkt, wint blijven: Hermes blijft (antwoord van de eigenaar,
23 november 2026) en niets anders weegt op tegen het verlies van de gedeelde machine. Het
verhuizen heeft één goede reden, het adres, en die reden is meetbaar: de ronde houdt per bron
bij wanneer die ons tegenhield (`blocked_until:<bron>`, `block_strikes:<bron>`) en de app toont
het in de hartslag. ADR 0006 maakt dat de aanleiding.
