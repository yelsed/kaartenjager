//! The paste block: everything known about one listing, in a shape another model can read.

use crate::config::{CardRule, Settings};
use crate::listing::{Delivery, Listing};
use crate::money;
use crate::pricing::matches_patterns;

const SELLER_QUESTIONS: [&str; 6] = [
    "Foto van de kaart zelf, met een briefje met de datum erbij",
    "Hoe lang in gebruik, waarvoor, en is er mee gemined",
    "Serienummer, zodat de fabrieksgarantie te controleren is",
    "Doet hij het nu nog, en mag ik een schermafdruk van GPU-Z zien",
    "Zitten de originele stroomkabels of verloopstekkers erbij",
    "Gaat hij in een doos met opvulling, niet in een envelop",
];

pub fn render(listing: &Listing, settings: &Settings) -> String {
    let mut out = String::new();
    out.push_str("```\n");

    out.push_str("ADVERTENTIE\n");
    row(&mut out, "Bron", &source_label(&listing.source));
    row(&mut out, "Titel", &listing.title);
    if listing.has_fees() {
        row(
            &mut out,
            "Vraagprijs",
            &format!(
                "{} (je betaalt {})",
                money::euros_precise(listing.asking_price_euros),
                money::euros_precise(listing.price_euros)
            ),
        );
    } else {
        row(
            &mut out,
            "Vraagprijs",
            &money::euros_precise(listing.price_euros),
        );
    }
    row(
        &mut out,
        "Levering",
        match listing.delivery {
            Delivery::ShippingAvailable => "verzenden mogelijk",
            Delivery::PickupOnly => "alleen ophalen",
            Delivery::Unknown => "onbekend",
        },
    );
    if let Some(distance) = listing.distance_km {
        row(&mut out, "Afstand", &format!("{distance:.0} km"));
    }
    if !listing.location.is_empty() {
        row(&mut out, "Plaats", &listing.location);
    }
    if !listing.seller.is_empty() {
        row(&mut out, "Verkoper", &listing.seller);
    }
    if !listing.condition.is_empty() {
        row(&mut out, "Staat", &listing.condition);
    }
    if !listing.posted.is_empty() {
        row(&mut out, "Geplaatst", &listing.posted);
    }
    row(&mut out, "Foto's", &listing.photo_count.to_string());
    row(&mut out, "URL", &listing.url);

    if listing.description.is_empty() {
        out.push_str("\nBESCHRIJVING\n(niet opgehaald — staat niet in het zoekresultaat)\n");
    } else {
        out.push_str("\nBESCHRIJVING\n");
        out.push_str(listing.description.trim());
        out.push('\n');
    }

    if let Some(card) = find_card(listing, settings) {
        out.push_str("\nKAART\n");
        row(&mut out, "Model", &card.name);
        row(&mut out, "Videogeheugen", &format!("{:.0} GB", card.vram_gb));
        if card.bandwidth_gbs > 0 {
            row(&mut out, "Bandbreedte", &format!("{} GB/s", card.bandwidth_gbs));
        }
        if card.tdp_watt > 0 {
            row(&mut out, "Verbruik", &format!("{} W", card.tdp_watt));
        }

        out.push_str("\nMARKT\n");
        row(
            &mut out,
            "Tweedehands",
            &format!(
                "{} – {}",
                money::euros(card.used_price_low),
                money::euros(card.used_price_high)
            ),
        );
        let under = card.used_price_low - listing.price_euros;
        if under > 0.0 {
            row(
                &mut out,
                "Deze prijs",
                &format!(
                    "{} → {} tot {} onder de markt",
                    money::euros_precise(listing.price_euros),
                    money::euros(under),
                    money::euros(card.used_price_high - listing.price_euros)
                ),
            );
        } else {
            row(
                &mut out,
                "Deze prijs",
                &money::euros_precise(listing.price_euros),
            );
        }
        if card.vram_gb > 0.0 {
            let market_middle = (card.used_price_low + card.used_price_high) / 2.0;
            row(
                &mut out,
                "Per GB",
                &format!(
                    "{} tegenover {} gemiddeld",
                    money::euros_precise(listing.price_euros / card.vram_gb),
                    money::euros_precise(market_middle / card.vram_gb)
                ),
            );
        }

        if let Some(system) = &settings.system {
            out.push_str("\nOP JOUW MACHINE\n");
            let billions = system.model_billions_that_fit(card.vram_gb);
            row(
                &mut out,
                "Model tot",
                &format!("ongeveer {billions:.0}B parameters op Q4"),
            );
            if card.tdp_watt > 0 && system.psu_watts > 0 {
                let headroom = system.headroom_watts(card.tdp_watt);
                row(
                    &mut out,
                    "Voeding",
                    &if headroom >= 0 {
                        format!("{headroom} W over op {} W", system.psu_watts)
                    } else {
                        format!("{} W tekort op {} W", headroom.abs(), system.psu_watts)
                    },
                );
            }
        }
    } else {
        out.push_str(
            "\nKAART\nGeen regel in de prijstabel voor dit model. Zoek zelf uit hoeveel \
             videogeheugen het heeft en wat het tweedehands waard is.\n",
        );
    }

    out.push_str("\nVRAGEN AAN DE VERKOPER\n");
    for question in SELLER_QUESTIONS {
        out.push_str(&format!("· {question}\n"));
    }

    out.push_str("```\n");
    out
}

fn find_card<'settings>(
    listing: &Listing,
    settings: &'settings Settings,
) -> Option<&'settings CardRule> {
    let text = listing.searchable_text();
    settings
        .cards
        .iter()
        .find(|card| matches_patterns(&text, &card.patterns, &card.exclude_patterns))
}

fn source_label(source: &str) -> String {
    match source {
        "vinted" => "Vinted Nederland".to_string(),
        "marktplaats" => "Marktplaats".to_string(),
        other => other.to_string(),
    }
}

fn row(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!("{label:<14}{value}\n"));
}
