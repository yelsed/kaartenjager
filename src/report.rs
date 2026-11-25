//! What a round prints. The cron delivers stdout to Discord verbatim, so this is the message
//! the user actually reads — and everything that is not an outlier stays out of it.
//!
//! Warnings deliberately do not belong here. A round that could not fetch five descriptions
//! would otherwise send a Discord message every hour; those go to stderr and to the app.

use crate::config::Settings;
use crate::db::PushCandidate;
use crate::listing::{Delivery, Level};
use crate::money;

/// Het bericht uit het ontwerp: wat voor vondst, wat en hoeveel, waarom, welke, waar. De rest
/// staat in de app. Een topvondst noemt de eigenaar als `mention` gegeven is.
pub fn render_push(candidate: &PushCandidate, settings: &Settings, mention: Option<&str>) -> String {
    let mut out = match (candidate.level, mention) {
        (Level::Ultra, Some(user)) => format!("<@{user}> ultra topvondst\n"),
        (Level::Ultra, None) => "ultra topvondst\n".to_string(),
        (Level::Top, Some(user)) => format!("<@{user}> topvondst\n"),
        (Level::Top, None) => "topvondst\n".to_string(),
        (Level::Deal, _) => "goede deal\n".to_string(),
    };
    out.push_str(&format!(
        "{}: {}\n",
        candidate.matched_as,
        money::euros_precise(candidate.price_euros)
    ));

    let reason = if let Some(score) = candidate.score_per_euro {
        Some(format!("{score:.0} CPU Mark per euro"))
    } else if candidate.is_computer {
        Some(shortened(
            &[vec!["geen CPU Mark bekend".to_string()], candidate.warnings.clone()].concat().join(" · "),
            MAX_LINE_CHARACTERS,
        ))
    } else if candidate.is_part {
        let limit = settings
            .part_rule(&candidate.matched_as)
            .map(|part| format!("onder {}", money::euros(part.alert_below)));
        match candidate.level {
            // Een onderdeel heeft geen marktbereik; het meldt omdat het onder zijn topvondstprijs zit.
            Level::Ultra | Level::Top => limit,
            Level::Deal => (!candidate.warnings.is_empty())
                .then(|| shortened(&candidate.warnings.join(" · "), MAX_LINE_CHARACTERS)),
        }
    } else {
        let card = settings.card_rule(&candidate.matched_as);
        let market = card
            .map(|card| {
                format!(
                    " ({} tot {})",
                    money::euros(card.used_price_low),
                    money::euros(card.used_price_high)
                )
            })
            .unwrap_or_default();
        if candidate.percent_under_market > 0.0 {
            Some(format!("{:.0}% onder de markt{market}", candidate.percent_under_market))
        } else {
            // Onder je koopgrens, maar niet onder de onderkant van het marktbereik.
            card.map(|card| format!("onder je grens van {}", money::euros(card.alert_below)))
        }
    };
    if let Some(reason) = reason {
        out.push_str(&reason);
        out.push('\n');
    }

    // Discord weigert een bericht boven de tweeduizend tekens; ingekort wordt alleen wat lang
    // kan worden, zodat de link onderaan altijd blijft.
    let mut parts = vec![shortened(&candidate.title, MAX_LINE_CHARACTERS)];
    if matches!(candidate.delivery, Delivery::PickupOnly) {
        parts.push("alleen ophalen".to_string());
    }
    parts.push(source_name(&candidate.source));
    out.push_str(&parts.join(" · "));
    out.push('\n');

    out.push_str(&candidate.url);
    out.push('\n');
    out
}

/// A round with no outliers prints nothing, so the cron delivers no message. Without that the
/// user gets fifteen notifications a day and turns it off within a week.
pub fn render_round(
    pushes: &[PushCandidate],
    reviews_waiting: usize,
    settings: &Settings,
) -> String {
    let mut out = String::new();

    for (index, candidate) in pushes.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&render_push(candidate, settings, None));
    }

    // Het vangnet onder het wekbericht naar Hermes. Blijft de wachtrij staan, dan is dat een
    // stille storing, en die hoort zichtbaar te worden.
    if reviews_waiting > 0 {
        if !out.is_empty() {
            out.push('\n');
        }
        let what = if reviews_waiting == 1 {
            "Eén beoordeling wacht al even".to_string()
        } else {
            format!("{reviews_waiting} beoordelingen wachten al even")
        };
        out.push_str(&format!(
            "{what} op Hermes. Vraag hem de wachtrij af te werken met: kaartenjager reviews take\n"
        ));
    }

    out
}

const MAX_LINE_CHARACTERS: usize = 300;

fn shortened(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut short: String = text.chars().take(limit).collect();
    short.push_str("...");
    short
}

fn source_name(source: &str) -> String {
    match source {
        "vinted" => "Vinted".to_string(),
        "marktplaats" => "Marktplaats".to_string(),
        other => other.to_string(),
    }
}
