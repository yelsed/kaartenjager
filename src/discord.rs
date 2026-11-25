//! Vondsten rechtstreeks naar een Discord-webhook (`docs/decisions/0004`).

use crate::config::Settings;
use crate::db::PushCandidate;
use crate::report;
use serde_json::{json, Value};
use std::time::Duration;

/// Discords vlag voor een bericht dat in het kanaal komt zonder melding op de telefoon.
const SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;

pub fn payload(candidate: &PushCandidate, settings: &Settings) -> Value {
    let mention = settings.notify.mention_user_id.as_deref();
    let loud = candidate.level.is_top() && settings.product_notifies(&candidate.product);
    let content = report::render_push(candidate, settings, if loud { mention } else { None });
    let mut body = json!({
        "username": "Kaartenjager",
        "content": content,
        // Alleen de eigenaar mag genoemd worden; een titel met @everyone blijft tekst.
        "allowed_mentions": { "parse": [], "users": if loud { mention.into_iter().collect::<Vec<_>>() } else { Vec::new() } },
    });
    if !loud {
        body["flags"] = json!(SUPPRESS_NOTIFICATIONS);
    }
    body
}

/// Discord laat een webhook ongeveer vijf berichten per twee seconden sturen.
const GAP_BETWEEN_MESSAGES: Duration = Duration::from_millis(450);

/// Post de vondsten na elkaar en roept `delivered` aan voor elke vondst die afgehandeld is: ook
/// een bericht dat Discord weigert, want opnieuw proberen geeft hetzelfde antwoord. Faalt de
/// webhook zelf, dan stopt het en wacht de rest op de volgende ronde. Geeft de problemen voor
/// de app terug.
pub fn post_all(
    webhook: &str,
    candidates: &[PushCandidate],
    settings: &Settings,
    mut delivered: impl FnMut(&PushCandidate) -> Result<(), String>,
) -> Vec<String> {
    let agent = agent();
    let mut problems = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if index > 0 {
            std::thread::sleep(GAP_BETWEEN_MESSAGES);
        }
        match post(&agent, webhook, &payload(candidate, settings)) {
            Ok(()) => {}
            Err(Refusal::ThisMessage(reason)) => {
                problems.push(format!("{reason}: {}. Die staat alleen in de app.", candidate.title));
            }
            Err(Refusal::Everything(reason)) => {
                problems.push(format!(
                    "{reason}. {} van de {} meldingen wachten op de volgende ronde.",
                    candidates.len() - index,
                    candidates.len()
                ));
                break;
            }
        }
        if let Err(error) = delivered(candidate) {
            problems.push(error);
            break;
        }
    }
    problems
}

/// Waarom een bericht niet aankwam, en wat dat zegt over de rest van de ronde.
enum Refusal {
    /// Discord weigerde dit ene bericht (400). Opnieuw proberen geeft hetzelfde antwoord.
    ThisMessage(String),
    /// De webhook, het netwerk of Discords tempo: de rest van de ronde lukt ook niet.
    Everything(String),
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into()
}

fn post(agent: &ureq::Agent, webhook: &str, body: &Value) -> Result<(), Refusal> {
    agent
        .post(webhook)
        .content_type("application/json")
        .send(body.to_string())
        .map(|_| ())
        // Zonder de foutmelding zelf: die kan de webhook-URL bevatten, en daarin staat het geheim.
        .map_err(|error| match error {
            ureq::Error::StatusCode(400) => {
                Refusal::ThisMessage("Discord weigerde een bericht (400)".to_string())
            }
            ureq::Error::StatusCode(code) => Refusal::Everything(format!("Discord antwoordde {code}")),
            ureq::Error::Timeout(_) => {
                Refusal::Everything("Discord antwoordde niet op tijd".to_string())
            }
            _ => Refusal::Everything("Discord was niet bereikbaar".to_string()),
        })
}
