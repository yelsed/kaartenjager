//! Configuration: two TOML files merged, with the hand-written one always winning.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    NotFound(Vec<PathBuf>),
    Unreadable { path: PathBuf, cause: String },
    Invalid { path: PathBuf, cause: String },
    Rejected(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound(searched) => {
                let list: Vec<String> =
                    searched.iter().map(|path| path.display().to_string()).collect();
                write!(
                    formatter,
                    "Geen configuratiebestand gevonden. Gezocht in: {}",
                    list.join(", ")
                )
            }
            ConfigError::Unreadable { path, cause } => {
                write!(formatter, "{} kon niet gelezen worden: {cause}", path.display())
            }
            ConfigError::Invalid { path, cause } => {
                write!(formatter, "{} is geen geldige TOML: {cause}", path.display())
            }
            ConfigError::Rejected(reason) => write!(formatter, "{reason}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The machine the user actually owns. Without it, the "does this fit" lines are left out
/// rather than guessed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemProfile {
    pub psu_watts: u32,
    #[serde(default)]
    pub other_draw_watts: u32,
    #[serde(default = "default_bits_per_weight")]
    pub model_bits_per_weight: f64,
    #[serde(default = "default_kv_overhead")]
    pub kv_overhead_gb: f64,
    #[serde(default)]
    pub psu_name: String,
}

fn default_bits_per_weight() -> f64 {
    4.8
}

fn default_kv_overhead() -> f64 {
    3.0
}

impl SystemProfile {
    /// Billions of parameters that fit in this much video memory at the configured quantisation.
    pub fn model_billions_that_fit(&self, vram_gb: f64) -> f64 {
        let usable = (vram_gb - self.kv_overhead_gb).max(0.0);
        usable * 8.0 / self.model_bits_per_weight
    }

    pub fn headroom_watts(&self, card_watts: u32) -> i64 {
        self.psu_watts as i64 - self.other_draw_watts as i64 - card_watts as i64
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CardRule {
    pub name: String,
    pub patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    pub vram_gb: f64,
    #[serde(default)]
    pub bandwidth_gbs: u32,
    #[serde(default)]
    pub tdp_watt: u32,
    pub used_price_low: f64,
    pub used_price_high: f64,
    pub alert_below: f64,
    pub suspicious_below: f64,
    #[serde(default)]
    pub require_memory_in_title: bool,
    /// Where a price came from. Required on rules the weekly review adds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Anything that is not a graphics card: power supplies, risers, adapters. No per-gigabyte
/// arithmetic exists for these, so they carry a fixed note instead.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PartRule {
    pub name: String,
    pub patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub require_all: Vec<String>,
    #[serde(default)]
    pub min_watts: Option<u32>,
    pub alert_below: f64,
    pub suspicious_below: f64,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Filters {
    #[serde(default)]
    pub postcode: String,
    #[serde(default = "default_max_pickup_km")]
    pub max_pickup_km: f64,
    #[serde(default)]
    pub skip_pickup_only: bool,
    #[serde(default = "default_wanted_words")]
    pub wanted_words: Vec<String>,
    /// Marktplaats vertical that means "this really is a graphics card".
    #[serde(default = "default_card_categories")]
    pub card_categories: Vec<String>,
    /// Checked against the title only. A description may mention a water block; a title that
    /// leads with one is selling the block, not the card.
    #[serde(default = "default_accessory_words")]
    pub accessory_words: Vec<String>,
    /// Minimum memory before a card with no rule of its own is worth queueing. Below this a
    /// card is useless for language models however cheap it is.
    #[serde(default = "default_min_unknown_vram")]
    pub min_unknown_vram_gb: u32,
}

fn default_max_pickup_km() -> f64 {
    30.0
}

fn default_wanted_words() -> Vec<String> {
    [
        "gezocht",
        "ruilen",
        "wtb",
        "ter overname gevraagd",
        "zoek ik",
        "wie heeft",
        "gevraagd",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect()
}

fn default_card_categories() -> Vec<String> {
    vec!["graphic_cards".to_string()]
}

fn default_min_unknown_vram() -> u32 {
    12
}

/// Every entry here came out of a live round as a false positive. Vinted is a European
/// marketplace, so the same trick shows up in five languages: a water block, an empty box, a
/// dead card or a scale model, all carrying a real model number in the title.
fn default_accessory_words() -> Vec<String> {
    [
        // Cooling parts sold on their own
        "waterblock",
        "water block",
        "wasserk",
        "koelblok",
        "dissipatore",
        "disipador",
        "heatsink",
        "heat pipe",
        "backplate",
        "back plate",
        "bloques",
        "bloc pour",
        "cooling system",
        "koelsysteem",
        "ventola",
        "shroud",
        // "for a card" in the languages Vinted actually serves
        "voor rtx",
        "voor gtx",
        "voor geforce",
        "für rtx",
        "für gtx",
        "für die",
        "for rtx",
        "for gtx",
        "para rtx",
        "para gtx",
        "pour rtx",
        "pour gtx",
        "per rtx",
        // Broken, incomplete or parts-only
        "for parts",
        "pour pièces",
        "pour pieces",
        "para piezas",
        "per pezzi",
        "senza chip",
        "sin chip",
        "defect",
        "defekt",
        "kapot",
        "niet werkend",
        "not working",
        "read description",
        // Packaging and models rather than hardware
        "lege doos",
        "alleen doos",
        "only box",
        "boite vide",
        "boîte vide",
        "caja vacia",
        "caja vacía",
        "scatola vuota",
        "verpakking",
        "miniatura",
        "modellino",
        "replica",
        "maquette",
        "sticker",
        "poster",
        // Brackets, stands and cables that name the card they fit
        "beugel",
        "houder voor",
        "rallonge",
        "verlengkabel",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect()
}

impl Default for Filters {
    fn default() -> Self {
        Filters {
            postcode: String::new(),
            max_pickup_km: default_max_pickup_km(),
            skip_pickup_only: false,
            wanted_words: default_wanted_words(),
            card_categories: default_card_categories(),
            accessory_words: default_accessory_words(),
            min_unknown_vram_gb: default_min_unknown_vram(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,
    #[serde(default = "default_vinted_domain")]
    pub vinted_domain: String,
    pub card_search_terms: Vec<String>,
    #[serde(default)]
    pub part_search_terms: Vec<String>,
    #[serde(default = "default_results_per_search")]
    pub results_per_search: u32,
    #[serde(default = "default_forget_days")]
    pub forget_seen_after_days: i64,
    #[serde(default = "default_delay")]
    pub delay_between_requests_ms: u64,
    #[serde(default)]
    pub system: Option<SystemProfile>,
    #[serde(default)]
    pub filters: Filters,
    #[serde(default, rename = "card")]
    pub cards: Vec<CardRule>,
    #[serde(default, rename = "part")]
    pub parts: Vec<PartRule>,
    /// Names that came from the hand-written file. The weekly review may not change these,
    /// and has to say so rather than reporting a change that never happened.
    #[serde(skip)]
    pub hand_written_cards: Vec<String>,
}

fn default_sources() -> Vec<String> {
    vec!["vinted".to_string(), "marktplaats".to_string()]
}

fn default_vinted_domain() -> String {
    "www.vinted.nl".to_string()
}

fn default_results_per_search() -> u32 {
    60
}

fn default_forget_days() -> i64 {
    30
}

fn default_delay() -> u64 {
    1500
}

/// Above this the configuration has grown greedy enough to risk a rate-limit block, and a
/// refusal to start is friendlier than a silent lockout.
pub const MAX_REQUESTS_PER_ROUND: usize = 60;

impl Settings {
    pub fn requests_per_round(&self) -> usize {
        (self.card_search_terms.len() + self.part_search_terms.len()) * self.sources.len()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.card_search_terms.is_empty() && self.part_search_terms.is_empty() {
            return Err(ConfigError::Rejected(
                "Geen zoektermen ingesteld: vul card_search_terms of part_search_terms.".into(),
            ));
        }

        if self.cards.is_empty() && self.parts.is_empty() {
            return Err(ConfigError::Rejected(
                "Geen [[card]]- of [[part]]-regels ingesteld, dus er valt niets te beoordelen."
                    .into(),
            ));
        }

        let requests = self.requests_per_round();
        if requests > MAX_REQUESTS_PER_ROUND {
            return Err(ConfigError::Rejected(format!(
                "{requests} verzoeken per ronde is te veel (grens {MAX_REQUESTS_PER_ROUND}). \
                 Schrap zoektermen of bronnen, anders loop je tegen een blokkade aan."
            )));
        }

        for card in &self.cards {
            validate_card(card)?;
        }
        for part in &self.parts {
            if part.suspicious_below >= part.alert_below {
                return Err(ConfigError::Rejected(format!(
                    "Onderdeel \"{}\": suspicious_below ({}) moet onder alert_below ({}) liggen.",
                    part.name, part.suspicious_below, part.alert_below
                )));
            }
        }

        Ok(())
    }

    /// Rules keyed by name, so the weekly review can compare old against proposed.
    pub fn cards_by_name(&self) -> BTreeMap<String, &CardRule> {
        self.cards.iter().map(|card| (card.name.clone(), card)).collect()
    }
}

pub fn validate_card(card: &CardRule) -> Result<(), ConfigError> {
    if card.patterns.is_empty() {
        return Err(ConfigError::Rejected(format!(
            "Kaart \"{}\" heeft geen patterns, dus hij kan nooit ergens op passen.",
            card.name
        )));
    }
    if card.suspicious_below >= card.alert_below {
        return Err(ConfigError::Rejected(format!(
            "Kaart \"{}\": suspicious_below ({}) moet onder alert_below ({}) liggen.",
            card.name, card.suspicious_below, card.alert_below
        )));
    }
    if card.alert_below > card.used_price_low {
        return Err(ConfigError::Rejected(format!(
            "Kaart \"{}\": alert_below ({}) ligt boven used_price_low ({}), \
             dan meldt hij gewone marktprijzen als koopje.",
            card.name, card.alert_below, card.used_price_low
        )));
    }
    if card.used_price_low > card.used_price_high {
        return Err(ConfigError::Rejected(format!(
            "Kaart \"{}\": used_price_low ({}) ligt boven used_price_high ({}).",
            card.name, card.used_price_low, card.used_price_high
        )));
    }
    Ok(())
}

/// Only the pieces the weekly review is allowed to touch.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AutoCards {
    #[serde(default, rename = "card")]
    pub cards: Vec<CardRule>,
}

pub fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(from_environment) = std::env::var("KAARTENJAGER_CONFIG") {
        paths.push(PathBuf::from(from_environment));
    }
    if let Some(home) = home_directory() {
        paths.push(home.join(".config/kaartenjager/kaartenjager.toml"));
    }
    paths.push(PathBuf::from("/etc/kaartenjager/kaartenjager.toml"));
    paths.push(PathBuf::from("kaartenjager.toml"));
    paths
}

pub fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn find_config(explicit: Option<&Path>) -> Result<PathBuf, ConfigError> {
    if let Some(path) = explicit {
        return if path.is_file() {
            Ok(path.to_path_buf())
        } else {
            Err(ConfigError::NotFound(vec![path.to_path_buf()]))
        };
    }

    let candidates = default_config_paths();
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    Err(ConfigError::NotFound(candidates))
}

pub fn parse_settings(text: &str, path: &Path) -> Result<Settings, ConfigError> {
    toml::from_str::<Settings>(text).map_err(|error| ConfigError::Invalid {
        path: path.to_path_buf(),
        cause: error.to_string(),
    })
}

/// Loads the hand-written file, then folds in the reviewed automatic rules. A card the user
/// wrote themselves is never overwritten, whatever the weekly review proposed.
pub fn load(explicit: Option<&Path>) -> Result<(Settings, PathBuf), ConfigError> {
    let path = find_config(explicit)?;
    let text = std::fs::read_to_string(&path).map_err(|error| ConfigError::Unreadable {
        path: path.clone(),
        cause: error.to_string(),
    })?;

    let mut settings = parse_settings(&text, &path)?;
    settings.hand_written_cards = settings.cards.iter().map(|card| card.name.clone()).collect();

    let auto_path = path.with_file_name("cards.auto.toml");
    if auto_path.is_file() {
        let auto_text =
            std::fs::read_to_string(&auto_path).map_err(|error| ConfigError::Unreadable {
                path: auto_path.clone(),
                cause: error.to_string(),
            })?;
        let auto: AutoCards =
            toml::from_str(&auto_text).map_err(|error| ConfigError::Invalid {
                path: auto_path.clone(),
                cause: error.to_string(),
            })?;
        merge_auto_cards(&mut settings, auto.cards);
    }

    settings.validate()?;
    Ok((settings, path))
}

impl Settings {
    pub fn is_hand_written(&self, name: &str) -> bool {
        self.hand_written_cards
            .iter()
            .any(|owned| owned.eq_ignore_ascii_case(name))
    }
}

pub fn merge_auto_cards(settings: &mut Settings, auto_cards: Vec<CardRule>) {
    for candidate in auto_cards {
        let already_hand_written = settings
            .cards
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&candidate.name));
        if !already_hand_written {
            settings.cards.push(candidate);
        }
    }
}
