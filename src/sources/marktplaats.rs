//! Marktplaats. No session, no cookies, and the full description already in the search
//! result — which is why layer two never has to open a Marktplaats page.

use super::Source;
use crate::http::HttpClient;
use crate::listing::{Delivery, Listing};
use serde_json::Value;

/// Only these carry a real asking price. Bidding and "op aanvraag" listings have none.
const USABLE_PRICE_TYPES: [&str; 2] = ["FIXED", "SEE_DESCRIPTION"];

pub struct Marktplaats<'client> {
    client: &'client mut HttpClient,
    postcode: String,
}

impl<'client> Marktplaats<'client> {
    pub fn new(client: &'client mut HttpClient, postcode: &str) -> Self {
        Marktplaats {
            client,
            postcode: postcode.to_string(),
        }
    }
}

impl Source for Marktplaats<'_> {
    fn search(&mut self, term: &str, limit: u32) -> Result<Vec<Listing>, String> {
        let mut url = format!(
            "https://www.marktplaats.nl/lrp/api/search?query={}&limit={}&offset=0",
            crate::http::url_encode(term),
            limit
        );
        // Without a postcode the response carries no distance, and the pickup filter goes blind.
        if !self.postcode.is_empty() {
            url.push_str(&format!(
                "&postcode={}",
                crate::http::url_encode(&self.postcode)
            ));
        }

        let body = self
            .client
            .get_json(&url, Some("https://www.marktplaats.nl/"))?;
        Ok(parse_search(&body))
    }
}

pub fn parse_search(body: &Value) -> Vec<Listing> {
    let Some(listings) = body.get("listings").and_then(Value::as_array) else {
        return Vec::new();
    };
    listings.iter().filter_map(parse_listing).collect()
}

fn parse_listing(item: &Value) -> Option<Listing> {
    let listing_id = item.get("itemId")?.as_str()?.to_string();
    let title = item.get("title")?.as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    let price_info = item.get("priceInfo")?;
    let price_type = price_info
        .get("priceType")
        .and_then(Value::as_str)
        .unwrap_or("FIXED");
    if !USABLE_PRICE_TYPES.contains(&price_type) {
        return None;
    }

    let cents = price_info.get("priceCents").and_then(Value::as_i64)?;
    // A sentinel of 0 means "bieden"; anything past a hundred grand is a data error.
    if cents <= 0 || cents > 10_000_000 {
        return None;
    }
    let price = cents as f64 / 100.0;

    let description = item
        .get("description")
        .or_else(|| item.get("categorySpecificDescription"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let location_block = item.get("location");
    let location = location_block
        .and_then(|block| block.get("cityName"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let distance_km = location_block
        .and_then(|block| block.get("distanceMeters"))
        .and_then(Value::as_f64)
        .filter(|meters| *meters >= 0.0)
        .map(|meters| meters / 1000.0);

    let attributes = collect_attributes(item);
    let delivery = match attributes
        .iter()
        .find(|(key, _)| key == "delivery")
        .map(|(_, value)| value.to_lowercase())
    {
        Some(value) if value.contains("verzenden") => Delivery::ShippingAvailable,
        Some(value) if value.contains("ophalen") => Delivery::PickupOnly,
        _ => Delivery::Unknown,
    };
    let condition = attributes
        .iter()
        .find(|(key, _)| key == "condition")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();

    let categories = item
        .get("verticals")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let vip_url = item
        .get("vipUrl")
        .and_then(Value::as_str)
        .unwrap_or(&format!("/v/{listing_id}"))
        .to_string();

    let photo_count = item
        .get("imageUrls")
        .and_then(Value::as_array)
        .map(Vec::len)
        .or_else(|| item.get("pictures").and_then(Value::as_array).map(Vec::len))
        .unwrap_or(0);

    Some(Listing {
        source: "marktplaats".to_string(),
        listing_id,
        title,
        price_euros: price,
        asking_price_euros: price,
        url: format!("https://www.marktplaats.nl{vip_url}"),
        description,
        location,
        seller: item
            .get("sellerInformation")
            .and_then(|block| block.get("sellerName"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        condition,
        delivery,
        distance_km,
        photo_count,
        posted: item
            .get("date")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        categories,
        reserved: item
            .get("reserved")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Both attribute lists carry the same keys; the extended one is the fuller of the two.
fn collect_attributes(item: &Value) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for field in ["extendedAttributes", "attributes"] {
        let Some(entries) = item.get(field).and_then(Value::as_array) else {
            continue;
        };
        for entry in entries {
            let (Some(key), Some(value)) = (
                entry.get("key").and_then(Value::as_str),
                entry.get("value").and_then(Value::as_str),
            ) else {
                continue;
            };
            if !pairs.iter().any(|(seen, _): &(String, String)| seen == key) {
                pairs.push((key.to_string(), value.to_string()));
            }
        }
    }
    pairs
}
