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
        (Level::Top, Some(user)) => format!("<@{user}> topvondst\n"),
        (Level::Top, None) => "topvondst\n".to_string(),
        (Level::Deal, _) => "goede deal\n".to_string(),
    };
    out.push_str(&format!(
        "{}: {}\n",
        candidate.matched_as,
        money::euros_precise(candidate.price_euros)
    ));

    let reason = if candidate.is_part {
        let limit = settings
            .parts
            .iter()
            .find(|part| part.name == candidate.matched_as)
            .map(|part| format!("onder {}", money::euros(part.alert_below)));
        match candidate.level {
            // Een onderdeel heeft geen marktbereik; het meldt omdat het onder zijn topvondstprijs zit.
            Level::Top => limit,
            Level::Deal => (!candidate.warnings.is_empty()).then(|| candidate.warnings.join(" · ")),
        }
    } else {
        let market = settings
            .cards
            .iter()
            .find(|card| card.name == candidate.matched_as)
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
            settings
                .cards
                .iter()
                .find(|card| card.name == candidate.matched_as)
                .map(|card| format!("onder je grens van {}", money::euros(card.alert_below)))
        }
    };
    if let Some(reason) = reason {
        out.push_str(&reason);
        out.push('\n');
    }

    let mut parts = vec![candidate.title.clone()];
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

fn source_name(source: &str) -> String {
    match source {
        "vinted" => "Vinted".to_string(),
        "marktplaats" => "Marktplaats".to_string(),
        other => other.to_string(),
    }
}
