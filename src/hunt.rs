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

/// A review request older than this without an answer is worth printing: the wake-up message
/// to Hermes may have gone missing, and a queue nobody works through is invisible otherwise.
const REVIEW_NAG_AFTER_SECONDS: i64 = 3600;

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

pub fn run_round(
    settings: &Settings,
    database: &Database,
    now: i64,
    dry_run: bool,
    explain_rejections: bool,
) -> Result<RoundOutcome, String> {
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

        for term in &terms {
            let listings = match source.search(term, settings.results_per_search) {
                Ok(listings) => listings,
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

        if source_failed {
            sources_failed += 1;
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
    if findings.len() > lookups {
        problems.push(format!(
            "{} vondsten kregen geen beschrijving (grens {} per ronde)",
            findings.len() - lookups,
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

    // Eén transactie per ronde, niet één per advertentie: anders staat de database duizend
    // keer kort op slot in plaats van één keer.
    database.begin()?;
    let written = write_round(database, &findings, &found_by, &judged_to_nothing, now);
    match written {
        Ok(no_longer_finds) => {
            database.commit()?;
            let pushes = decide_pushes(database, settings, now)?;
            let reviews_waiting =
                database.reviews_waiting_longer_than(REVIEW_NAG_AFTER_SECONDS, now)?;

            record_heartbeat(database, settings, now, &problems);

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

    for stored in database.due_for_recheck(RECHECKS_PER_ROUND)? {
        let key = stored.key();
        // Deze ronde al in de zoekresultaten langsgekomen: dan is hij aantoonbaar nog in de
        // verkoop en hoeft zijn pagina niet ook nog opgehaald te worden.
        if already_seen.contains(&key) {
            database.note_still_there(&key, now)?;
            continue;
        }

        checked += 1;
        match detail::recheck(&stored, client) {
            Ok(PageState::Gone) => {
                if database.note_gone(&key, now)? {
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
            // Een netwerkfout, een 429 of een kapotte bron telt niet mee: dan blijft
            // last_checked staan en komt de advertentie de volgende ronde weer aan de beurt.
            Err(error) => problems.push(format!("hercontrole mislukt: {error}")),
        }
    }

    Ok(RecheckOutcome {
        checked,
        newly_gone,
        problems,
    })
}

#[allow(clippy::too_many_arguments)]
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
