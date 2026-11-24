//! Recognises what a listing is, decides whether it is cheap enough to mention, and builds
//! the plain-language reasons that go with it.

use crate::config::{CardRule, CaseProfile, ComputerRule, PartRule, Settings, SystemProfile};
use std::collections::BTreeMap;
use crate::listing::{Confidence, Finding, FindingKind, Level, Listing};
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
            .find(|word| match word.strip_prefix('^') {
                Some(start) => title.starts_with(&start.to_lowercase()),
                None => title.contains(&word.to_lowercase()),
            });

        let computer = self.match_computer(&title);
        // "OptiPlex 3060 Micro" bevat het kaartpatroon "3060". Noemt de titel een pc-model,
        // dan is het alleen een kaart als er ook een kaartwoord als "RTX" in staat.
        let card_allowed =
            computer.is_none() || CARD_WORDS.iter().any(|word| title.contains(word));
        if card_allowed && accessory.is_none() && self.category_allows_a_card(listing) {
            if let Some(card) = self.match_card(&title) {
                return self.judge_card(listing, card);
            }
        }
        // Na de kaarten: "RTX A2000 uit een EliteDesk" is een kaart. Voor de zeef op
        // accessoires: "incl. adapter" hoort bij een mini-pc gewoon in de titel. Valt hij af,
        // dan alleen nog de onderdelen ("M720q voedingsadapter"), geen onbekende videokaart.
        if let Some(computer) = computer {
            return self
                .judge_computer(listing, computer)
                .or_else(|| self.match_part(&title).and_then(|part| self.judge_part(listing, part)));
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

    fn match_computer(&self, title: &str) -> Option<&ComputerRule> {
        self.settings
            .computers
            .iter()
            .find(|computer| matches_patterns(title, &computer.patterns, &computer.exclude_patterns))
    }

    /// Het geheugen is een harde eis, de processor bepaalt de waarde (`docs/decisions/0007`).
    fn judge_computer(&self, listing: &Listing, computer: &ComputerRule) -> Option<Finding> {
        if listing.price_euros < computer.min_price || listing.price_euros <= 0.0 {
            return None;
        }
        let mut warnings = listing_warnings(listing);
        let mut reasons = Vec::new();
        let mut confidence = Confidence::Clear;
        let mut queue_note = None;

        let stated = listing
            .memory_gb
            .or_else(|| stated_ram_gb(&listing.title))
            .or_else(|| stated_ram_gb(&listing.description));
        match stated {
            Some(memory) if memory >= computer.min_memory_gb => {
                reasons.push(format!("{memory} GB geheugen"));
            }
            Some(_) => return None,
            // Vinted zet geheugen en processor vaak alleen in de beschrijving, en die komt pas
            // na het zoeken binnen. Tot dan voorlopig; daarna beslist `judge` opnieuw.
            None if listing.source == "vinted" && listing.description.is_empty() => {
                confidence = Confidence::NeedsReview;
                queue_note = Some("Geheugen nog onbekend: de beschrijving is nog niet gelezen.".to_string());
                warnings.push("geheugen onbekend".to_string());
            }
            None => return None,
        }

        // De titel eerst: een beschrijving noemt soms een andere processor ter vergelijking.
        let processor = find_processor(&listing.title, &computer.processors)
            .or_else(|| find_processor(&listing.description, &computer.processors));
        let (matched_as, level, score_per_euro) = match processor {
            Some((name, score)) => {
                let per_euro = score / listing.price_euros;
                reasons.insert(
                    0,
                    format!("{name}: CPU Mark {}, {per_euro:.0} per euro", crate::money::whole_number(score)),
                );
                let provisional = queue_note.is_some();
                let level = if per_euro >= computer.top_score_per_euro && !provisional {
                    Level::Top
                } else {
                    Level::Deal
                };
                // Zonder bekend geheugen geen score, zodat hij niet bovenaan de lijst staat.
                (format!("{}, {name}", computer.name), level, (!provisional).then_some(per_euro))
            }
            None => {
                confidence = Confidence::NeedsReview;
                let named = named_processor(&listing.title).or_else(|| named_processor(&listing.description));
                let matched_as = match &named {
                    Some(name) => format!("{}, {name} niet in de tabel", computer.name),
                    None => format!("{}, processor onbekend", computer.name),
                };
                queue_note.get_or_insert_with(|| match named {
                    Some(name) => format!("{name} staat niet in de processortabel; zoek de CPU Mark op en vul hem aan."),
                    None => "Geen processor gevonden in de advertentie; zoek uit welke erin zit.".to_string(),
                });
                (matched_as, Level::Deal, None)
            }
        };

        Some(Finding {
            listing: listing.clone(),
            matched_as,
            kind: FindingKind::Computer,
            product: computer.product.clone(),
            level,
            confidence,
            percent_under_market: None,
            euros_under_market: None,
            score_per_euro,
            reasons,
            warnings,
            queue_note,
        })
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
        let product = card.product.clone().unwrap_or_else(|| self.settings.card_product_id());
        let far_enough_under = percent_under_market
            .is_some_and(|percent| percent >= self.settings.push_percent_for(&product));
        let level = if far_enough_under && listing.price_euros >= card.suspicious_below {
            Level::Top
        } else {
            Level::Deal
        };

        Some(Finding {
            listing: listing.clone(),
            matched_as: card.name.clone(),
            kind: FindingKind::Card,
            product,
            level,
            confidence,
            percent_under_market,
            euros_under_market: Some(euros_under_market),
            score_per_euro: None,
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
        let mut level = Level::Top;
        let mut warnings = Vec::new();
        let mut reasons = vec![format!(
            "onder de drempel van {}",
            money::euros(part.alert_below)
        )];

        // Sommige dingen haal je zelf op, wat de verkoper ook aanvinkt. Daarom telt de
        // afstand hier altijd, en niet — zoals in de gewone zeef — alleen wanneer de
        // bezorgkeuze op "alleen ophalen" staat.
        if let Some(limit) = part.max_pickup_km {
            match listing.distance_km {
                Some(distance) if distance > limit => return None,
                Some(distance) => reasons.push(format!("{distance:.0} km hiervandaan")),
                // Vinted geeft nooit een afstand. Wil de regel die toch zien, dan als goede deal
                // en nooit als topvondst: hij kan net zo goed in Groningen staan.
                None if part.show_without_distance && listing.source == "vinted" => {
                    level = Level::Deal;
                    warnings.push(
                        "afstand onbekend: kijk waar hij staat voordat je gaat rijden".to_string(),
                    );
                }
                // Marktplaats geeft alleen een afstand met een postcode in [filters]. Iets wat je
                // zelf moet ophalen en niet te plaatsen is, is geen vondst: dan sta je zo in
                // Groningen voor een televisie.
                None => return None,
            }
        }

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

        if listing.price_euros < part.suspicious_below {
            warnings.push(format!(
                "ver onder de bodem van {}",
                money::euros(part.suspicious_below)
            ));
            confidence = Confidence::NeedsReview;
            level = Level::Deal;
        }
        warnings.extend(listing_warnings(listing));

        Some(Finding {
            listing: listing.clone(),
            matched_as: part.name.clone(),
            kind: FindingKind::Part,
            product: part
                .product
                .clone()
                .unwrap_or_else(|| crate::config::product_id_from_name(&part.name)),
            level,
            confidence,
            // Onderdelen hebben geen marktbereik, dus er valt niets tegen af te zetten.
            percent_under_market: None,
            euros_under_market: None,
            score_per_euro: None,
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
            product: self.settings.card_product_id(),
            level: Level::Deal,
            confidence: Confidence::NeedsReview,
            percent_under_market: None,
            euros_under_market: None,
            score_per_euro: None,
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
    // Nul betekent onbekend, niet "geen foto's". Marktplaats leest het veld met `unwrap_or(0)`,
    // en de Vinted-cataloguspagina toont er één per rij wat over het werkelijke aantal niets zegt
    // -- daar vult `detail::enrich` het aan. Een waarschuwing hangen aan een veld dat ook gewoon
    // kan ontbreken zet "maar één foto" onder élke Vinted-vondst, en die gaat mee naar Discord.
    if listing.photo_count == 1 {
        warnings.push("maar één foto".to_string());
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
        // "3090" hoort niet te passen in het onderdeelnummer "102B4030900".
        let starts_with_digit = pattern.starts_with(|character: char| character.is_ascii_digit());
        let ends_with_digit = pattern.ends_with(|character: char| character.is_ascii_digit());
        return text.match_indices(&pattern).any(|(start, found)| {
            let digit_before = text[..start].chars().next_back().is_some_and(|character| character.is_ascii_digit());
            let digit_after = text[start + found.len()..].chars().next().is_some_and(|character| character.is_ascii_digit());
            !(starts_with_digit && digit_before) && !(ends_with_digit && digit_after)
        });
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

/// Werkgeheugen uit vrije tekst: "16GB", "16 GB DDR4", "16 Go", "16gb ram". Het eerste dat
/// genoemd wordt telt, want "16GB (8GB + 8GB)" en "8GB, uit te breiden tot 32GB" noemen het
/// geheugen eerst. "8/16GB" is een keuze en telt als de laagste, "2 x 8GB" als 16, en in
/// "16/256GB SSD" is de 16 het geheugen. Opslag ("256GB SSD") telt niet.
pub fn stated_ram_gb(text: &str) -> Option<u32> {
    let lowered = text.to_lowercase();
    let bytes = lowered.as_bytes();
    let number_at = |start: usize| -> (u32, usize) {
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        (lowered[start..end].parse().unwrap_or(0), end)
    };
    let skip_spaces = |mut position: usize| {
        while position < bytes.len() && bytes[position] == b' ' {
            position += 1;
        }
        position
    };
    let storage_words = ["ssd", "nvme", "hdd", "m.2", "m2", "opslag", "emmc", "schijf", "disk", "harde"];
    let mut index = 0;
    while index < bytes.len() {
        let starts_a_number =
            bytes[index].is_ascii_digit() && (index == 0 || !bytes[index - 1].is_ascii_alphanumeric());
        if !starts_a_number {
            index += 1;
            continue;
        }
        let (first, mut end) = number_at(index);
        index = end;
        let mut amounts = vec![first];
        let operator = skip_spaces(end);
        let operand = skip_spaces(operator + 1);
        let operand_follows = operand < bytes.len() && bytes[operand].is_ascii_digit();
        if operator < bytes.len() && bytes[operator] == b'x' && operand_follows {
            // "2x8gb", "2 x 8GB": twee repen.
            let (per_module, after) = number_at(operand);
            amounts = vec![first.saturating_mul(per_module)];
            end = after;
        } else {
            // "8/16gb", "8 / 16 GB", "16/256GB".
            let mut slash = operator;
            while slash < bytes.len() && bytes[slash] == b'/' {
                let next = skip_spaces(slash + 1);
                if next >= bytes.len() || !bytes[next].is_ascii_digit() {
                    break;
                }
                let (amount, after) = number_at(next);
                amounts.push(amount);
                end = after;
                slash = skip_spaces(after);
            }
        }
        index = index.max(end);
        let rest = lowered[end..].trim_start();
        let Some(after_unit) = rest.strip_prefix("gb").or_else(|| rest.strip_prefix("go")) else {
            continue;
        };
        // Een woord dat met "go" of "gb" begint is geen eenheid ("4 goede voetjes"), behalve
        // "16gbram" en "16gbddr4"; "1gbit" en "10gbps" gaan over het netwerk.
        let glued_word = after_unit.starts_with(|character: char| character.is_ascii_alphabetic());
        if glued_word && !(after_unit.starts_with("ram") || after_unit.starts_with("ddr")) {
            continue;
        }
        let storage_follows = storage_words.iter().any(|word| after_unit.trim_start().starts_with(word));
        let ram_like: Vec<u32> = amounts.iter().copied().filter(|amount| RAM_SIZES.contains(amount)).collect();
        if storage_follows {
            // "16/256GB SSD": het eerste is het geheugen, het laatste de opslag.
            let last = *amounts.last().unwrap_or(&0);
            if amounts.len() > 1 && RAM_SIZES.contains(&amounts[0]) && !RAM_SIZES.contains(&last) {
                return Some(amounts[0]);
            }
            continue;
        }
        if ram_like.len() == amounts.len() {
            return ram_like.into_iter().min();
        }
    }
    None
}

const RAM_SIZES: [u32; 8] = [4, 8, 12, 16, 24, 32, 48, 64];

/// De processor uit de tekst, met zijn CPU Mark. Herkend op het typenummer, het laatste woord
/// van de naam in de tabel: "i5-9500T" op "9500t", ook in "i5 9500T" en "i59500t". Het langste
/// typenummer eerst, zodat "8500t" wint van "8500".
pub fn find_processor(text: &str, processors: &BTreeMap<String, f64>) -> Option<(String, f64)> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .flat_map(|word| {
            // "i59500t" is "i5" en "9500t" aan elkaar.
            let glued = word.len() >= 6
                && word.starts_with('i')
                && matches!(word.as_bytes()[1], b'3' | b'5' | b'7' | b'9')
                && word.as_bytes()[2].is_ascii_digit();
            if glued {
                vec![word[..2].to_string(), word[2..].to_string()]
            } else {
                vec![word.to_string()]
            }
        })
        .collect();
    let mut candidates: Vec<(&String, f64, Vec<String>)> = processors
        .iter()
        .map(|(name, score)| (name, *score, processor_words(name)))
        .filter(|(_, _, key)| !key.is_empty())
        .collect();
    candidates.sort_by_key(|(_, _, key)| std::cmp::Reverse(key.last().map_or(0, String::len)));
    candidates
        .into_iter()
        .find(|(_, _, key)| {
            let model = &key[key.len() - 1];
            // Een kaal getal ("8500") komt overal voor; dan moet het woord ervoor ook kloppen.
            let needs_prefix = model.chars().all(|character| character.is_ascii_digit()) && key.len() > 1;
            words.iter().enumerate().any(|(position, word)| {
                word == model
                    && (!needs_prefix || (position > 0 && words[position - 1] == key[key.len() - 2]))
            })
        })
        .map(|(name, score, _)| (name.clone(), score))
}

/// Een processor die de tekst noemt, ook als de tabel hem niet kent: "i7 6700T" wordt
/// "i7-6700T", "ryzen 7 pro 5750ge" wordt "Ryzen 7 PRO 5750GE". Alleen een typenummer van vier
/// cijfers of meer telt, zodat "i5 9e generatie" geen processor is.
pub fn named_processor(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect();
    let is_model = |word: &str| word.chars().take_while(char::is_ascii_digit).count() >= 4;
    for (position, word) in words.iter().enumerate() {
        let next = words.get(position + 1).map(String::as_str).unwrap_or_default();
        let is_intel_family = matches!(word.as_str(), "i3" | "i5" | "i7" | "i9");
        if is_intel_family && is_model(next) {
            return Some(format!("{word}-{}", next.to_uppercase()));
        }
        // "i76700t": familie en typenummer aan elkaar.
        if word.len() >= 6 && matches!(&word[..2], "i3" | "i5" | "i7" | "i9") && is_model(&word[2..]) {
            return Some(format!("{}-{}", &word[..2], word[2..].to_uppercase()));
        }
        if word == "ryzen" && next.len() == 1 && next.chars().all(|character| character.is_ascii_digit()) {
            let after = words.get(position + 2).map(String::as_str).unwrap_or_default();
            let (pro, model) = if after == "pro" {
                (" PRO", words.get(position + 3).map(String::as_str).unwrap_or_default())
            } else {
                ("", after)
            };
            if is_model(model) {
                return Some(format!("Ryzen {next}{pro} {}", model.to_uppercase()));
            }
        }
    }
    None
}

/// "i5-9500T" wordt ["i5", "9500t"]. Het laatste woord is het typenummer.
pub fn processor_words(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}
