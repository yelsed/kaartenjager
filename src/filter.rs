//! Drops listings that should never reach the price table.

use crate::config::Filters;
use crate::listing::{Delivery, Listing, Rejection};

pub struct Sieve<'filters> {
    filters: &'filters Filters,
}

impl<'filters> Sieve<'filters> {
    pub fn new(filters: &'filters Filters) -> Self {
        Sieve { filters }
    }

    pub fn check(&self, listing: &Listing) -> Result<(), Rejection> {
        if listing.reserved {
            return Err(Rejection::Reserved);
        }

        if listing.price_euros <= 0.0 {
            return Err(Rejection::UnusablePrice);
        }

        // Search for a popular model and a share of the results are people who want one.
        let text = listing.searchable_text();
        for word in &self.filters.wanted_words {
            if text.contains(&word.to_lowercase()) {
                return Err(Rejection::WantedAdvertisement(word.clone()));
            }
        }

        if matches!(listing.delivery, Delivery::PickupOnly) {
            // A real bargain is worth knowing about even when collecting it is awkward, so
            // only cheap things get dropped for being far away. Above the threshold the find
            // is reported with a warning instead.
            let cheap_enough_to_ignore =
                listing.price_euros < self.filters.always_report_above_euros;

            if cheap_enough_to_ignore {
                if self.filters.skip_pickup_only {
                    return Err(Rejection::PickupTooFar(listing.distance_km.unwrap_or(0.0)));
                }
                if let Some(distance) = listing.distance_km {
                    if distance > self.filters.max_pickup_km {
                        return Err(Rejection::PickupTooFar(distance));
                    }
                }
            }
        }

        Ok(())
    }
}
