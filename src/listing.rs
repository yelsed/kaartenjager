//! The one shape every source produces, and the verdict the price table attaches to it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Delivery {
    ShippingAvailable,
    PickupOnly,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub source: String,
    pub listing_id: String,
    pub title: String,
    /// What the buyer actually pays. On Vinted that includes buyer protection.
    pub price_euros: f64,
    /// The number shown on the listing, before fees. Equal to `price_euros` when there are none.
    pub asking_price_euros: f64,
    pub url: String,
    pub description: String,
    pub location: String,
    pub seller: String,
    pub condition: String,
    pub delivery: Delivery,
    pub distance_km: Option<f64>,
    pub photo_count: usize,
    pub posted: String,
    /// Wanneer de advertentie geplaatst is, voor zover de bron dat prijsgeeft. Vinted zegt het
    /// niet met zoveel woorden, maar de eerste foto draagt zijn uploadmoment mee, en dat is in
    /// de praktijk het plaatsingsmoment. Zonder dit is "hoe snel ging die deal weg" niet te
    /// beantwoorden: dan weet je alleen wanneer wíj hem zagen.
    #[serde(default)]
    pub posted_at: Option<i64>,
    /// Hoeveel mensen ernaar keken en hoeveel het bewaarden. Alleen Vinted geeft dit, en het
    /// is de enige maat voor belangstelling die we hebben — bij een echte koopje loopt dit
    /// binnen minuten op.
    #[serde(default)]
    pub view_count: Option<i64>,
    #[serde(default)]
    pub favourite_count: Option<i64>,
    /// Source-specific category labels, used to drop listings that are not the thing we want.
    pub categories: Vec<String>,
    pub reserved: bool,
}

impl Listing {
    /// Identity across runs. Sources reuse numeric ids among themselves, so the source
    /// name is part of the key.
    pub fn key(&self) -> String {
        format!("{}:{}", self.source, self.listing_id)
    }

    /// Everything a filter or matcher wants to read, lowercased once instead of per check.
    pub fn searchable_text(&self) -> String {
        format!("{} {}", self.title, self.description).to_lowercase()
    }

    pub fn has_fees(&self) -> bool {
        (self.price_euros - self.asking_price_euros).abs() >= 0.01
    }
}

impl Default for Listing {
    fn default() -> Self {
        Listing {
            source: String::new(),
            listing_id: String::new(),
            title: String::new(),
            price_euros: 0.0,
            asking_price_euros: 0.0,
            url: String::new(),
            description: String::new(),
            location: String::new(),
            seller: String::new(),
            condition: String::new(),
            delivery: Delivery::Unknown,
            distance_km: None,
            photo_count: 0,
            posted: String::new(),
            posted_at: None,
            view_count: None,
            favourite_count: None,
            categories: Vec::new(),
            reserved: false,
        }
    }
}

/// Why a listing was dropped before it reached the price table.
#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    WantedAdvertisement(String),
    Reserved,
    UnusablePrice,
    PickupTooFar(f64),
}

impl Rejection {
    pub fn describe(&self) -> String {
        match self {
            Rejection::WantedAdvertisement(word) => {
                format!("vraagadvertentie (bevat \"{word}\")")
            }
            Rejection::Reserved => "gereserveerd".to_string(),
            Rejection::UnusablePrice => "geen bruikbare prijs".to_string(),
            Rejection::PickupTooFar(km) => format!("alleen ophalen, {km:.0} km"),
        }
    }
}

/// How confident the program is, which decides whether layer two also gets a look.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Confidence {
    /// Known model, threshold met, nothing ambiguous.
    Clear,
    /// Reported, but something needs a human or the agent to check.
    NeedsReview,
}

/// Which kind of rule matched. Only cards have a market range, so only cards can be measured
/// against the push threshold.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum FindingKind {
    Card,
    Part,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub listing: Listing,
    /// Name from the price table, or a generic label when nothing matched.
    pub matched_as: String,
    /// Defaulted so the one-off migration can read findings written before this field existed.
    #[serde(default)]
    pub kind: FindingKind,
    pub confidence: Confidence,
    /// How far below the bottom of the market range this sits, as a percentage. None for
    /// parts, which have no market range to measure against.
    #[serde(default)]
    pub percent_under_market: Option<f64>,
    #[serde(default)]
    pub euros_under_market: Option<f64>,
    /// Plain-language reasons this is worth looking at, already formatted.
    pub reasons: Vec<String>,
    /// Things that should give pause, already formatted.
    pub warnings: Vec<String>,
    pub queue_note: Option<String>,
}

impl Finding {
    pub fn should_queue(&self) -> bool {
        self.confidence == Confidence::NeedsReview
    }
}
