//! Fetching one listing's detail page.
//!
//! Vinted's search results carry no description and nothing about delivery, so a listing that
//! says "remise en main propre" in its own text looks shippable until you open it. Only
//! findings get looked up, never every listing, so the request cost stays small.

use crate::http::HttpClient;
use crate::listing::{Delivery, Listing};

/// Vinted renders a schema.org block into every item page. It is far more stable than the
/// surrounding markup and carries the full seller text.
const SCHEMA_OPEN: &str = r#"<script type="application/ld+json">"#;
const SCHEMA_CLOSE: &str = "</script>";

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

pub fn extract_description(html: &str) -> Option<String> {
    let mut cursor = 0;
    while let Some(relative) = html[cursor..].find(SCHEMA_OPEN) {
        let start = cursor + relative + SCHEMA_OPEN.len();
        let end = start + html[start..].find(SCHEMA_CLOSE)?;
        let block = &html[start..end];
        cursor = end;

        let Ok(value) = serde_json::from_str::<serde_json::Value>(block) else {
            continue;
        };
        if value.get("@type").and_then(|kind| kind.as_str()) != Some("Product") {
            continue;
        }
        if let Some(text) = value.get("description").and_then(|text| text.as_str()) {
            return Some(text.to_string());
        }
    }
    None
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
