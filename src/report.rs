//! What layer one prints. The cron delivers stdout to Discord verbatim, so this is the
//! message the user actually reads.

use crate::listing::{Confidence, Delivery, Finding};
use crate::money;

pub fn render_findings(findings: &[Finding]) -> String {
    let mut out = String::new();
    for (index, finding) in findings.iter().enumerate() {
        if index > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&render_one(finding));
    }
    out
}

pub fn render_one(finding: &Finding) -> String {
    let listing = &finding.listing;
    let mut out = String::new();

    let flag = match finding.confidence {
        Confidence::Clear => "",
        Confidence::NeedsReview => "  ⟨uitzoeken⟩",
    };
    out.push_str(&format!(
        "{} — {}{}\n",
        finding.matched_as,
        money::euros_precise(listing.price_euros),
        flag
    ));

    if listing.has_fees() {
        out.push_str(&format!(
            "vraagprijs {} + kosten\n",
            money::euros_precise(listing.asking_price_euros)
        ));
    }

    out.push_str(&summary_line(finding));
    out.push('\n');

    if !finding.reasons.is_empty() {
        out.push_str("\nWAAROM INTERESSANT\n");
        for reason in &finding.reasons {
            out.push_str(&format!("· {reason}\n"));
        }
    }

    if !finding.warnings.is_empty() {
        out.push_str("\nLET OP\n");
        for warning in &finding.warnings {
            out.push_str(&format!("· {warning}\n"));
        }
    }

    out.push_str(&format!("\n{}\n", listing.url));
    out
}

fn summary_line(finding: &Finding) -> String {
    let listing = &finding.listing;
    let mut parts = vec![listing.title.clone()];

    if !listing.condition.is_empty() {
        parts.push(listing.condition.clone());
    }
    if !listing.location.is_empty() {
        parts.push(listing.location.clone());
    }
    match listing.delivery {
        Delivery::ShippingAvailable => parts.push("verzenden".to_string()),
        Delivery::PickupOnly => parts.push("ophalen".to_string()),
        Delivery::Unknown => {}
    }
    parts.push(match listing.source.as_str() {
        "vinted" => "Vinted".to_string(),
        "marktplaats" => "Marktplaats".to_string(),
        other => other.to_string(),
    });

    parts.join(" · ")
}

/// A round that found nothing prints nothing, so the cron delivers no message. Without this
/// the user gets fifteen notifications a day and turns it off within a week.
pub fn render_round(findings: &[Finding], problems: &[String]) -> String {
    if findings.is_empty() && problems.is_empty() {
        return String::new();
    }

    let mut out = String::new();

    if !findings.is_empty() {
        let heading = if findings.len() == 1 {
            "1 vondst".to_string()
        } else {
            format!("{} vondsten", findings.len())
        };
        out.push_str(&format!("{heading}\n\n"));
        out.push_str(&render_findings(findings));
    }

    if !problems.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str("PROBLEMEN\n");
        for problem in problems {
            out.push_str(&format!("· {problem}\n"));
        }
    }

    out
}
