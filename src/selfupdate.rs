//! The weekly price review. The agent proposes, this module disposes.
//!
//! A hallucinated number has to become a refused proposal with a reason, never a watcher
//! that quietly stops working.

use crate::config::{validate_card, AutoCards, CardRule, ConfigError, Settings};
use crate::money;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Prices do not move by half in a week. A proposal that does is a mistake, not a market.
pub const MAX_WEEKLY_CHANGE: f64 = 0.20;

/// No graphics card rule can sensibly live outside this range.
pub const ABSOLUTE_FLOOR_EUROS: f64 = 20.0;
pub const ABSOLUTE_CEILING_EUROS: f64 = 5_000.0;

const VERSIONS_KEPT: usize = 4;

#[derive(Debug)]
pub struct Change {
    pub card: String,
    pub field: String,
    pub from: f64,
    pub to: f64,
}

impl Change {
    fn percent(&self) -> f64 {
        if self.from == 0.0 {
            return 0.0;
        }
        (self.to - self.from) / self.from * 100.0
    }
}

#[derive(Debug, Default)]
pub struct Review {
    pub applied: Vec<Change>,
    pub added: Vec<String>,
    pub refused: Vec<String>,
    pub unchanged: Vec<String>,
    /// Proposals for cards the user wrote by hand. Reporting these as applied would be a lie:
    /// the hand-written file always wins.
    pub user_owned: Vec<String>,
}

impl Review {
    pub fn render(&self) -> String {
        let mut out = String::from("Prijstabel bijgewerkt\n");

        if self.applied.is_empty()
            && self.added.is_empty()
            && self.refused.is_empty()
            && self.user_owned.is_empty()
        {
            return format!(
                "Prijstabel — geen wijzigingen. {} modellen bekeken, alles binnen de marge.\n",
                self.unchanged.len()
            );
        }

        if !self.applied.is_empty() || !self.added.is_empty() {
            let mut by_card: BTreeMap<&str, Vec<&Change>> = BTreeMap::new();
            for change in &self.applied {
                by_card.entry(change.card.as_str()).or_default().push(change);
            }
            // Counted per card, not per field: four adjustments to one card is one change
            // as far as the reader is concerned.
            out.push_str(&format!(
                "\nTOEGEPAST ({} {})\n",
                by_card.len() + self.added.len(),
                if by_card.len() + self.added.len() == 1 {
                    "model"
                } else {
                    "modellen"
                }
            ));
            for (card, changes) in by_card {
                out.push_str(&format!("\n  {card}\n"));
                for change in changes {
                    out.push_str(&format!(
                        "    {:<18} {} → {}   {:+.0}%\n",
                        change.field,
                        money::euros(change.from),
                        money::euros(change.to),
                        change.percent()
                    ));
                }
            }
            for added in &self.added {
                out.push_str(&format!("\n  {added}   NIEUW\n"));
            }
        }

        if !self.user_owned.is_empty() {
            out.push_str(&format!(
                "\nNIET AANGERAAKT ({})\n  {}\n  staan met de hand in kaartenjager.toml; dat bestand wint\n",
                self.user_owned.len(),
                self.user_owned.join(" · ")
            ));
        }

        if !self.refused.is_empty() {
            out.push_str(&format!("\nGEWEIGERD ({})\n", self.refused.len()));
            for reason in &self.refused {
                out.push_str(&format!("\n  {reason}\n"));
            }
        }

        if !self.unchanged.is_empty() {
            out.push_str(&format!(
                "\nONGEWIJZIGD ({})\n  {}\n",
                self.unchanged.len(),
                self.unchanged.join(" · ")
            ));
        }

        out.push_str("\nTerugdraaien: kaartenjager config rollback\n");
        out
    }
}

/// Checks a proposal against the current table. Nothing is written here; the caller decides.
pub fn review(current: &Settings, proposed: &[CardRule]) -> Review {
    let mut review = Review::default();
    let existing = current.cards_by_name();
    let mut touched: Vec<String> = Vec::new();

    for candidate in proposed {
        if current.is_hand_written(&candidate.name) {
            let differs = existing
                .get(&candidate.name)
                .map(|present| compare(present, candidate).map(|changes| !changes.is_empty()))
                .unwrap_or(Ok(false))
                .unwrap_or(true);
            if differs {
                review.user_owned.push(candidate.name.clone());
            }
            touched.push(candidate.name.clone());
            continue;
        }

        match existing.get(&candidate.name) {
            Some(present) => {
                match compare(present, candidate) {
                    Ok(changes) if changes.is_empty() => {}
                    Ok(changes) => {
                        touched.push(candidate.name.clone());
                        review.applied.extend(changes);
                    }
                    Err(reason) => review.refused.push(reason),
                }
            }
            None => match accept_new(candidate) {
                Ok(()) => {
                    touched.push(candidate.name.clone());
                    review.added.push(candidate.name.clone());
                }
                Err(reason) => review.refused.push(reason),
            },
        }
    }

    // A card the proposal simply left out keeps its current values rather than vanishing.
    // A card whose change was refused is also unchanged, but naming it twice is noise; the
    // refusal already says what happened.
    for name in existing.keys() {
        let was_refused = review
            .refused
            .iter()
            .any(|reason| reason.starts_with(&format!("{name}:")));
        if !touched.contains(name) && !was_refused {
            review.unchanged.push(name.clone());
        }
    }

    review
}

fn compare(present: &CardRule, candidate: &CardRule) -> Result<Vec<Change>, String> {
    validate_card(candidate).map_err(|error| format!("{}: {error}", candidate.name))?;

    let fields: [(&str, f64, f64); 4] = [
        ("used_price_low", present.used_price_low, candidate.used_price_low),
        ("used_price_high", present.used_price_high, candidate.used_price_high),
        ("alert_below", present.alert_below, candidate.alert_below),
        ("suspicious_below", present.suspicious_below, candidate.suspicious_below),
    ];

    let mut changes = Vec::new();
    for (field, from, to) in fields {
        if (from - to).abs() < 0.005 {
            continue;
        }
        if !(ABSOLUTE_FLOOR_EUROS..=ABSOLUTE_CEILING_EUROS).contains(&to) {
            return Err(format!(
                "{}: {field} van {} naar {}\n    buiten de grenzen {} tot {}",
                candidate.name,
                money::euros(from),
                money::euros(to),
                money::euros(ABSOLUTE_FLOOR_EUROS),
                money::euros(ABSOLUTE_CEILING_EUROS)
            ));
        }
        let shift = if from == 0.0 { 1.0 } else { (to - from).abs() / from };
        if shift > MAX_WEEKLY_CHANGE {
            return Err(format!(
                "{}: {field} van {} naar {}\n    stap van {:+.0}%, grens is {:.0}%",
                candidate.name,
                money::euros(from),
                money::euros(to),
                (to - from) / from * 100.0,
                MAX_WEEKLY_CHANGE * 100.0
            ));
        }
        changes.push(Change {
            card: candidate.name.clone(),
            field: field.to_string(),
            from,
            to,
        });
    }

    Ok(changes)
}

fn accept_new(candidate: &CardRule) -> Result<(), String> {
    validate_card(candidate).map_err(|error| format!("{}: {error}", candidate.name))?;

    // Without a citation there is no way to tell a researched price from an invented one.
    let has_source = candidate
        .source
        .as_ref()
        .map(|text| text.trim().len() >= 10)
        .unwrap_or(false);
    if !has_source {
        return Err(format!(
            "{}: nieuw model zonder bruikbare bronvermelding\n    \
             vul source in met waar de prijs vandaan komt",
            candidate.name
        ));
    }

    for value in [
        candidate.used_price_low,
        candidate.used_price_high,
        candidate.alert_below,
        candidate.suspicious_below,
    ] {
        if !(ABSOLUTE_FLOOR_EUROS..=ABSOLUTE_CEILING_EUROS).contains(&value) {
            return Err(format!(
                "{}: bedrag {} valt buiten de grenzen {} tot {}",
                candidate.name,
                money::euros(value),
                money::euros(ABSOLUTE_FLOOR_EUROS),
                money::euros(ABSOLUTE_CEILING_EUROS)
            ));
        }
    }

    Ok(())
}

/// Writes the accepted rules, keeping the previous versions so a bad week can be undone.
pub fn apply(
    config_dir: &Path,
    current: &Settings,
    proposed: &[CardRule],
    review: &Review,
    today: &str,
) -> Result<PathBuf, ConfigError> {
    let target = config_dir.join("cards.auto.toml");
    let history = config_dir.join("config-history");

    if target.is_file() {
        std::fs::create_dir_all(&history).map_err(|error| ConfigError::Unreadable {
            path: history.clone(),
            cause: error.to_string(),
        })?;
        let backup = history.join(format!("cards.auto.{today}.toml"));
        let _ = std::fs::copy(&target, &backup);
        prune_history(&history);
    }

    let accepted: Vec<CardRule> = proposed
        .iter()
        .filter(|candidate| {
            review.added.contains(&candidate.name)
                || review
                    .applied
                    .iter()
                    .any(|change| change.card == candidate.name)
        })
        .cloned()
        .collect();

    // The automatic file accumulates. A week that proposes nothing about a model must leave
    // it standing; writing only this week's accepted rules would silently drop everything
    // earlier weeks had learned.
    let mut to_write: Vec<CardRule> = current
        .cards
        .iter()
        .filter(|card| !current.is_hand_written(&card.name))
        .cloned()
        .collect();

    for card in accepted {
        // Rules the user wrote by hand stay out of the automatic file entirely, so they can
        // never be shadowed by it.
        if current.is_hand_written(&card.name) {
            continue;
        }
        match to_write
            .iter_mut()
            .find(|existing| existing.name.eq_ignore_ascii_case(&card.name))
        {
            Some(existing) => *existing = card,
            None => to_write.push(card),
        }
    }

    to_write.sort_by(|left, right| left.name.cmp(&right.name));

    let document = AutoCards { cards: to_write };
    let text = toml::to_string_pretty(&document).map_err(|error| ConfigError::Rejected(
        format!("Kon het voorstel niet als TOML wegschrijven: {error}"),
    ))?;

    let banner = format!(
        "# Geschreven door de wekelijkse prijsherziening op {today}.\n\
         # Handmatige wijzigingen horen in kaartenjager.toml; die wint altijd.\n\n"
    );

    let temporary = target.with_extension("toml.tmp");
    std::fs::write(&temporary, format!("{banner}{text}")).map_err(|error| {
        ConfigError::Unreadable {
            path: temporary.clone(),
            cause: error.to_string(),
        }
    })?;
    std::fs::rename(&temporary, &target).map_err(|error| ConfigError::Unreadable {
        path: target.clone(),
        cause: error.to_string(),
    })?;

    Ok(target)
}

fn prune_history(history: &Path) {
    let Ok(entries) = std::fs::read_dir(history) else {
        return;
    };
    let mut versions: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("cards.auto."))
                .unwrap_or(false)
        })
        .collect();
    versions.sort();
    while versions.len() > VERSIONS_KEPT {
        let oldest = versions.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
}

pub fn rollback(config_dir: &Path, to_date: Option<&str>) -> Result<PathBuf, String> {
    let history = config_dir.join("config-history");
    if !history.is_dir() {
        return Err(
            "Er is nog geen bewaarde versie. De eerste herziening die iets wijzigt maakt er een."
                .to_string(),
        );
    }
    let entries = std::fs::read_dir(&history)
        .map_err(|error| format!("{} niet leesbaar: {error}", history.display()))?;

    let mut versions: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("cards.auto."))
                .unwrap_or(false)
        })
        .collect();
    versions.sort();

    let chosen = match to_date {
        Some(date) => versions
            .into_iter()
            .find(|path| path.to_string_lossy().contains(date))
            .ok_or_else(|| format!("Geen bewaarde versie van {date} gevonden."))?,
        None => versions
            .pop()
            .ok_or_else(|| "Er is nog geen bewaarde versie om naar terug te gaan.".to_string())?,
    };

    let target = config_dir.join("cards.auto.toml");
    std::fs::copy(&chosen, &target)
        .map_err(|error| format!("Terugzetten mislukt: {error}"))?;
    Ok(chosen)
}

pub fn read_proposal(path: &Path) -> Result<Vec<CardRule>, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|error| ConfigError::Unreadable {
        path: path.to_path_buf(),
        cause: error.to_string(),
    })?;
    let parsed: AutoCards = toml::from_str(&text).map_err(|error| ConfigError::Invalid {
        path: path.to_path_buf(),
        cause: error.to_string(),
    })?;
    Ok(parsed.cards)
}
