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
    /// Shortest and longest variant sold of this model. Listings rarely name the variant, so
    /// the report has to work with the range rather than one number.
    #[serde(default)]
    pub length_mm_min: u32,
    #[serde(default)]
    pub length_mm_max: u32,
    #[serde(default)]
    pub slots_max: u32,
    /// Where a price came from. Required on rules the weekly review adds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The product this rule belongs to. Filled in by `resolve_products` when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
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
    /// Voor dingen die je hoe dan ook zelf ophaalt. Het afstandsfilter in [filters] grijpt
    /// alleen aan bij advertenties die "alleen ophalen" zeggen, en een beeldbuistelevisie
    /// van dertig kilo staat vaak op "ophalen of verzenden" omdat de verkoper dat uit
    /// gewoonte aanvinkt. Staat dit gezet, dan telt de afstand voor deze regel altijd.
    #[serde(default)]
    pub max_pickup_km: Option<f64>,
    pub alert_below: f64,
    pub suspicious_below: f64,
    #[serde(default)]
    pub note: String,
    /// Vinted geeft nooit een afstand. Staat dit aan, dan valt een advertentie zonder afstand
    /// niet weg maar verschijnt hij als goede deal, met de waarschuwing dat de afstand onbekend is.
    #[serde(default)]
    pub show_without_distance: bool,
    /// The product this rule belongs to. Filled in by `resolve_products` when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
}

/// Een computer waarvan de processor de waarde bepaalt, niet het model (`docs/decisions/0007`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComputerRule {
    pub name: String,
    pub product: String,
    /// De doelmodellen, gezocht in de titel.
    pub patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Harde eis: zoveel geheugen moet in de advertentie zelf staan.
    #[serde(default = "default_min_memory_gb")]
    pub min_memory_gb: u32,
    /// CPU Mark per processor, met de naam zoals hij in de app staat ("i5-9500T").
    pub processors: BTreeMap<String, f64>,
    /// Vanaf hoeveel CPU Mark per euro het een topvondst is.
    pub top_score_per_euro: f64,
    /// Daaronder is het een beugel, adapter of kap, geen computer.
    #[serde(default)]
    pub min_price: f64,
    /// Daaronder is het geen vondst, en zonder bekende processor ook niet. Leeg: elke computer
    /// met genoeg geheugen telt.
    #[serde(default)]
    pub min_score_per_euro: Option<f64>,
}

/// Een machine die je al hebt, zodat een vondst zegt hoeveel sneller hij is ("1,4× zo snel als
/// ZenBook (i7-8565U)").
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReferenceMachine {
    pub name: String,
    pub cpu_mark: f64,
}

fn default_min_memory_gb() -> u32 {
    16
}

/// Iets waar je naar zoekt: videokaarten, beeldbuistelevisies, mini-pc's. Een product heeft zijn
/// eigen zoektermen en regels, en in de app zijn eigen overzicht.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Product {
    pub id: String,
    pub name: String,
    /// Waar hij mee begint. Komt één keer in de database; daarna beheert de app de zoektermen.
    #[serde(default)]
    pub search_terms: Vec<String>,
    /// Of topvondsten van dit product een melding opleveren, een die de telefoon laat trillen.
    #[serde(default = "default_notify")]
    pub notify: bool,
    /// Of goede deals van dit product stil in het kanaal komen.
    #[serde(default)]
    pub notify_deals: bool,
    /// Hoe ver onder de markt een kaart van dit product een topvondst is. Zonder waarde geldt
    /// die uit `[notify]`.
    #[serde(default)]
    pub push_below_market_percent: Option<f64>,
}

fn default_notify() -> bool {
    true
}

/// The product cards belong to when the configuration does not name one.
pub const CARD_PRODUCT_ID: &str = "videokaarten";
const CARD_PRODUCT_NAME: &str = "Videokaarten";

/// Turns a rule name into a product id: "Voeding 1000 W of meer" becomes
/// "voeding-1000-w-of-meer".
pub fn product_id_from_name(name: &str) -> String {
    let mut id = String::new();
    for character in name.to_lowercase().chars() {
        if character.is_alphanumeric() {
            id.push(character);
        } else if !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_matches('-').to_string()
}

/// The case the card has to physically go into. Without it the fit lines are left out.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CaseProfile {
    #[serde(default)]
    pub name: String,
    /// What fits today, with the machine as it stands.
    pub max_gpu_length_mm: u32,
    /// What would fit after work you have not done yet. Kept separate so a card is never
    /// reported as fitting on the strength of a change that has not happened.
    #[serde(default)]
    pub max_gpu_length_mm_after_work: u32,
    #[serde(default)]
    pub work_needed: String,
    #[serde(default)]
    pub free_slots: u32,
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
    /// Phrases meaning "collection only". Vinted reports nothing about delivery, so the only
    /// place this shows up is the seller's own description, in whatever language they wrote.
    #[serde(default = "default_pickup_words")]
    pub pickup_words: Vec<String>,
    /// Below this a pickup-only listing far away is genuinely useless and gets dropped. Above
    /// it the find is reported anyway, with a warning: a real bargain is worth knowing about
    /// even when collecting it is awkward.
    #[serde(default = "default_always_report_above")]
    pub always_report_above_euros: f64,
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

fn default_always_report_above() -> f64 {
    150.0
}

/// Collection-only, in the five languages Vinted actually serves. The French phrasing is the
/// one that caught a card out: "remise en main propre" on an otherwise shippable-looking ad.
fn default_pickup_words() -> Vec<String> {
    [
        "alleen ophalen",
        "enkel ophalen",
        "ophalen only",
        "af te halen",
        "niet verzenden",
        "geen verzending",
        "wordt niet verzonden",
        "remise en main propre",
        "en main propre",
        "retrait uniquement",
        "pas d\u{2019}envoi",
        "pas d'envoi",
        "aucun envoi",
        "nur abholung",
        "kein versand",
        "abholung only",
        "solo ritiro",
        "ritiro a mano",
        "no spedizione",
        "solo recogida",
        "entrega en mano",
        "no envio",
        "no env\u{ed}o",
        "pickup only",
        "collection only",
        "no shipping",
        "local pickup",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect()
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
        // Laptop parts share their model number with desktop cards but not their specs:
        // an "RTX 4090 Mobile" is a 16 GB chip, not the 24 GB card the rule is written for.
        "mobile",
        "laptop",
        "notebook",
        "max-q",
        "xg mobile",
        "egpu",
        // Brackets, stands and cables that name the card they fit
        "beugel",
        "houder voor",
        "rallonge",
        "verlengkabel",
        // Uit een ronde op 24 november 2026: koelers, dozen, adapters en kabels in andere talen.
        // Met ^ telt het woord alleen aan het begin: "RTX 2060, con scatola originale" is een kaart.
        "cooler",
        "kühler",
        "kuhler",
        "enfriador",
        "ventoinha",
        "raffreddamento",
        "^watercooling",
        "eiswolf",
        "monoblock",
        "^scatola",
        "^caja",
        "^caixa",
        "boîtes vides",
        "boites vides",
        "châssis",
        "adattatore",
        "adaptador",
        "adaptateur",
        "12vhpwr",
        "cablemod",
        "orologio",
        "ohne gpu",
        "sans gpu",
        "senza gpu",
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
            pickup_words: default_pickup_words(),
            always_report_above_euros: default_always_report_above(),
        }
    }
}

/// Hoe de ronde zich verdeelt over zoeken en volgen.
///
/// Zoeken hoort vaak te gebeuren — een echt koopje op Vinted is soms binnen een half uur weg,
/// dus elk uur kijken betekent dat je het misloopt. Hercontroleren hoort dat juist níét: dat
/// volgt prijzen van advertenties die je al kent, en dagelijks is daarvoor genoeg. Zonder dit
/// onderscheid zou elke ronde van vijf minuten dertig extra verzoeken doen voor niets.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Scan {
    #[serde(default = "default_recheck_minutes")]
    pub recheck_every_minutes: i64,
    /// Een verse vondst wordt wél elke ronde nagekeken, want juist daar wil je weten hoe snel
    /// hij weg is. Na deze tijd valt hij terug in het gewone tempo.
    #[serde(default = "default_close_watch_hours")]
    pub close_watch_hours: i64,
}

fn default_recheck_minutes() -> i64 {
    30
}

fn default_close_watch_hours() -> i64 {
    6
}

impl Default for Scan {
    fn default() -> Self {
        Scan {
            recheck_every_minutes: default_recheck_minutes(),
            close_watch_hours: default_close_watch_hours(),
        }
    }
}

/// Wanneer een vondst het waard is om je voor te storen. Alles daaronder staat in de app en
/// blijft stil.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Notify {
    /// Verder dan dit onder de onderkant van het marktbereik. Alleen kaarten hebben zo'n
    /// bereik, dus onderdelen halen deze drempel per definitie nooit.
    #[serde(default = "default_push_percent")]
    pub push_below_market_percent: f64,
    /// Waar vondsten heen gaan. Zonder webhook drukt de ronde de topvondsten af en post Hermes
    /// ze; `KAARTENJAGER_DISCORD_WEBHOOK` gaat voor op deze waarde.
    #[serde(default)]
    pub discord_webhook: Option<String>,
    /// Wie een topvondst noemt, zodat de telefoon trilt.
    #[serde(default)]
    pub mention_user_id: Option<String>,
}

impl Notify {
    pub fn webhook(&self) -> Option<String> {
        let given = |url: String| {
            let url = url.trim().to_string();
            (!url.is_empty()).then_some(url)
        };
        std::env::var("KAARTENJAGER_DISCORD_WEBHOOK")
            .ok()
            .and_then(given)
            .or_else(|| self.discord_webhook.clone().and_then(given))
    }
}

fn default_push_percent() -> f64 {
    35.0
}

impl Default for Notify {
    fn default() -> Self {
        Notify {
            push_below_market_percent: default_push_percent(),
            discord_webhook: None,
            mention_user_id: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,
    #[serde(default = "default_vinted_domain")]
    pub vinted_domain: String,
    #[serde(default)]
    pub card_search_terms: Vec<String>,
    #[serde(default)]
    pub part_search_terms: Vec<String>,
    #[serde(default = "default_results_per_search")]
    pub results_per_search: u32,
    #[serde(default = "default_delay")]
    pub delay_between_requests_ms: u64,
    #[serde(default)]
    pub system: Option<SystemProfile>,
    #[serde(default, rename = "case")]
    pub computer_case: Option<CaseProfile>,
    /// Cap on detail-page lookups per round. Only findings are looked up, never every listing,
    /// so a normal round costs a handful of extra requests and a cold start a few dozen.
    #[serde(default = "default_detail_lookups")]
    pub detail_lookups_per_round: usize,
    #[serde(default)]
    pub scan: Scan,
    #[serde(default)]
    pub notify: Notify,
    #[serde(default)]
    pub filters: Filters,
    #[serde(default)]
    pub browser: BrowserSettings,
    #[serde(default, rename = "card")]
    pub cards: Vec<CardRule>,
    #[serde(default, rename = "part")]
    pub parts: Vec<PartRule>,
    #[serde(default, rename = "computer")]
    pub computers: Vec<ComputerRule>,
    /// De eigen machine waarmee elke gevonden computer vergeleken wordt.
    #[serde(default)]
    pub reference_machine: Option<ReferenceMachine>,
    #[serde(default, rename = "product")]
    pub products: Vec<Product>,
    /// The products were made from `card_search_terms` and `part_search_terms`, not written
    /// as `[[product]]`. Then none of them is new to a database that already has terms.
    #[serde(skip)]
    pub products_from_legacy_lists: bool,
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

fn default_delay() -> u64 {
    1500
}

fn default_detail_lookups() -> usize {
    12
}

/// Hoe Vinted wordt bezocht. Sinds versie 2.0 gaat dat via een echte Chromium, omdat de
/// rechtstreekse API 403 geeft. Elke sleutel heeft een standaard, zodat een configuratiebestand
/// van 1.8 of 1.9 ongewijzigd blijft laden.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrowserSettings {
    /// Uit betekent: Vinted levert niets en Marktplaats draait gewoon door.
    #[serde(default = "default_browser_enabled")]
    pub enabled: bool,
    /// Leeg laten is het gewone geval; dan wordt Chromium in PATH gezocht.
    #[serde(default)]
    pub executable: String,
    /// Het raster telt zesennegentig advertenties per pagina, dus bij de standaardlimiet van
    /// zestig komt er nooit een tweede aan te pas. Dit is er voor wie die limiet omhoog zet.
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_page_timeout")]
    pub page_timeout_ms: u64,
    #[serde(default = "default_between_pages")]
    pub delay_between_pages_ms: u64,
    /// De noodrem op de klok. Dertien zoektermen die allemaal op hun eigen timeout wachten zouden
    /// samen langer duren dan het rondeslot van een kwartier; dan staat de wachter stil zonder dat
    /// er iets stuk is.
    #[serde(default = "default_round_budget")]
    pub round_budget_seconds: u64,
    /// Alleen nodig als root of in een container. Als gewone gebruiker werkt de zandbak gewoon, en
    /// die uitzetten zonder reden is beveiliging weggeven voor niets.
    #[serde(default)]
    pub no_sandbox: bool,
    /// Leeg = naast de database. Het profiel blijft tussen rondes staan: driehonderd keer per dag
    /// als een gloednieuwe bezoeker langskomen vanaf hetzelfde adres valt meer op dan één die
    /// terugkomt. Na een controlepagina wordt hij wel weggegooid.
    #[serde(default)]
    pub profile_dir: String,
}

fn default_browser_enabled() -> bool {
    true
}

fn default_max_pages() -> u32 {
    2
}

fn default_page_timeout() -> u64 {
    20_000
}

fn default_between_pages() -> u64 {
    2_000
}

fn default_round_budget() -> u64 {
    240
}

impl Default for BrowserSettings {
    fn default() -> Self {
        BrowserSettings {
            enabled: default_browser_enabled(),
            executable: String::new(),
            max_pages: default_max_pages(),
            page_timeout_ms: default_page_timeout(),
            delay_between_pages_ms: default_between_pages(),
            round_budget_seconds: default_round_budget(),
            no_sandbox: false,
            profile_dir: String::new(),
        }
    }
}

/// Above this the configuration has grown greedy enough to risk a rate-limit block, and a
/// refusal to start is friendlier than a silent lockout.
pub const MAX_REQUESTS_PER_ROUND: usize = 60;

impl Settings {
    pub fn requests_per_round(&self) -> usize {
        let terms: usize = self.products.iter().map(|product| product.search_terms.len()).sum();
        self.searches_for(terms)
    }

    /// Gives every rule a product. A configuration without `[[product]]` keeps working: the
    /// cards together become "Videokaarten" and every part becomes a product of its own, with
    /// the part search terms that fit its patterns. Running it twice changes nothing.
    pub fn resolve_products(&mut self) {
        if self.products.is_empty() {
            self.products = self.products_from_legacy_lists();
            self.products_from_legacy_lists = true;
        } else {
            // Eén [[product]] toevoegen aan een oude configuratie mag de producten die uit de
            // oude lijsten kwamen niet wegnemen, anders verdwijnen hun regels en zoektermen.
            let cards_without_product = !self.card_search_terms.is_empty()
                || self.cards.iter().any(|card| card.product.is_none());
            let unnamed_part_products: Vec<String> = self
                .parts
                .iter()
                .filter(|part| part.product.is_none())
                .map(|part| product_id_from_name(&part.name))
                .collect();
            for derived in self.products_from_legacy_lists() {
                let still_needed = if derived.id == CARD_PRODUCT_ID {
                    cards_without_product
                } else {
                    unnamed_part_products.contains(&derived.id)
                };
                if still_needed && !self.products.iter().any(|product| product.id == derived.id) {
                    self.products.push(derived);
                }
            }
        }
        let card_product = self.card_product_id();
        for card in &mut self.cards {
            card.product.get_or_insert_with(|| card_product.clone());
        }
        for part in &mut self.parts {
            part.product
                .get_or_insert_with(|| product_id_from_name(&part.name));
        }
    }

    fn products_from_legacy_lists(&self) -> Vec<Product> {
        let mut products = Vec::new();
        if !self.cards.is_empty() || !self.card_search_terms.is_empty() {
            products.push(Product {
                id: CARD_PRODUCT_ID.to_string(),
                name: CARD_PRODUCT_NAME.to_string(),
                search_terms: self.card_search_terms.clone(),
                notify: true,
                notify_deals: false,
                push_below_market_percent: None,
            });
        }
        for part in &self.parts {
            let id = part
                .product
                .clone()
                .unwrap_or_else(|| product_id_from_name(&part.name));
            if products.iter().any(|product: &Product| product.id == id) {
                continue;
            }
            // Onderdelen meldden nooit; zo blijft het tot de configuratie anders zegt.
            products.push(Product {
                id,
                name: part.name.clone(),
                search_terms: Vec::new(),
                notify: false,
                notify_deals: false,
                push_below_market_percent: None,
            });
        }
        for term in &self.part_search_terms {
            let owner = self
                .part_owning_term(term)
                .and_then(|part| part.product.clone().or_else(|| Some(product_id_from_name(&part.name))));
            let index = owner
                .and_then(|id| products.iter().position(|product| product.id == id))
                .unwrap_or(0);
            if let Some(product) = products.get_mut(index) {
                product.search_terms.push(term.clone());
            }
        }
        products
    }

    fn part_owning_term(&self, term: &str) -> Option<&PartRule> {
        let lowered = term.to_lowercase();
        self.parts.iter().find(|part| {
            part.patterns
                .iter()
                .any(|pattern| crate::pricing::pattern_occurs(&lowered, pattern))
        })
    }

    /// The product a search term most likely belongs to, for terms that came from before
    /// products existed or were added without one.
    pub fn product_for_term(&self, term: &str) -> String {
        let lowered = term.to_lowercase();
        if let Some(product) = self
            .products
            .iter()
            .find(|product| product.search_terms.iter().any(|known| known.to_lowercase() == lowered))
        {
            return product.id.clone();
        }
        if let Some(part) = self.part_owning_term(term) {
            if let Some(id) = &part.product {
                return id.clone();
            }
        }
        if let Some(card) = self.cards.iter().find(|card| {
            card.patterns
                .iter()
                .any(|pattern| crate::pricing::pattern_occurs(&lowered, pattern))
        }) {
            if let Some(id) = &card.product {
                return id.clone();
            }
        }
        self.card_product_id()
    }

    /// The product of a rule by its name, as a finding stores it in `matched_as`. Findings
    /// without a rule of their own (an unknown card) belong to the card product.
    pub fn product_for_rule_name(&self, name: &str) -> String {
        self.cards
            .iter()
            .find(|card| card.name == name)
            .and_then(|card| card.product.clone())
            .or_else(|| {
                self.parts
                    .iter()
                    .find(|part| part.name == name)
                    .and_then(|part| part.product.clone())
            })
            .unwrap_or_else(|| self.card_product_id())
    }

    /// Where cards without a rule of their own go: the product of the first card rule.
    pub fn card_product_id(&self) -> String {
        self.cards
            .iter()
            .find_map(|card| card.product.clone())
            .or_else(|| {
                self.products
                    .iter()
                    .find(|product| product.id == CARD_PRODUCT_ID)
                    .or(self.products.first())
                    .map(|product| product.id.clone())
            })
            .unwrap_or_else(|| CARD_PRODUCT_ID.to_string())
    }

    fn product(&self, id: &str) -> Option<&Product> {
        let id = if id.is_empty() { self.card_product_id() } else { id.to_string() };
        self.products.iter().find(|product| product.id == id)
    }

    pub fn product_notifies(&self, id: &str) -> bool {
        self.product(id).is_some_and(|product| product.notify)
    }

    pub fn product_notifies_deals(&self, id: &str) -> bool {
        self.product(id).is_some_and(|product| product.notify_deals)
    }

    pub fn card_rule(&self, name: &str) -> Option<&CardRule> {
        self.cards.iter().find(|card| card.name == name)
    }

    pub fn part_rule(&self, name: &str) -> Option<&PartRule> {
        self.parts.iter().find(|part| part.name == name)
    }

    /// Wat `check` per product zegt over melden.
    pub fn describe_notify(&self, product: &Product) -> String {
        let has_cards = self.cards.iter().any(|card| {
            self.product(card.product.as_deref().unwrap_or(""))
                .is_some_and(|owner| owner.id == product.id)
        });
        let mut parts = Vec::new();
        if product.notify {
            parts.push(if has_cards {
                format!(
                    "meldt topvondsten vanaf {:.0}% onder de markt",
                    self.push_percent_for(&product.id)
                )
            } else {
                "meldt topvondsten".to_string()
            });
        }
        if product.notify_deals {
            let what = if parts.is_empty() { "meldt alles stil" } else { "goede deals stil" };
            parts.push(if self.notify.webhook().is_some() {
                what.to_string()
            } else {
                format!("{what} zodra er een discord_webhook is")
            });
        }
        if parts.is_empty() {
            "meldt niets".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Hoe ver onder de markt een kaart van dit product een topvondst is.
    pub fn push_percent_for(&self, id: &str) -> f64 {
        self.product(id)
            .and_then(|product| product.push_below_market_percent)
            .unwrap_or(self.notify.push_below_market_percent)
    }

    /// Hoeveel paginabezoeken een ronde kost bij dit aantal zoektermen.
    ///
    /// Vinted telt zwaarder sinds 2.0: daar staat een browser die per zoekterm tot `max_pages`
    /// pagina's kan laden. Deze rekensom staat hier één keer omdat `check` hem over de termen uit
    /// TOML doet en de ronde over de termen uit de database — en twee kopieën die uiteenlopen is
    /// precies het soort fout dat niemand ziet.
    pub fn searches_for(&self, terms: usize) -> usize {
        self.sources
            .iter()
            .map(|source| {
                if source == "vinted" && self.browser.enabled {
                    terms * self.browser.max_pages.max(1) as usize
                } else {
                    terms
                }
            })
            .sum()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.products.iter().all(|product| product.search_terms.is_empty()) {
            return Err(ConfigError::Rejected(
                "Geen zoektermen ingesteld: geef een [[product]] search_terms, of vul \
                 card_search_terms of part_search_terms."
                    .into(),
            ));
        }

        if self.cards.is_empty() && self.parts.is_empty() && self.computers.is_empty() {
            return Err(ConfigError::Rejected(
                "Geen [[card]]-, [[part]]- of [[computer]]-regels ingesteld, dus er valt niets te beoordelen."
                    .into(),
            ));
        }

        for product in &self.products {
            if let Some(percent) = product.push_below_market_percent {
                if !(0.0..100.0).contains(&percent) {
                    return Err(ConfigError::Rejected(format!(
                        "Product \"{}\": push_below_market_percent ({percent}) moet tussen 0 en 100 \
                         liggen.",
                        product.name
                    )));
                }
            }
        }
        if let Some(user) = &self.notify.mention_user_id {
            if user.is_empty() || !user.chars().all(|character| character.is_ascii_digit()) {
                return Err(ConfigError::Rejected(format!(
                    "mention_user_id (\"{user}\") hoort je Discord-gebruikers-ID te zijn, alleen \
                     cijfers. Zet in Discord Ontwikkelaarsmodus aan en kies Gebruikers-ID kopiëren."
                )));
            }
        }

        let requests = self.requests_per_round();
        if requests > MAX_REQUESTS_PER_ROUND {
            return Err(ConfigError::Rejected(format!(
                "{requests} verzoeken per ronde is te veel (grens {MAX_REQUESTS_PER_ROUND}). \
                 Schrap zoektermen of bronnen, anders loop je tegen een blokkade aan."
            )));
        }

        // Twee regels met dezelfde naam is altijd een ongeluk: bij onderdelen wint de eerste
        // die past en komt de tweede nooit aan bod, en bij kaarten weet de prijsherziening
        // niet meer welke ze moet bijwerken. Het viel niemand op omdat er niets stukging —
        // een regel die je net toevoegde deed alleen niets.
        for (soort, namen) in [
            ("Kaart", self.cards.iter().map(|card| &card.name).collect::<Vec<_>>()),
            ("Onderdeel", self.parts.iter().map(|part| &part.name).collect::<Vec<_>>()),
            ("Computer", self.computers.iter().map(|computer| &computer.name).collect::<Vec<_>>()),
        ] {
            let mut gezien = std::collections::BTreeSet::new();
            for naam in namen {
                if !gezien.insert(naam) {
                    return Err(ConfigError::Rejected(format!(
                        "{soort} \"{naam}\" staat er twee keer in. Haal er één weg."
                    )));
                }
            }
        }

        let mut product_ids = std::collections::BTreeSet::new();
        for product in &self.products {
            if product.id.trim().is_empty() || product.name.trim().is_empty() {
                return Err(ConfigError::Rejected(
                    "Elk [[product]] heeft een id en een name nodig.".into(),
                ));
            }
            if !product_ids.insert(product.id.as_str()) {
                return Err(ConfigError::Rejected(format!(
                    "Product \"{}\" staat er twee keer in. Haal er één weg.",
                    product.id
                )));
            }
        }
        // Een regel die naar een product wijst dat niet bestaat, verdwijnt stil uit elk
        // overzicht in de app. Dat hoort `check` te zeggen.
        let rules = self
            .cards
            .iter()
            .map(|card| (&card.name, &card.product))
            .chain(self.parts.iter().map(|part| (&part.name, &part.product)));
        let computer_products: Vec<Option<String>> =
            self.computers.iter().map(|computer| Some(computer.product.clone())).collect();
        let computer_rules = self.computers.iter().map(|computer| &computer.name).zip(&computer_products);
        for (name, product) in rules.chain(computer_rules) {
            if let Some(id) = product {
                if !product_ids.contains(id.as_str()) {
                    return Err(ConfigError::Rejected(format!(
                        "Regel \"{name}\" hoort bij product \"{id}\", maar dat product staat \
                         niet in de [[product]]-lijst."
                    )));
                }
            }
        }

        for card in &self.cards {
            validate_card(card)?;
        }
        if let Some(machine) = &self.reference_machine {
            if machine.cpu_mark <= 0.0 || machine.name.trim().is_empty() {
                return Err(ConfigError::Rejected(
                    "[reference_machine] heeft een naam en een cpu_mark boven nul nodig.".into(),
                ));
            }
        }
        for computer in &self.computers {
            if computer.processors.is_empty()
                || computer.top_score_per_euro <= 0.0
                || computer.min_price < 0.0
                || computer.min_memory_gb == 0
                || computer.min_score_per_euro.is_some_and(|minimum| minimum < 0.0)
                || computer.processors.values().any(|score| *score <= 0.0)
            {
                return Err(ConfigError::Rejected(format!(
                    "Computer \"{}\" heeft processors met een CPU Mark boven nul nodig, een \
                     top_score_per_euro boven nul, een min_memory_gb en geen negatieve min_price of \
                     min_score_per_euro.",
                    computer.name
                )));
            }
            // Twee processors met hetzelfde typenummer: dan wint stil steeds dezelfde.
            let mut models = std::collections::BTreeMap::new();
            for name in computer.processors.keys() {
                let model = crate::pricing::processor_words(name).last().cloned().unwrap_or_default();
                if let Some(earlier) = models.insert(model.clone(), name) {
                    return Err(ConfigError::Rejected(format!(
                        "Computer \"{}\": \"{earlier}\" en \"{name}\" hebben hetzelfde typenummer \
                         \"{model}\". Houd er een.",
                        computer.name
                    )));
                }
            }
        }
        for part in &self.parts {
            if part.suspicious_below >= part.alert_below {
                return Err(ConfigError::Rejected(format!(
                    "Onderdeel \"{}\": suspicious_below ({}) moet onder alert_below ({}) liggen.",
                    part.name, part.suspicious_below, part.alert_below
                )));
            }
            // Zonder postcode geeft Marktplaats geen afstand terug, en een regel die op
            // afstand zeeft gooit dan élke advertentie weg zonder dat er iets kapot lijkt.
            if part.max_pickup_km.is_some() && self.filters.postcode.trim().is_empty() {
                return Err(ConfigError::Rejected(format!(
                    "Onderdeel \"{}\" zeeft op afstand, maar er staat geen postcode in \
                     [filters]. Zonder postcode geeft Marktplaats geen afstand terug en zou \
                     deze regel alles weggooien.",
                    part.name
                )));
            }
        }

        Ok(())
    }

    /// Kaartregels die nooit een Discord-bericht kunnen opleveren.
    ///
    /// De meldgrens ligt op een percentage onder de marktprijs; de oplichtingsbodem ligt op
    /// een vast bedrag. Kruisen die elkaar, dan is elke advertentie die de meldgrens haalt
    /// per definitie al als oplichterij weggezet en blijft het kanaal stil. Dat is precies
    /// het soort fout dat je pas na weken opvalt, dus `check` hoort het te zeggen.
    /// Per kaart met de drempel van zijn product.
    pub fn cards_that_can_never_notify(&self) -> Vec<(&str, f64, f64)> {
        self.cards
            .iter()
            .filter_map(|card| {
                let percent = self.push_percent_for(card.product.as_deref().unwrap_or(""));
                let push_below = card.used_price_low * (1.0 - percent / 100.0);
                (card.suspicious_below >= push_below).then_some((
                    card.name.as_str(),
                    push_below,
                    card.suspicious_below,
                ))
            })
            .collect()
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
    let mut settings = toml::from_str::<Settings>(text).map_err(|error| ConfigError::Invalid {
        path: path.to_path_buf(),
        cause: error.to_string(),
    })?;
    settings.resolve_products();
    Ok(settings)
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
        settings.resolve_products();
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
