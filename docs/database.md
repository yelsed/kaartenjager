# De database

Eén SQLite-bestand in WAL-modus, standaard `~/.local/share/kaartenjager/kaartenjager.db`
(`KAARTENJAGER_DB` wijst een ander aan). Het programma schrijft elke ronde, Hermes werkt de
wachtrij af, en de app leest en schrijft wat je aanklikt. Het schema hoort bij het programma
(`src/db.rs`); de app migreert niets en weigert een `PRAGMA user_version` die hij niet kent.

## Versies

| Versie | Wat erbij kwam |
|---|---|
| 1 | De eerste database, overgezet uit de losse JSON-bestanden, met de zoektermen. |
| 2 | `sighting` (prijs en belangstelling door de tijd), `gone_reason` en `posted_at` op `listing`. |
| 3 | Producten: tabel `product`, `finding.product`, `finding.level`, `search_term.product`. |

Het programma migreert bij het openen, alle stappen samen in één transactie. Bij de stap naar 3
zet het `app_state.terms_from_legacy`. De eerste `sync_products` geeft bestaande zoektermen en
vondsten hun product. Een product dat al termen heeft telt als gezaaid. Komen de producten uit
de oude lijsten (geen `[[product]]` in de configuratie), dan telt elk product als gezaaid, ook
een dat de eigenaar leeggemaakt heeft; met `[[product]]` krijgt een product zonder een enkele
bestaande term zijn `search_terms`, want dan is hij nieuw.

## Tabellen

**listing**: één rij per advertentie die ooit een vondst werd. Sleutel `key` is
`<bron>:<id>`. Houdt titel, adres, plaats, verkoper, staat, bezorging, en wanneer hij gezien,
nagekeken en verdwenen is (`gone_since`, `gone_reason` sold of removed).

**sighting**: de prijs en belangstelling per waarneming. Een nieuwe rij alleen als de prijs
veranderde of bij een hercontrole.

**finding**: het oordeel over een advertentie.
- `matched_as`, `kind` (card, part, unknown), `confidence` (clear, review).
- `percent_under_market`, `euros_under_market`, `reasons`, `warnings` (JSON-lijsten).
- `product`: `product.id` van de regel die hem herkende. NULL bij vondsten van voor schema 3
  die nog geen ronde zagen.
- `level`: `top` of `deal` (`docs/decisions/0002`).
- `still_a_find`, `pushed_at`, `pushed_at_price`: of hij nog telt en of hij al gemeld is. Ook een
  vondst die stil blijft krijgt `pushed_at`; met een webhook pas als Discord hem aannam.

**decision**: wat jij ermee deed: `state` inbox, watching of archived, en de prijs bij het
archiveren, zodat een daling van meer dan tien procent hem terug in de inbox brengt.

**review_request**: de wachtrij voor Hermes. Een unieke index houdt één open verzoek per
advertentie.

**search_term**: de zoektermen.
- `term` (sleutel, kleine letters), `enabled`, `added_at`, `added_by` (config of app).
- `kind`: `card` of `part` voor termen van voor schema 3, `product` voor nieuwe.
- `product`: het product waar de term bij hoort.

**product**: wat er bestaat komt uit de configuratie, of het aan staat is van de app
(`docs/decisions/0003`).
- `id`, `name`, `position`: elke start opnieuw uit de configuratie geschreven; een product dat
  uit de configuratie verdwijnt, verdwijnt hier ook. Zijn zoektermen en vondsten houden hun
  `product`, worden niet meer gezocht en staan niet in de inbox; komt het product terug, dan
  horen ze er weer bij zonder dat het opnieuw gezaaid wordt.
- `enabled`: alleen de app schrijft dit. Uit: zijn zoektermen gaan niet mee in de ronde, hij
  meldt niets en staat niet in de inbox.
- `seeded_at`: wanneer zijn `search_terms` in `search_term` gezet zijn. Dat gebeurt één keer.

**app_state**: losse waarden als naam en tekst.
- `last_round_at`, `last_round_problems`, `round_gap_seconds`, `round_running_since`,
  `last_round_seconds`: de hartslag van de wachter, en hoe lang de laatste ronde duurde.
- `last_visit`, `previous_visit`: wat de app als nieuw toont.
- `terms_seeded`, `terms_from_legacy`: of de zoektermen al gezaaid zijn (zie Versies).
- `max_search_terms`: hoeveel zoektermen er aan mogen; het programma rekent het uit de bronnen.
- `last_recheck_at`, `last_review_nag_at`, `migrated_from_files_at`, `doctor_probe`.
