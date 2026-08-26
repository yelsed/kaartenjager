//! The one-off move from files to the database.
//!
//! `seen.json` is deliberately not migrated. It cannot be: it holds a key and a timestamp,
//! while `listing` needs a title and a URL, so migrating it would create thousands of ghost
//! rows with empty titles. And it need not be: that file only fed the already-seen gate, and
//! that gate is gone — every round now sieves and judges everything again.

use crate::db::Database;
use crate::listing::{Confidence, Finding};
use std::path::Path;

pub struct Migration {
    pub findings: usize,
    pub needing_review: usize,
    /// Regels die niet overkwamen: onleesbaar, of zonder titel of URL. Stil overslaan zou
    /// betekenen dat je gegevens kwijtraakt zonder het te horen.
    pub skipped: usize,
}

/// Reads the old files into the database. The first `run` does this by itself, and it can be
/// repeated with `kaartenjager migrate --from-files`.
///
/// Everything that comes across is stamped as already pushed, so Discord does not repeat two
/// hundred old acquaintances. On the first migration the visit markers are stamped too, so
/// the inbox starts empty rather than with everything at once.
pub fn from_files(database: &Database, data_dir: &Path, now: i64) -> Result<Migration, String> {
    let recent = read_findings(&data_dir.join("recent.jsonl"));
    let pending = read_findings(&data_dir.join("queue.jsonl"));
    let taken = read_findings(&data_dir.join("queue.taken.jsonl"));

    let mut outcome = Migration {
        findings: 0,
        needing_review: 0,
        skipped: recent.unreadable + pending.unreadable + taken.unreadable,
    };

    database.begin()?;
    match carry_everything(database, &mut outcome, recent, pending, taken, now) {
        Ok(()) => {
            database.commit()?;
            Ok(outcome)
        }
        Err(error) => {
            // Anders blijft de transactie openstaan op een verbinding die de aanroeper
            // gewoon blijft gebruiken, en faalt de eerstvolgende ronde met "cannot start a
            // transaction within a transaction".
            database.rollback();
            Err(error)
        }
    }
}

fn carry_everything(
    database: &Database,
    outcome: &mut Migration,
    recent: Loaded,
    pending: Loaded,
    taken: Loaded,
    now: i64,
) -> Result<(), String> {
    for (loaded, force_review) in [(recent, false), (pending, true), (taken, true)] {
        for mut finding in loaded.findings {
            if finding.listing.title.is_empty() || finding.listing.url.is_empty() {
                outcome.skipped += 1;
                continue;
            }
            if force_review {
                finding.confidence = Confidence::NeedsReview;
                outcome.needing_review += 1;
            }
            carry_over(database, &finding, now)?;
            outcome.findings += 1;
        }
    }

    // Zonder deze twee stempels zou alles wat overkomt als "nieuw sinds je laatste bezoek"
    // gelden, en dat is precies de muur tekst waar de app vanaf moest.
    //
    // Alleen als er ook werkelijk iets overkwam, en alleen de eerste keer. Op een verse
    // installatie zonder oude bestanden zou stempelen juist averechts werken: de overgang
    // draait vlak vóór de eerste ronde, dus alles wat die ronde vindt krijgt hetzelfde
    // tijdstempel en zou meteen als "al gezien" gelden. Dan is je allereerste inbox leeg.
    if outcome.findings > 0 && database.state("last_visit").is_none() {
        database.set_state("last_visit", &now.to_string())?;
        database.set_state("previous_visit", &now.to_string())?;
    }
    database.set_state("migrated_from_files_at", &now.to_string())
}

fn carry_over(database: &Database, finding: &Finding, now: i64) -> Result<(), String> {
    database.record_listing(&finding.listing, &[], now)?;
    database.record_finding(finding, now)?;
    database.mark_pushed(&finding.listing.key(), finding.listing.price_euros, now)
}

/// What one file yielded, and how much of it was unreadable.
struct Loaded {
    findings: Vec<Finding>,
    unreadable: usize,
}

fn read_findings(path: &Path) -> Loaded {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Loaded {
            findings: Vec::new(),
            unreadable: 0,
        };
    };

    let mut findings = Vec::new();
    let mut unreadable = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        // One unreadable line must not cost the rest of the file — but it must be counted,
        // or the migration reports a clean run while dropping data.
        match serde_json::from_str::<Finding>(line) {
            Ok(finding) => findings.push(finding),
            Err(_) => unreadable += 1,
        }
    }
    Loaded {
        findings,
        unreadable,
    }
}
