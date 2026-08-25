//! Vinted. Needs a browser session, which the front page hands out for free.

use super::{loose_f64, Source};
use crate::http::HttpClient;
use crate::listing::{Delivery, Listing};
use serde_json::Value;

pub struct Vinted<'client> {
    client: &'client mut HttpClient,
    domain: String,
    has_session: bool,
}

impl<'client> Vinted<'client> {
    pub fn new(client: &'client mut HttpClient, domain: &str) -> Self {
        Vinted {
            client,
            domain: domain.to_string(),
            has_session: false,
        }
    }

    fn ensure_session(&mut self) -> Result<(), String> {
        if self.has_session {
            return Ok(());
        }
        // The API answers only to a browser session; loading the front page sets the cookies.
        self.client.get_text(&format!("https://{}/", self.domain))?;
        self.has_session = true;
        Ok(())
    }

    fn catalog_url(&self, term: &str, limit: u32) -> String {
        format!(
            "https://{}/api/v2/catalog/items?search_text={}&order=newest_first&per_page={}&page=1",
            self.domain,
            crate::http::url_encode(term),
            limit
        )
    }
}

impl Source for Vinted<'_> {
    fn search(&mut self, term: &str, limit: u32) -> Result<Vec<Listing>, String> {
        self.ensure_session()?;
        let url = self.catalog_url(term, limit);
        let referer = format!("https://{}/catalog", self.domain);

        let body = match self.client.get_json(&url, Some(&referer)) {
            Ok(body) => body,
            Err(first_failure) => {
                // A stale session shows up as 401 or 403; one fresh handshake usually fixes it.
                self.has_session = false;
                self.client.clear_cookies();
                self.ensure_session()
                    .map_err(|error| format!("{first_failure}; opnieuw verbinden gaf: {error}"))?;
                self.client.get_json(&url, Some(&referer))?
            }
        };

        Ok(parse_search(&body, &self.domain))
    }
}

pub fn parse_search(body: &Value, domain: &str) -> Vec<Listing> {
    let Some(items) = body.get("items").and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| parse_item(item, domain))
        .collect()
}

fn parse_item(item: &Value, domain: &str) -> Option<Listing> {
    let listing_id = match item.get("id")? {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        _ => return None,
    };

    let title = item.get("title")?.as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    let asking = read_amount(item, "price")?;
    // Buyer protection is not optional on Vinted, so the total is what the buyer pays and
    // the only number comparable to a Marktplaats price.
    let total = read_amount(item, "total_item_price").unwrap_or(asking);

    let url = item
        .get("url")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            item.get("path")
                .and_then(Value::as_str)
                .map(|path| format!("https://{domain}{path}"))
        })
        .unwrap_or_else(|| format!("https://{domain}/items/{listing_id}"));

    let seller = item
        .get("user")
        .and_then(|user| user.get("login"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let photo_count = item
        .get("photos")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    Some(Listing {
        source: "vinted".to_string(),
        listing_id,
        title,
        price_euros: total,
        asking_price_euros: asking,
        url,
        description: String::new(),
        location: String::new(),
        seller,
        condition: item
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        // Vinted's search results say nothing about delivery, and sellers do offer
        // collection-only. Claiming "ships" here hid a French pickup-only card behind a
        // perfectly shippable-looking entry. The detail page settles it later.
        delivery: Delivery::Unknown,
        distance_km: None,
        photo_count,
        posted: String::new(),
        // Vinted sends no usable category in search results, so the sieve skips that check.
        categories: Vec::new(),
        reserved: !item
            .get("is_visible")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn read_amount(item: &Value, field: &str) -> Option<f64> {
    let value = item.get(field)?;
    // Vinted has shipped this field as a bare string and as an object; accept both.
    match value {
        Value::Object(_) => loose_f64(value.get("amount")?),
        other => loose_f64(other),
    }
}
