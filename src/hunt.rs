//! One round: search every enabled term on every source, sieve, judge, recheck what is
//! already being followed, write it all away in one transaction, and print only the outliers.

use crate::config::{Settings, MAX_REQUESTS_PER_ROUND};
use crate::db::{Database, PushCandidate};
use crate::detail::{self, PageState};
use crate::filter::Sieve;
use crate::http::HttpClient;
use crate::listing::{Confidence, Delivery, Finding, Listing};
use crate::pricing::PriceTable;
use crate::sources::{marktplaats::Marktplaats, vinted::Vinted, Source};
use std::collections::{BTreeMap, BTreeSet};

/// How many listings get their own page fetched per round. Rotating oldest-first, this brings
/// every followed listing round about once a day at fifteen rounds.
pub const RECHECKS_PER_ROUND: usize = 30;

/// Wat er wél elke ronde nagekeken wordt, ook tussen de volle hercontroles door: de verse
/// vondsten. Klein genoeg om elke vijf minuten te mogen, groot genoeg om te zien hoe snel een
/// koopje verdwijnt.
pub const FRESH_RECHECKS_PER_ROUND: usize = 5;

/// Een verzoek dat langer dan dit onbeantwoord staat hoort in Discord te komen: het wekbericht
/// kan verloren zijn, en een wachtrij waar niemand doorheen gaat is verder onzichtbaar.
///
/// Een uur paste bij rondes van een uur. Nu er elke vijf minuten gedraaid wordt, is een
/// kwartier ruim genoeg om Hermes de kans te geven en snel genoeg om er iets aan te hebben.
const REVIEW_NAG_AFTER_SECONDS: i64 = 900;

/// En daarna hoogstens eens per uur opnieuw, hoe vaak er ook gedraaid wordt.
const REVIEW_NAG_REPEAT_SECONDS: i64 = 3600;

pub struct RoundOutcome {
    /// What goes to Discord. Everything else stays in the database.
    pub pushes: Vec<PushCandidate>,
    pub problems: Vec<String>,
    /// Why listings were dropped, counted per reason. Only filled when verbose is on, since
    /// it is a debugging aid rather than something the user reads every hour.
    pub rejections: Vec<(String, usize)>,
    pub listings_seen: usize,
    pub findings: usize,
    pub no_longer_finds: usize,
    pub rechecked: usize,
    pub newly_gone: usize,
    pub requests_made: u32,
    pub every_source_failed: bool,
    pub reviews_waiting: usize,
}

/// Zo lang geldt een ronde als bezig. Daarna is hij vastgelopen of het proces is omgevallen,
/// en mag de volgende gewoon starten.
const ROUND_LOCK_SECONDS: i64 = 900;

pub fn run_round(
    settings: &Settings,
    database: &Database,
    now: i64,
    dry_run: bool,
    explain_rejections: bool,
) -> Result<RoundOutcome, String> {
    // Bij rondes van vijf minuten kan een trage ronde de volgende inhalen. Twee rondes naast
    // elkaar leveren niets extra's op en verdubbelen wel het aantal verzoeken aan Vinted en
    // Marktplaats — precies de blokkade die het verzoekbudget moet voorkomen.
    if !dry_run && !database.take_round_lock(now, ROUND_LOCK_SECONDS)? {
        return Err("Er loopt al een ronde. Deze slaat over.".to_string());
    }

    let terms = database.enabled_terms()?;
    if terms.is_empty() {
        return Err(
            "Geen zoektermen aanstaan. Zet er minstens één aan in de app, anders zoekt deze \
             ronde niets af."
                .to_string(),
        );
    }

    // De grens gaat over zoekverzoeken; hercontroles en beschrijvingen komen daar bovenop
    // en staan in `check`. De grens hoort in het formulier te vallen, niet hier; dit is het
    // vangnet eronder.
    let searches = terms.len() * settings.sources.len();
    if searches > MAX_REQUESTS_PER_ROUND {
        return Err(format!(
            "{searches} zoekverzoeken per ronde is te veel (grens {MAX_REQUESTS_PER_ROUND}): \
             {} zoektermen maal {} bronnen. Zet zoektermen uit in de app.",
            terms.len(),
            settings.sources.len()
        ));
    }

    let mut client = HttpClient::new(settings.delay_between_requests_ms);
    let mut problems: Vec<String> = Vec::new();
    let sieve = Sieve::new(&settings.filters);
    let table = PriceTable::new(settings);

    let mut findings: Vec<Finding> = Vec::new();
    let mut found_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut judged_to_nothing: Vec<(String, f64)> = Vec::new();
    let mut seen_this_round: BTreeSet<String> = BTreeSet::new();
    let mut listings_seen = 0usize;
    let mut sources_tried = 0usize;
    let mut sources_failed = 0usize;
    let mut rejection_counts: BTreeMap<String, usize> = BTreeMap::new();

    for source_name in &settings.sources {
        // Een bron die ons kort geleden tegenhield krijgt rust. Elke ronde opnieuw dertien
        // zoekopdrachten tegen een dichte deur gooien is precies hoe je een tijdelijke rem in
        // een lange blokkade verandert.
        let blocked_until = database.source_blocked_until(source_name);
        if now < blocked_until {
            problems.push(format!(
                "{source_name} hield ons tegen; met rust gelaten tot over {} minuten",
                (blocked_until - now) / 60 + 1
            ));
            continue;
        }

        // Zelfregulerend tempo: elke keer dat deze bron ons tegenhield komt er een halve
        // tussenruimte bij. Bij een geslaagde ronde valt dat vanzelf terug naar normaal.
        let strikes = database.source_strikes(source_name);
        let delay = settings.delay_between_requests_ms * (2 + strikes as u64) / 2;
        client.set_delay(delay);
        if strikes > 0 {
            problems.push(format!(
                "{source_name} krijgt {delay} ms tussen verzoeken na {strikes} blokkade(s)"
            ));
        }

        // Built once per source, not once per term: rebuilding the Vinted adapter would
        // fetch the front page again for every search and half again the request count.
        let mut source: Box<dyn Source> = match source_name.as_str() {
            "vinted" => Box::new(Vinted::new(&mut client, &settings.vinted_domain)),
            "marktplaats" => Box::new(Marktplaats::new(&mut client, &settings.filters.postcode)),
            other => {
                problems.push(format!("Onbekende bron \"{other}\" overgeslagen"));
                continue;
            }
        };

        sources_tried += 1;
        let mut source_failed = false;

        let mut blocked = false;
        for term in &terms {
            let listings = match source.search(term, settings.results_per_search) {
                Ok(listings) => listings,
                // Tegengehouden: stoppen met deze bron, niet doorgaan met de andere twaalf
                // zoektermen. Doorrammen is wat een korte rem in een lange blokkade verandert.
                Err(crate::http::Failure::Blocked(reason)) => {
                    problems.push(format!("{source_name} houdt ons tegen: {reason}"));
                    blocked = true;
                    source_failed = true;
                    break;
                }
                Err(error) => {
                    problems.push(format!("{source_name} · \"{term}\": {error}"));
                    source_failed = true;
                    continue;
                }
            };

            for listing in listings {
                listings_seen += 1;
                let key = listing.key();
                found_by.entry(key.clone()).or_default().insert(term.clone());

                // Elke ronde opnieuw beoordelen is het punt: daar komen last_seen, de
                // prijsgeschiedenis en still_a_find vandaan. Alleen niet tweemaal binnen
                // dezelfde ronde, want dezelfde advertentie komt op meerdere termen terug.
                if !seen_this_round.insert(key.clone()) {
                    continue;
                }

                if let Err(rejection) = sieve.check(&listing) {
                    if explain_rejections {
                        *rejection_counts.entry(rejection.describe()).or_insert(0) += 1;
                    }
                    // Geweerd telt hetzelfde als te duur: dit is niets om te melden. Zonder
                    // deze regel blijft een advertentie die gereserveerd raakt als levende
                    // vondst in de inbox staan, want de zeef stopt hem vóór de beoordeling
                    // en de hercontrole slaat hem over omdat hij deze ronde gezien is.
                    judged_to_nothing.push((key, listing.price_euros));
                    continue;
                }

                match table.judge(&listing) {
                    Some(finding) => findings.push(finding),
                    None => judged_to_nothing.push((key, listing.price_euros)),
                }
            }
        }

        drop(source);

        if blocked && !dry_run {
            let wait = database.note_source_blocked(source_name, now)?;
            problems.push(format!(
                "{source_name} wordt de komende {} minuten overgeslagen",
                wait / 60
            ));
        } else if !source_failed && !dry_run {
            database.note_source_healthy(source_name)?;
        }

        if source_failed {
            sources_failed += 1;
        }
    }

    // Eerst kijken wat we al hebben. Vinted stuurt de beschrijving niet mee in het
    // zoekresultaat, dus zonder deze stap haalt elke ronde dezelfde detailpagina opnieuw op.
    for finding in findings.iter_mut() {
        if finding.listing.description.is_empty() {
            if let Some(stored) = database.stored_description(&finding.listing.key()) {
                finding.listing.description = stored;
            }
        }
    }

    // Only findings get a detail lookup, never every listing. In exchange, Vinted findings
    // gain their description, which is the only place a seller says "collection only".
    let lookups = settings.detail_lookups_per_round.min(findings.len());
    let mut looked_up = 0usize;
    for finding in findings.iter_mut() {
        if looked_up >= lookups {
            break;
        }
        if finding.listing.source != "vinted" || !finding.listing.description.is_empty() {
            continue;
        }
        looked_up += 1;
        if let Err(error) = detail::enrich(&mut finding.listing, &mut client) {
            problems.push(format!("beschrijving niet opgehaald: {error}"));
        }
    }
    let still_without = findings
        .iter()
        .filter(|finding| finding.listing.source == "vinted" && finding.listing.description.is_empty())
        .count();
    if still_without > 0 {
        problems.push(format!(
            "{still_without} vondsten kregen geen beschrijving (grens {} per ronde)",
            settings.detail_lookups_per_round
        ));
    }

    // Now that the descriptions are in, the seller's own words decide delivery.
    for finding in findings.iter_mut() {
        apply_pickup_words(finding, settings);
    }

    let every_source_failed = sources_tried > 0 && sources_failed == sources_tried;

    if dry_run {
        return Ok(RoundOutcome {
            pushes: Vec::new(),
            problems,
            rejections: rejection_counts.into_iter().collect(),
            listings_seen,
            findings: findings.len(),
            no_longer_finds: 0,
            rechecked: 0,
            newly_gone: 0,
            requests_made: client.requests_made,
            every_source_failed,
            reviews_waiting: 0,
        });
    }

    let recheck = recheck_followed_listings(settings, database, &mut client, now, &seen_this_round)?;
    problems.extend(recheck.problems);
    if recheck.checked > 0 || recheck.ran {
        database.set_state("last_recheck_at", &now.to_string())?;
    }

    // Eén transactie per ronde, niet één per advertentie: anders staat de database duizend
    // keer kort op slot in plaats van één keer.
    database.begin()?;
    let written = write_round(database, &findings, &found_by, &judged_to_nothing, now);
    match written {
        Ok(no_longer_finds) => {
            database.commit()?;
            let pushes = decide_pushes(database, settings, now)?;
            // Hoogstens één keer per uur. Bij rondes van vijf minuten zou dit anders elke
            // ronde hetzelfde bericht sturen zolang niemand de wachtrij afwerkt — precies de
            // ruis waar dit hele ontwerp vanaf moest.
            let waiting = database.reviews_waiting_longer_than(REVIEW_NAG_AFTER_SECONDS, now)?;
            let last_nag: i64 = database
                .state("last_review_nag_at")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let reviews_waiting = if waiting > 0 && now - last_nag >= REVIEW_NAG_REPEAT_SECONDS {
                database.set_state("last_review_nag_at", &now.to_string())?;
                waiting
            } else {
                0
            };

            record_heartbeat(database, settings, now, &problems);
            let _ = database.release_round_lock();

            Ok(RoundOutcome {
                pushes,
                problems,
                rejections: rejection_counts.into_iter().collect(),
                listings_seen,
                findings: findings.len(),
                no_longer_finds,
                rechecked: recheck.checked,
                newly_gone: recheck.newly_gone,
                requests_made: client.requests_made,
                every_source_failed,
                reviews_waiting,
            })
        }
        Err(error) => {
            database.rollback();
            let _ = database.release_round_lock();
            Err(error)
        }
    }
}

fn apply_pickup_words(finding: &mut Finding, settings: &Settings) {
    if let Some(word) = detail::apply_pickup(&mut finding.listing, &settings.filters.pickup_words) {
        finding.warnings.insert(
            0,
            format!("ALLEEN OPHALEN — de verkoper schrijft \"{word}\""),
        );
        finding.confidence = Confidence::NeedsReview;
        finding.queue_note.get_or_insert_with(|| {
            "De verkoper zegt alleen ophalen. Vraag of verzenden alsnog kan, en waar hij zit."
                .to_string()
        });
    } else if matches!(finding.listing.delivery, Delivery::Unknown)
        && finding.listing.source == "vinted"
    {
        finding.listing.delivery = Delivery::ShippingAvailable;
    }
}

fn write_round(
    database: &Database,
    findings: &[Finding],
    found_by: &BTreeMap<String, BTreeSet<String>>,
    judged_to_nothing: &[(String, f64)],
    now: i64,
) -> Result<usize, String> {
    for finding in findings {
        let key = finding.listing.key();
        let terms: Vec<String> = found_by
            .get(&key)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        database.record_listing(&finding.listing, &terms, now)?;
        database.record_finding(finding, now)?;
    }

    // De prijs ging omhoog of de tabel veranderde. Zonder deze schrijfactie blijft een oude
    // vondst eeuwig in de inbox staan, want een te dure advertentie levert geen vondst op om
    // weg te schrijven.
    let mut no_longer_finds = 0usize;
    for (key, price) in judged_to_nothing {
        if database.has_finding(key) && database.clear_finding(key, *price, now)? {
            no_longer_finds += 1;
        }
    }

    Ok(no_longer_finds)
}

struct RecheckOutcome {
    /// Of de volledige ronde langs de gevolgde advertenties liep, of alleen langs de verse.
    ran: bool,
    checked: usize,
    newly_gone: usize,
    problems: Vec<String>,
}

/// Follows listings through their own page rather than through the search results.
///
/// Both sources return only the sixty newest results per term, so a listing drops out of the
/// window within days while still being for sale. Reading absence as "sold" would mark almost
/// everything gone, hide every price drop, and stop the archive from ever handing back the
/// seller who gives in after two weeks.
fn recheck_followed_listings(
    settings: &Settings,
    database: &Database,
    client: &mut HttpClient,
    now: i64,
    already_seen: &BTreeSet<String>,
) -> Result<RecheckOutcome, String> {
    let table = PriceTable::new(settings);
    let mut checked = 0usize;
    let mut newly_gone = 0usize;
    let mut problems = Vec::new();
    let mut unreadable: Vec<Listing> = Vec::new();

    // Prijzen volgen hoeft niet elke ronde. Zoeken wel — daar zit het koopje — maar een
    // advertentie die je al kent verandert niet elke vijf minuten van prijs. Zonder dit
    // onderscheid zou een ronde van vijf minuten dertig extra verzoeken doen voor niets.
    let last_recheck: i64 = database
        .state("last_recheck_at")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let due = now - last_recheck >= settings.scan.recheck_every_minutes * 60;

    // Verse vondsten gaan wél elke ronde mee: juist daar wil je weten hoe snel hij weg was.
    let fresh_since = now - settings.scan.close_watch_hours * 3600;
    let budget = if due { RECHECKS_PER_ROUND } else { FRESH_RECHECKS_PER_ROUND };

    for stored in database.due_for_recheck(budget, fresh_since)? {
        let key = stored.key();
        // Deze ronde al in de zoekresultaten langsgekomen: dan is hij aantoonbaar nog in de
        // verkoop en hoeft zijn pagina niet ook nog opgehaald te worden.
        if already_seen.contains(&key) {
            database.note_still_there(&key, now)?;
            continue;
        }

        checked += 1;
        match detail::recheck(&stored, client) {
            Ok(PageState::Gone { sold }) => {
                if database.note_gone(&key, sold, now)? {
                    newly_gone += 1;
                }
            }
            Ok(PageState::Present {
                price_euros,
                description,
            }) => {
                database.note_still_there(&key, now)?;
                if let Some(price) = price_euros {
                    rejudge_at_new_price(
                        database, &table, settings, &stored, price, description, now,
                    )?;
                }
            }
            // Een pagina zonder leesbare inhoud: op Vinted het teken dat iets verkocht is,
            // maar ook hoe een opmaakwijziging eruitziet. Daarom pas beslissen als de hele
            // ronde erdoorheen is — dan is te zien of het om één advertentie gaat.
            Ok(PageState::Unreadable) => unreadable.push(stored),

            // Een netwerkfout, een 429 of een kapotte bron telt niet mee: dan blijft
            // last_checked staan en komt de advertentie de volgende ronde weer aan de beurt.
            Err(error) => problems.push(format!("hercontrole mislukt: {error}")),
        }
    }

    // Eén onleesbare pagina tussen leesbare is een verkochte advertentie. Kwam er deze ronde
    // geen enkele pagina leesbaar terug, dan is niet de markt leeggekocht maar waarschijnlijk
    // de opmaak veranderd — en doorpakken zou dan de hele inbox leegvegen, onherstelbaar,
    // want wat verdwenen heet wordt niet meer gecontroleerd.
    //
    // Kwam er wél minstens één pagina leesbaar terug, dan werkt de lezer gewoon en betekent
    // onleesbaar dus echt verkocht. Dat is een scherper onderscheid dan tellen hoeveel er
    // onleesbaar waren: een inbox vol oude vondsten is juist grotendeels verkocht, en die
    // horen dan ook gemarkeerd te worden.
    let readable = checked - unreadable.len();
    if source_markup_looks_broken(readable, unreadable.len()) {
        problems.push(format!(
            "Geen van de {checked} hercontroles leverde een leesbare pagina op. Dat lijkt op \
             een opmaakwijziging bij de bron, niet op verkochte advertenties, dus er is niets \
             als verdwenen gemarkeerd."
        ));
    } else {
        for stored in &unreadable {
            let key = stored.key();
            // Op Vinted verliest een verkochte advertentie zijn schema.org-blok. Voor andere
            // bronnen weten we dat niet, dus daar blijft het onbeslist.
            if stored.source == "vinted" {
                if database.note_gone(&key, true, now)? {
                    newly_gone += 1;
                }
            } else {
                database.note_still_there(&key, now)?;
            }
        }
    }

    Ok(RecheckOutcome {
        ran: due,
        checked,
        newly_gone,
        problems,
    })
}

#[allow(clippy::too_many_arguments)]
/// Of we de onleesbare pagina's van deze ronde mogen vertrouwen.
///
/// Kwam er minstens één pagina leesbaar terug, dan werkt de lezer en betekent onleesbaar dus
/// echt verkocht. Kwam er geen enkele doorheen, dan valt niet te zeggen of de advertenties
/// weg zijn of dat de bron zijn opmaak veranderde — en dan is niets doen het enige veilige,
/// want wat eenmaal verdwenen heet wordt niet meer gecontroleerd.
pub(crate) fn source_markup_looks_broken(readable: usize, unreadable: usize) -> bool {
    unreadable > 0 && readable == 0
}

fn rejudge_at_new_price(
    database: &Database,
    table: &PriceTable,
    settings: &Settings,
    stored: &Listing,
    asking_now: f64,
    description: Option<String>,
    now: i64,
) -> Result<(), String> {
    let mut listing = stored.clone();
    if let Some(text) = description {
        listing.description = text;
    }

    // De pagina noemt de vraagprijs; op Vinted betaal je daarbovenop kopersbescherming. Die
    // opslag is ongeveer evenredig, dus de verhouding van de vorige meting wordt aangehouden
    // in plaats van de kosten weg te laten en zo een daling voor te spiegelen.
    let fee_ratio = if stored.asking_price_euros > 0.0 {
        stored.price_euros / stored.asking_price_euros
    } else {
        1.0
    };
    listing.asking_price_euros = asking_now;
    listing.price_euros = asking_now * fee_ratio;

    database.record_price(&listing.key(), listing.price_euros, asking_now, now)?;

    match table.judge(&listing) {
        Some(mut finding) => {
            // Dezelfde nabewerking als in de zoeklus. Zonder deze regel wist een hercontrole
            // elke dag de ophaalvlag, de waarschuwing en de notitie voor de beoordelaar, want
            // `record_finding` schrijft die velden onvoorwaardelijk over.
            apply_pickup_words(&mut finding, settings);
            database.record_listing(&finding.listing, &[], now)?;
            database.record_finding(&finding, now)?;
        }
        None => {
            database.clear_finding(&listing.key(), listing.price_euros, now)?;
        }
    }
    Ok(())
}

pub(crate) fn decide_pushes(
    database: &Database,
    settings: &Settings,
    now: i64,
) -> Result<Vec<PushCandidate>, String> {
    let candidates = database.findings_to_push(settings.notify.push_below_market_percent)?;

    let (worth_telling, too_good_to_be_true): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|candidate| !below_the_floor(settings, candidate));

    // Ook de gefilterde vondsten krijgen hun stempel. Zonder dat blijven ze elke ronde
    // opnieuw langskomen, en zodra de prijs een keer stijgt zou de eerstvolgende ronde ze
    // alsnog melden.
    for candidate in too_good_to_be_true.iter().chain(worth_telling.iter()) {
        database.mark_pushed(&candidate.key, candidate.price_euros, now)?;
    }
    Ok(worth_telling)
}

/// Onder `suspicious_below` is iets vaker oplichterij dan een buitenkans, en juist die
/// advertenties halen de hoogste kortingspercentages. Zonder deze zeef bestaat het
/// Discord-kanaal — het kanaal dat zeldzaam en betrouwbaar hoort te zijn — vooral uit
/// nepadvertenties: een 5090 voor €105 is 96% onder de markt en dus altijd de luidste
/// melding van de dag. Ze staan gewoon in de app, met de waarschuwing erbij en de
/// Hermes-knop eronder.
fn below_the_floor(settings: &Settings, candidate: &PushCandidate) -> bool {
    settings
        .cards
        .iter()
        .find(|card| card.name == candidate.matched_as)
        .is_some_and(|card| candidate.price_euros < card.suspicious_below)
}

/// The heartbeat. A watcher that died looks exactly like a market with no bargains, so the app
/// needs to be able to tell the difference without anyone remembering to check.
fn record_heartbeat(database: &Database, settings: &Settings, now: i64, problems: &[String]) {
    // Hoe vaak er gedraaid wordt staat in de cron, en die kent de app niet. Door het gat
    // tussen twee rondes door te geven kan de app zelf bepalen wanneer stilte verdacht is —
    // bij rondes van vijf minuten hoort dat veel eerder te zijn dan bij rondes van een uur.
    if let Some(previous) = database
        .state("last_round_at")
        .and_then(|value| value.parse::<i64>().ok())
    {
        let gap = now - previous;
        // Een gat van meer dan zes uur is de nacht of een herstart, geen tempo.
        if (60..6 * 3600).contains(&gap) {
            let _ = database.set_state("round_gap_seconds", &gap.to_string());
        }
    }

    let _ = database.set_state("last_round_at", &now.to_string());
    let json = serde_json::to_string(problems).unwrap_or_else(|_| "[]".to_string());
    let _ = database.set_state("last_round_problems", &json);

    // Hoeveel zoektermen er hoogstens aan mogen staan. De app moet die grens in het
    // formulier afdwingen, en zou hem anders moeten afleiden uit het aantal bronnen — dat
    // staat in TOML, dat de app niet leest.
    let sources = settings.sources.len().max(1);
    let _ = database.set_state(
        "max_search_terms",
        &(MAX_REQUESTS_PER_ROUND / sources).to_string(),
    );
}

/// Looks a listing up for the dossier command.
pub fn find_listing(database: &Database, key: &str) -> Option<Listing> {
    database.listing(key)
}
