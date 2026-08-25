//! One round: search every term on every source, sieve, judge, remember, report.

use crate::config::Settings;
use crate::detail;
use crate::filter::Sieve;
use crate::http::HttpClient;
use crate::listing::{Confidence, Delivery, Finding, Listing};
use crate::pricing::PriceTable;
use crate::queue::Queue;
use crate::sources::{marktplaats::Marktplaats, vinted::Vinted, Source};
use crate::state::History;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// Findings kept around so `dossier` can look one up after the fact.
const RECENT_KEPT: usize = 200;

pub struct RoundOutcome {
    pub findings: Vec<Finding>,
    pub problems: Vec<String>,
    /// Why listings were dropped, counted per reason. Only filled when verbose is on, since
    /// it is a debugging aid rather than something the user reads every hour.
    pub rejections: Vec<(String, usize)>,
    pub listings_seen: usize,
    pub listings_new: usize,
    pub requests_made: u32,
    pub every_source_failed: bool,
}

pub fn run_round(
    settings: &Settings,
    data_dir: &Path,
    now: i64,
    dry_run: bool,
    explain_rejections: bool,
) -> Result<RoundOutcome, String> {
    let mut client = HttpClient::new(settings.delay_between_requests_ms);
    let (mut history, warning) = History::load(
        &data_dir.join("seen.json"),
        settings.forget_seen_after_days,
        now,
    );

    let mut problems: Vec<String> = warning.into_iter().collect();
    let sieve = Sieve::new(&settings.filters);
    let table = PriceTable::new(settings);
    let queue = Queue::new(data_dir);

    let mut findings = Vec::new();
    let mut returned_this_round: HashSet<String> = HashSet::new();
    let mut listings_seen = 0usize;
    let mut listings_new = 0usize;
    let mut sources_tried = 0usize;
    let mut sources_failed = 0usize;
    let mut rejection_counts: BTreeMap<String, usize> = BTreeMap::new();

    let all_terms: Vec<&String> = settings
        .card_search_terms
        .iter()
        .chain(settings.part_search_terms.iter())
        .collect();

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

        for term in &all_terms {
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

                if !returned_this_round.insert(key.clone()) {
                    continue;
                }
                if !history.is_new(&key) {
                    continue;
                }
                if let Err(rejection) = sieve.check(&listing) {
                    if explain_rejections {
                        *rejection_counts.entry(rejection.describe()).or_insert(0) += 1;
                    }
                    // Rejected listings still count as seen, so the same noise is not
                    // examined again every hour.
                    if !dry_run {
                        history.remember(&key);
                    }
                    continue;
                }

                listings_new += 1;
                if let Some(finding) = table.judge(&listing) {
                    findings.push(finding);
                }
                if !dry_run {
                    history.remember(&key);
                }
            }
        }

        drop(source);

        if source_failed {
            sources_failed += 1;
        }
    }

    // Only findings get a detail lookup, never every listing. A normal round costs a handful
    // of extra requests; a cold start a few dozen. In exchange, Vinted findings gain their
    // description, which is the only place a seller says "collection only".
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

    if !dry_run {
        for finding in &findings {
            if finding.should_queue() {
                if let Err(error) = queue.push(finding) {
                    problems.push(format!("Kon niet op de stapel schrijven: {error}"));
                }
            }
        }
        if let Err(error) = write_recent(data_dir, &findings) {
            problems.push(format!("Kon recente vondsten niet bewaren: {error}"));
        }
        // Written only after every search finished, so a crash halfway never marks listings
        // as seen that were never reported.
        if let Err(error) = history.save() {
            problems.push(format!("Kon de geschiedenis niet bewaren: {error}"));
        }
    }

    Ok(RoundOutcome {
        findings,
        problems,
        rejections: rejection_counts.into_iter().collect(),
        listings_seen,
        listings_new,
        requests_made: client.requests_made,
        every_source_failed: sources_tried > 0 && sources_failed == sources_tried,
    })
}

fn write_recent(data_dir: &Path, findings: &[Finding]) -> std::io::Result<()> {
    let path = data_dir.join("recent.jsonl");
    let mut all = read_recent(data_dir);
    all.extend(findings.iter().cloned());
    if all.len() > RECENT_KEPT {
        all.drain(0..all.len() - RECENT_KEPT);
    }

    std::fs::create_dir_all(data_dir)?;
    let mut text = String::new();
    for finding in &all {
        text.push_str(&serde_json::to_string(finding)?);
        text.push('\n');
    }
    let temporary = path.with_extension("jsonl.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, &path)
}

pub fn read_recent(data_dir: &Path) -> Vec<Finding> {
    let path = data_dir.join("recent.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Finding>(line).ok())
        .collect()
}

/// Looks a listing up among everything this installation has seen recently.
pub fn find_listing(data_dir: &Path, key: &str) -> Option<Listing> {
    let queue = Queue::new(data_dir);
    let mut candidates = read_recent(data_dir);
    if let Ok(queued) = queue.peek() {
        candidates.extend(queued);
    }
    candidates
        .into_iter()
        .find(|finding| finding.listing.key() == key)
        .map(|finding| finding.listing)
}
