//! Vondsten rechtstreeks naar een Discord-webhook (`docs/decisions/0004`).

use crate::config::Settings;
use crate::db::PushCandidate;
use crate::listing::Level;
use crate::report;
use serde_json::{json, Value};
use std::time::Duration;

/// Discords vlag voor een bericht dat in het kanaal komt zonder melding op de telefoon.
const SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;

pub fn payload(candidate: &PushCandidate, settings: &Settings) -> Value {
    let mention = settings.notify.mention_user_id.as_deref().filter(|id| !id.trim().is_empty());
    let loud = candidate.level == Level::Top;
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

pub fn post(webhook: &str, body: &Value) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    agent
        .post(webhook)
        .content_type("application/json")
        .send(body.to_string())
        .map(|_| ())
        .map_err(|error| format!("Discord nam de melding niet aan: {error}"))
}
