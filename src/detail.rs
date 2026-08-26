//! Fetching one listing's own page.
//!
//! Two jobs live here. The first is enrichment: Vinted's search results carry no description
//! and nothing about delivery, so a listing that says "remise en main propre" in its own text
//! looks shippable until you open it.
//!
//! The second is the recheck. Both sources return only the sixty newest results per term, so
//! a listing drops out of the search window within days while still being for sale. Absence
//! therefore says nothing, and prices and disappearance have to be read off the listing's own
//! page instead.

use crate::http::{Failure, HttpClient};
use crate::listing::{Delivery, Listing};
use serde_json::Value;

/// Both sources render a schema.org block into their item pages. It is far more stable than
/// the surrounding markup and carries the seller text and the current price.
const SCHEMA_OPEN: &str = r#"<script type="application/ld+json">"#;
const SCHEMA_CLOSE: &str = "</script>";

/// What a recheck concluded about a listing.
pub enum PageState {
    /// Niet meer te koop. Verkocht en verwijderd zijn allebei "weg", maar het verschil zegt
    /// iets: verkocht binnen het uur is een koopje dat iemand anders zag, verwijderd is
    /// vaker een verkoper die zich bedacht.
    Gone { sold: bool },
    Present {
        price_euros: Option<f64>,
        description: Option<String>,
    },
    /// De pagina kwam terug, maar er valt niets uit te lezen. Op Vinted is dat het teken dat
    /// iets verkocht is — een verkochte advertentie verliest zijn schema.org-blok — maar het
    /// is óók hoe een opmaakwijziging eruitziet. De aanroeper beslist, want die ziet of het
    /// om één advertentie gaat of om allemaal tegelijk.
    Unreadable,
}

pub fn enrich(listing: &mut Listing, client: &mut HttpClient) -> Result<(), String> {
    if listing.source != "vinted" || !listing.description.is_empty() {
        return Ok(());
    }

    let html = client.get_text(&listing.url)?;
    let Some(description) = extract_description(&html) else {
        return Err(format!("{}: geen beschrijving in de pagina", listing.url));
    };

    listing.description = description;
    Ok(())
}

/// Reads the listing's own page and reports whether it still exists and what it costs now.
///
/// Only an unambiguous answer counts as gone: HTTP 404 or 410, or an availability marker that
/// says the item is sold. A timeout, a rate limit or a broken connection returns an error, and
/// the caller leaves the listing alone — a Vinted outage must never read as "everything sold".
pub fn recheck(listing: &Listing, client: &mut HttpClient) -> Result<PageState, String> {
    match client.get_page(&listing.url) {
        Ok(html) => Ok(read_page(&html)),
        // De pagina bestaat niet meer. Dat is verwijderd, niet verkocht: een verkochte
        // advertentie blijft op beide sites gewoon staan, met een markering.
        Err(Failure::Gone) => Ok(PageState::Gone { sold: false }),
        Err(other) => Err(format!("{}: {other}", listing.url)),
    }
}

/// The parsing half of a recheck, kept apart from the fetching half so it can be checked
/// without a network.
pub fn read_page(html: &str) -> PageState {
    let Some(product) = product_block(html) else {
        // Gemeten op 26 augustus 2026: een lévende Vinted-advertentie levert dit blok altijd,
        // met `availability: InStock`. Een verkochte levert een pagina van bijna twee megabyte
        // zonder blok. Het woord "Verkocht" staat er wel in, maar alleen in de taalbestanden
        // die op elke pagina meekomen, dus daar valt niet op te toetsen.
        return PageState::Unreadable;
    };

    if sold_out(&product) {
        return PageState::Gone { sold: true };
    }

    PageState::Present {
        price_euros: offer_price(&product),
        description: product
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

pub fn extract_description(html: &str) -> Option<String> {
    product_block(html)?
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn product_block(html: &str) -> Option<Value> {
    let mut cursor = 0;
    while let Some(relative) = html[cursor..].find(SCHEMA_OPEN) {
        let start = cursor + relative + SCHEMA_OPEN.len();
        let end = start + html[start..].find(SCHEMA_CLOSE)?;
        let block = &html[start..end];
        cursor = end;

        let Ok(value) = serde_json::from_str::<Value>(block) else {
            continue;
        };
        if value.get("@type").and_then(Value::as_str) == Some("Product") {
            return Some(value);
        }
    }
    None
}

/// schema.org writes a price as a number on one site and as a string on the next, so both are
/// accepted.
fn offer_price(product: &Value) -> Option<f64> {
    let offers = product.get("offers")?;
    let offer = match offers {
        Value::Array(list) => list.first()?,
        other => other,
    };
    match offer.get("price")? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.replace(',', ".").parse().ok(),
        _ => None,
    }
}

/// A sold listing is as gone as a deleted one: you cannot buy it either way.
fn sold_out(product: &Value) -> bool {
    let Some(offers) = product.get("offers") else {
        return false;
    };
    let offer = match offers {
        Value::Array(list) => match list.first() {
            Some(first) => first,
            None => return false,
        },
        other => other,
    };
    let Some(availability) = offer.get("availability").and_then(Value::as_str) else {
        return false;
    };
    let lowered = availability.to_lowercase();
    lowered.contains("soldout") || lowered.contains("outofstock") || lowered.contains("discontinued")
}

/// Reads collection-only out of the seller's own words. A listing that says so in its text is
/// pickup-only whatever the platform suggests.
pub fn detect_pickup(listing: &Listing, pickup_words: &[String]) -> Option<String> {
    let text = format!("{} {}", listing.title, listing.description).to_lowercase();
    pickup_words
        .iter()
        .find(|word| text.contains(&word.to_lowercase()))
        .cloned()
}

pub fn apply_pickup(listing: &mut Listing, pickup_words: &[String]) -> Option<String> {
    let found = detect_pickup(listing, pickup_words)?;
    listing.delivery = Delivery::PickupOnly;
    Some(found)
}
