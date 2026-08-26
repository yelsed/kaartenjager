//! Search adapters. Each turns one search term into Listings.
//!
//! Neither endpoint is documented, and both schemas change without notice. Every parser here
//! skips a row it cannot read rather than failing the whole round.

pub mod marktplaats;
pub mod vinted;

use crate::http::Failure;
use crate::listing::Listing;

pub trait Source {
    /// Een `Failure::Blocked` betekent: laat deze bron met rust. De ronde stopt er dan mee in
    /// plaats van dertien keer achter elkaar tegen dezelfde dichte deur te lopen.
    fn search(&mut self, term: &str, limit: u32) -> Result<Vec<Listing>, Failure>;
}

/// Reads a number that the source may send as a JSON number or as a quoted string.
pub fn loose_f64(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    }
}
