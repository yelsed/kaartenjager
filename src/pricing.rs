//! Recognises what a listing is, decides whether it is cheap enough to mention, and builds
//! the plain-language reasons that go with it.

use crate::config::{CardRule, CaseProfile, PartRule, Settings, SystemProfile};
use crate::listing::{Confidence, Finding, FindingKind, Listing};
use crate::money;

/// Memory sizes that actually ship on consumer cards. Anything else in a title is a model
/// number, a price, or a bus width, not a capacity.
const PLAUSIBLE_MEMORY_SIZES: [u32; 12] = [4, 6, 8, 10, 11, 12, 16, 20, 24, 32, 48, 96];

/// Below this a graphics card is cheap enough to be worth a look whatever the model.
const UNKNOWN_CARD_INTERESTING_BELOW: f64 = 250.0;

/// Words that make a listing a graphics card even when no rule matches its model.
const CARD_WORDS: [&str; 10] = [
    "rtx", "gtx", "geforce", "radeon", "videokaart", "grafische kaart", "graphics card",
    "quadro", "tesla", "gpu",
];

pub struct PriceTable<'settings> {
    settings: &'settings Settings,
}

impl<'settings> PriceTable<'settings> {
    pub fn new(settings: &'settings Settings) -> Self {
        PriceTable { settings }
    }

    /// Returns a Finding when the listing is worth mentioning, otherwise None.
    ///
    /// Matching reads the title alone. A live round reported a "PNY GeForce RTX 5080" as a
    /// 5090 because its description compared the two; the description is useful for spotting
    /// wanted ads, never for deciding what is being sold.
    pub fn judge(&self, listing: &Listing) -> Option<Finding> {
        let title = listing.title.to_lowercase();

        // Water blocks, empty boxes, dead cards and scale models all carry a model number in
        // their title. None of them is the card.
        let accessory = self
            .settings
            .filters
            .accessory_words
            .iter()
            .find(|word| title.contains(&word.to_lowercase()));

        if accessory.is_none() && self.category_allows_a_card(listing) {
            if let Some(card) = self.match_card(&title) {
                return self.judge_card(listing, card);
            }
        }
        // Parts are matched even when an accessory word is present, because a riser cable is
        // supposed to say "cable".
        if let Some(part) = self.match_part(&title) {
            return self.judge_part(listing, part);
        }
        if accessory.is_some() {
            return None;
        }
        self.judge_unknown_card(listing, &title)
    }

    /// Marktplaats says per listing where it belongs. A power supply sits in another category
    /// by definition, so this check belongs to card matching rather than to the sieve, where
    /// it would have thrown away every part on Marktplaats.
    fn category_allows_a_card(&self, listing: &Listing) -> bool {
        let wanted = &self.settings.filters.card_categories;
        if listing.categories.is_empty() || wanted.is_empty() {
            // Vinted reports no category at all; absence must not mean rejection.
            return true;
        }
        listing
            .categories
            .iter()
            .any(|category| wanted.contains(category))
    }

    fn match_card(&self, title: &str) -> Option<&CardRule> {
        self.settings
            .cards
            .iter()
            .find(|card| matches_patterns(title, &card.patterns, &card.exclude_patterns))
    }

    fn match_part(&self, title: &str) -> Option<&PartRule> {
        self.settings.parts.iter().find(|part| {
            matches_patterns(title, &part.patterns, &part.exclude_patterns)
                && part
                    .require_all
                    .iter()
                    .all(|needed| title.contains(&needed.to_lowercase()))
        })
    }

    fn judge_card(&self, listing: &Listing, card: &CardRule) -> Option<Finding> {
        if listing.price_euros >= card.alert_below {
            return None;
        }

        let mut confidence = Confidence::Clear;
        let mut queue_note = None;

        if card.require_memory_in_title {
            match stated_memory_gb(&listing.title) {
                Some(stated) if (stated as f64 - card.vram_gb).abs() > 0.5 => {
                    // A 3060 8GB is a different, cheaper card that shares its digits with the
                    // 12GB one. Reporting it as a bargain would be wrong, not uncertain.
                    return None;
                }
                Some(_) => {}
                None => {
                    confidence = Confidence::NeedsReview;
                    queue_note = Some(format!(
                        "Geheugengrootte staat niet in de titel. Deze regel geldt voor de {:.0} GB-uitvoering; controleer of dit niet de kleine is.",
                        card.vram_gb
                    ));
                }
            }
        }

        let mut reasons = Vec::new();

        let under_low = card.used_price_low - listing.price_euros;
        let under_high = card.used_price_high - listing.price_euros;
        if under_low > 0.0 {
            reasons.push(format!(
                "{} tot {} onder de markt (normaal {}–{})",
                money::euros(under_low),
                money::euros(under_high),
                money::euros(card.used_price_low),
                money::euros(card.used_price_high),
            ));
        } else {
            reasons.push(format!(
                "onder de drempel van {} (markt {}–{})",
                money::euros(card.alert_below),
                money::euros(card.used_price_low),
                money::euros(card.used_price_high),
            ));
        }

        if card.vram_gb > 0.0 {
            let market_middle = (card.used_price_low + card.used_price_high) / 2.0;
            reasons.push(format!(
                "{} per GB videogeheugen — markt zit op {}",
                money::euros_precise(listing.price_euros / card.vram_gb),
                money::euros_precise(market_middle / card.vram_gb),
            ));
        }

        if let Some(system) = &self.settings.system {
            reasons.extend(system_reasons(system, card));
        }
        if let Some(computer_case) = &self.settings.computer_case {
            reasons.extend(fit_reasons(computer_case, card));
        }

        let mut warnings = Vec::new();
        if listing.price_euros < card.suspicious_below {
            warnings.push(format!(
                "ver onder de bodem van {} — dit is vaker oplichterij dan een buitenkans",
                money::euros(card.suspicious_below)
            ));
            confidence = Confidence::NeedsReview;
            queue_note.get_or_insert_with(|| {
                "Prijs ligt onder de bodem. Controleer foto's, verkoper en beschrijving."
                    .to_string()
            });
        }
        if card.tdp_watt >= 400 {
            warnings.push(format!(
                "{} W: dit vraagt een 12VHPWR-stekker of drie losse 8-pins",
                card.tdp_watt
            ));
        }
        warnings.extend(listing_warnings(listing));

        // De maat waarop §6 de uitschieterdrempel legt: hoe ver onder de onderkant van het
        // marktbereik deze prijs zit.
        let euros_under_market = card.used_price_low - listing.price_euros;
        let percent_under_market = if card.used_price_low > 0.0 {
            Some(euros_under_market / card.used_price_low * 100.0)
        } else {
            None
        };

        Some(Finding {
            listing: listing.clone(),
            matched_as: card.name.clone(),
            kind: FindingKind::Card,
            confidence,
            percent_under_market,
            euros_under_market: Some(euros_under_market),
            reasons,
            warnings,
            queue_note,
        })
    }

    fn judge_part(&self, listing: &Listing, part: &PartRule) -> Option<Finding> {
        if listing.price_euros >= part.alert_below {
            return None;
        }

        let mut confidence = Confidence::Clear;
        let mut queue_note = None;
        let mut reasons = vec![format!(
            "onder de drempel van {}",
            money::euros(part.alert_below)
        )];

        if let Some(minimum) = part.min_watts {
            match stated_watts(&listing.title) {
                Some(found) if found < minimum => return None,
                Some(found) => reasons.push(format!("{found} W volgens de titel")),
                None => {
                    // "Corsair voeding modulair" may well be an RM1000x. Worth a look rather
                    // than a silent drop.
                    confidence = Confidence::NeedsReview;
                    queue_note = Some(format!(
                        "Geen wattage in de titel; deze regel wil minstens {minimum} W."
                    ));
                }
            }
        }

        if !part.note.is_empty() {
            reasons.push(part.note.clone());
        }

        let mut warnings = Vec::new();
        if listing.price_euros < part.suspicious_below {
            warnings.push(format!(
                "ver onder de bodem van {}",
                money::euros(part.suspicious_below)
            ));
            confidence = Confidence::NeedsReview;
        }
        warnings.extend(listing_warnings(listing));

        Some(Finding {
            listing: listing.clone(),
            matched_as: part.name.clone(),
            kind: FindingKind::Part,
            confidence,
            // Onderdelen hebben geen marktbereik, dus er valt niets tegen af te zetten.
            percent_under_market: None,
            euros_under_market: None,
            reasons,
            warnings,
            queue_note,
        })
    }

    /// Catches cheap cards with no rule of their own. The program will not guess what a
    /// 6700 XT is worth; it only notes that this is a card and that it is cheap.
    fn judge_unknown_card(&self, listing: &Listing, title: &str) -> Option<Finding> {
        let looks_like_a_card = self.category_allows_a_card(listing)
            && (CARD_WORDS.iter().any(|word| title.contains(word))
                || listing
                    .categories
                    .iter()
                    .any(|category| self.settings.filters.card_categories.contains(category)));

        if !looks_like_a_card || listing.price_euros >= UNKNOWN_CARD_INTERESTING_BELOW {
            return None;
        }

        // Without a memory floor every old card at its ordinary price becomes a notification;
        // a live round produced 112 of them. A card below the floor is useless for language
        // models however cheap it is.
        let floor = self.settings.filters.min_unknown_vram_gb;
        let memory = stated_memory_gb(&listing.title);
        let stated = match memory {
            Some(size) if size >= floor => size,
            _ => return None,
        };

        Some(Finding {
            listing: listing.clone(),
            matched_as: "Onbekend model".to_string(),
            kind: FindingKind::Unknown,
            confidence: Confidence::NeedsReview,
            percent_under_market: None,
            euros_under_market: None,
            reasons: vec![format!(
                "{stated} GB videogeheugen voor onder {}, maar er staat geen regel voor dit model in de tabel",
                money::euros(UNKNOWN_CARD_INTERESTING_BELOW)
            )],
            warnings: listing_warnings(listing),
            queue_note: Some(
                "Onbekend model. Zoek uit wat dit is, hoeveel videogeheugen het heeft en wat het tweedehands waard is."
                    .to_string(),
            ),
        })
    }
}

fn system_reasons(system: &SystemProfile, card: &CardRule) -> Vec<String> {
    let mut reasons = Vec::new();

    if card.vram_gb > 0.0 {
        let billions = system.model_billions_that_fit(card.vram_gb);
        if billions >= 1.0 {
            let bandwidth = if card.bandwidth_gbs > 0 {
                format!(" en {} GB/s", card.bandwidth_gbs)
            } else {
                String::new()
            };
            reasons.push(format!(
                "{:.0} GB{}: genoeg voor een model van ongeveer {:.0}B op Q4",
                card.vram_gb, bandwidth, billions
            ));
        }
    }

    if card.tdp_watt > 0 && system.psu_watts > 0 {
        let headroom = system.headroom_watts(card.tdp_watt);
        let psu = if system.psu_name.is_empty() {
            format!("{} W-voeding", system.psu_watts)
        } else {
            system.psu_name.clone()
        };
        if headroom >= 0 {
            reasons.push(format!(
                "{} W — je {} trekt dit met {} W over",
                card.tdp_watt, psu, headroom
            ));
        } else {
            reasons.push(format!(
                "{} W — dit past NIET op je {}, je komt {} W tekort",
                card.tdp_watt,
                psu,
                headroom.abs()
            ));
        }
    }

    reasons
}

/// Listings almost never name the variant, and a 4090 runs from 304 mm to 359 mm depending on
/// which one it is. So the report works with the range, and separates what fits today from
/// what would fit after work that has not been done yet.
fn fit_reasons(computer_case: &CaseProfile, card: &CardRule) -> Vec<String> {
    if card.length_mm_max == 0 {
        return Vec::new();
    }

    let now = computer_case.max_gpu_length_mm;
    let later = computer_case.max_gpu_length_mm_after_work.max(now);
    let case_name = if computer_case.name.is_empty() {
        "de kast".to_string()
    } else {
        format!("je {}", computer_case.name)
    };
    let lengths = if card.length_mm_min == card.length_mm_max {
        format!("{} mm", card.length_mm_max)
    } else {
        format!("{}–{} mm", card.length_mm_min, card.length_mm_max)
    };
    let after = if computer_case.work_needed.is_empty() {
        String::new()
    } else {
        format!(" — {}", computer_case.work_needed)
    };

    let mut reasons = Vec::new();

    if card.length_mm_max <= now {
        reasons.push(format!("{lengths} lang: past nu al in {case_name} ({now} mm)"));
    } else if card.length_mm_min > later {
        reasons.push(format!(
            "PAST NIET: de kortste uitvoering is {} mm en {case_name} neemt hooguit {later} mm",
            card.length_mm_min
        ));
    } else if card.length_mm_min > now {
        reasons.push(format!(
            "{lengths} lang: past NIET in {case_name} zoals hij nu staat ({now} mm), wel bij {later} mm{after}"
        ));
    } else if card.length_mm_max <= later {
        reasons.push(format!(
            "{lengths} lang: de korte uitvoeringen passen nu ({now} mm), de lange pas bij {later} mm{after} — VRAAG WELK MODEL het is"
        ));
    } else {
        reasons.push(format!(
            "{lengths} lang tegen hooguit {later} mm{after}: VRAAG WELK MODEL, de langste uitvoeringen passen niet"
        ));
    }

    if card.slots_max >= 3 && computer_case.free_slots > 0 {
        reasons.push(format!(
            "tot {} sleuven dik: bij de dikste uitvoeringen past er geen tweede kaart naast",
            card.slots_max
        ));
    }

    reasons
}

fn listing_warnings(listing: &Listing) -> Vec<String> {
    let mut warnings = Vec::new();
    if listing.photo_count == 1 {
        warnings.push("maar één foto".to_string());
    }
    if listing.photo_count == 0 {
        warnings.push("geen foto's".to_string());
    }
    if matches!(listing.delivery, crate::listing::Delivery::PickupOnly) {
        let where_from = if listing.location.is_empty() {
            String::new()
        } else {
            format!(" in {}", listing.location)
        };
        warnings.push(format!("alleen ophalen{where_from}"));
    }
    warnings
}

/// A rule matches when at least one pattern occurs and no exclude pattern does. Order in the
/// file therefore stops being load-bearing, which removes a whole class of mistakes.
pub fn matches_patterns(text: &str, patterns: &[String], excludes: &[String]) -> bool {
    let hit = patterns.iter().any(|pattern| pattern_occurs(text, pattern));
    if !hit {
        return false;
    }
    !excludes.iter().any(|pattern| pattern_occurs(text, pattern))
}

/// Dutch plural and inflection endings. A pattern may carry these and still count as a whole
/// word: "voedingen" is the same thing as "voeding", and an exclude pattern that stops firing
/// on "kabels" would let a bag of PSU cables through as a power supply.
const WORD_ENDINGS: [&str; 3] = ["en", "s", "e"];

/// Letters get word boundaries, digits do not.
///
/// A book about breastfeeding reached the queue because "borstvoeding" contains "voeding".
/// Model numbers need the opposite: "rtx3090ti" is a common spelling, and a boundary around
/// "3090 ti" would stop matching it. Splitting the rule on whether the pattern holds a digit
/// settles both cases without anything to configure.
///
/// The boundary is strict in front and forgiving behind: what is glued to the front changes
/// the word ("borstvoeding" is not a power supply), while what is glued to the back is
/// usually just a plural.
pub fn pattern_occurs(text: &str, pattern: &str) -> bool {
    let pattern = pattern.to_lowercase();
    if pattern.is_empty() {
        return false;
    }
    if pattern.chars().any(|character| character.is_ascii_digit()) {
        return text.contains(&pattern);
    }

    let mut from = 0;
    while let Some(relative) = text[from..].find(&pattern) {
        let start = from + relative;
        let end = start + pattern.len();
        let free_before = text[..start]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_alphanumeric());
        if free_before && word_ends_at(&text[end..]) {
            return true;
        }
        // Verder zoeken vanaf het volgende teken, niet de volgende byte: een titel kan
        // meerdere keren hetzelfde woord bevatten, en niet elk teken is één byte.
        from = start
            + text[start..]
                .chars()
                .next()
                .map_or(1, |character| character.len_utf8());
    }
    false
}

/// True when the word stops here, possibly after a plural ending.
fn word_ends_at(rest: &str) -> bool {
    let stops = |text: &str| {
        text.chars()
            .next()
            .is_none_or(|character| !character.is_alphanumeric())
    };
    stops(rest)
        || WORD_ENDINGS
            .iter()
            .any(|ending| rest.strip_prefix(ending).is_some_and(stops))
}

/// Reads "24GB" or "24 GB" out of a title, ignoring numbers that cannot be a capacity.
pub fn stated_memory_gb(title: &str) -> Option<u32> {
    let lowered = title.to_lowercase();
    let bytes = lowered.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let Ok(number) = lowered[start..index].parse::<u32>() else {
            continue;
        };

        let mut after = index;
        while after < bytes.len() && bytes[after] == b' ' {
            after += 1;
        }
        let unit_follows = lowered[after..].starts_with("gb") || lowered[after..].starts_with("g ");
        if unit_follows && PLAUSIBLE_MEMORY_SIZES.contains(&number) {
            return Some(number);
        }
    }
    None
}

/// Reads "850W" or "850 watt" out of a title.
pub fn stated_watts(title: &str) -> Option<u32> {
    let lowered = title.to_lowercase();
    let bytes = lowered.as_bytes();
    let mut index = 0;
    let mut best = None;

    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let Ok(number) = lowered[start..index].parse::<u32>() else {
            continue;
        };

        let mut after = index;
        while after < bytes.len() && bytes[after] == b' ' {
            after += 1;
        }
        let rest = &lowered[after..];
        let is_watts = rest.starts_with('w') && !rest.starts_with("wifi");
        // Parsed from 50 W upward so that a stated-but-too-small supply is rejected by
        // min_watts instead of falling through as "no wattage given".
        if is_watts && (50..=2000).contains(&number) {
            best = Some(number);
        }
    }
    best
}
