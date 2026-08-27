//! Uitzoeken waarom de wachter stilstaat.
//!
//! `check` gaat over de configuratie en stopt bij de eerste fout. Deze opdracht doet het
//! omgekeerde: hij loopt álles na en gaat door waar het misgaat, want juist de dingen die
//! ná de eerste fout komen vertellen wat er aan de hand is.
//!
//! De aanleiding: het programma viel negen uur stil zonder één spoor. Elke ronde schrijft
//! zijn hartslag weg, dus als die niet meer bijwerkt, struikelt hij vóór de ronde — op de
//! configuratie of op de database. Dat is van een afstand niet te zien, en met een muur
//! shell-opdrachten in een chatvenster ook niet prettig te achterhalen.

use crate::config;
use crate::db::{self, Database};
use std::path::Path;

pub struct Report {
    lines: Vec<String>,
    problems: usize,
}

impl Report {
    fn new() -> Self {
        Report {
            lines: Vec::new(),
            problems: 0,
        }
    }

    fn ok(&mut self, what: &str, detail: impl AsRef<str>) {
        self.lines.push(format!("ok    {what:<22} {}", detail.as_ref()));
    }

    fn bad(&mut self, what: &str, detail: impl AsRef<str>) {
        self.problems += 1;
        self.lines.push(format!("FOUT  {what:<22} {}", detail.as_ref()));
    }

    fn note(&mut self, what: &str, detail: impl AsRef<str>) {
        self.lines.push(format!("      {what:<22} {}", detail.as_ref()));
    }

    pub fn problems(&self) -> usize {
        self.problems
    }
}

/// Loopt alles na. Afdrukken doet de aanroeper, zodat de zelftest hem kan gebruiken zonder
/// een half rapport door zijn eigen uitvoer te mengen.
pub fn diagnose(explicit_config: Option<&Path>, now: i64) -> Report {
    let mut report = Report::new();

    report.note("versie", env!("CARGO_PKG_VERSION"));
    check_clock(&mut report, now);
    let settings = check_config(&mut report, explicit_config);
    check_database(&mut report, settings.as_ref(), now);
    report
}

/// Loopt alles na, drukt het af, en geeft true als er niets mis is.
pub fn run(explicit_config: Option<&Path>, now: i64) -> bool {
    let report = diagnose(explicit_config, now);

    for line in &report.lines {
        println!("{line}");
    }
    println!();
    if report.problems == 0 {
        println!("Niets mis gevonden.");
    } else {
        println!(
            "{} probleem(en). Dat is waar de wachter op stukloopt.",
            report.problems
        );
    }
    report.problems == 0
}

/// Een klok die verkeerd loopt maakt elke tijdstempel onbetrouwbaar, en dat is precies het
/// soort storing dat je nergens aan ziet.
fn check_clock(report: &mut Report, now: i64) {
    if now < 1_700_000_000 {
        report.bad(
            "systeemklok",
            format!("staat op {now} (unix), dat is vóór 2023 — de klok loopt niet goed"),
        );
    } else {
        report.ok("systeemklok", format!("{now} (unix)"));
    }
}

fn check_config(report: &mut Report, explicit: Option<&Path>) -> Option<config::Settings> {
    match config::load(explicit) {
        Ok((settings, path)) => {
            report.ok("configuratie", path.display().to_string());
            report.note(
                "zoektermen in TOML",
                format!(
                    "{} kaarten, {} onderdelen",
                    settings.card_search_terms.len(),
                    settings.part_search_terms.len()
                ),
            );
            report.note(
                "regels",
                format!("{} kaarten, {} onderdelen", settings.cards.len(), settings.parts.len()),
            );
            let stil = settings
                .cards_that_can_never_notify(settings.notify.push_below_market_percent);
            if stil.is_empty() {
                report.ok("meldgrens", format!(
                    "{:.0}% — elke kaartregel kan melden",
                    settings.notify.push_below_market_percent
                ));
            } else {
                report.bad(
                    "meldgrens",
                    format!(
                        "{} kaartregel(s) kunnen nooit melden, want de bodem ligt boven de \
                         meldgrens: {}",
                        stil.len(),
                        stil.iter().map(|(naam, _, _)| *naam).collect::<Vec<_>>().join(", ")
                    ),
                );
            }
            Some(settings)
        }
        Err(error) => {
            // Dit is de meest waarschijnlijke oorzaak van een wachter die stilvalt zonder
            // spoor: het programma stopt hier, vóór het ook maar iets kan wegschrijven.
            report.bad("configuratie", error.to_string());
            None
        }
    }
}

fn check_database(report: &mut Report, settings: Option<&config::Settings>, now: i64) {
    let path = db::default_path();
    report.note("databasepad", path.display().to_string());

    if !path.exists() {
        report.bad(
            "database",
            "bestaat niet. Draai `kaartenjager run` — die maakt hem aan",
        );
        return;
    }

    let database = match Database::open(&path) {
        Ok(database) => database,
        Err(error) => {
            report.bad("database", error);
            return;
        }
    };
    report.ok("database", format!("schema {} in orde", db::SCHEMA_VERSION));

    // Schrijfbaar? Een volle schijf of verkeerde rechten laat het lezen werken en het
    // schrijven stil falen — de app blijft dan gewoon pagina's tonen.
    match database.set_state("doctor_probe", &now.to_string()) {
        Ok(()) => report.ok("schrijfbaar", "een proefregel kon weggeschreven worden"),
        Err(error) => report.bad("schrijfbaar", format!("kan niet schrijven: {error}")),
    }

    check_heartbeat(report, &database, now);
    check_lock(report, &database, now);
    check_terms(report, &database, settings);
    check_sources(report, &database, settings, now);

    report.note(
        "inhoud",
        format!(
            "{} advertenties, {} vondsten, {} openstaande beoordelingen",
            database.count("listing"),
            database.count("finding"),
            database.open_reviews().map(|open| open.len()).unwrap_or(0)
        ),
    );
}

fn check_heartbeat(report: &mut Report, database: &Database, now: i64) {
    let Some(last) = database
        .state("last_round_at")
        .and_then(|value| value.parse::<i64>().ok())
    else {
        report.bad("laatste ronde", "er heeft nog nooit een ronde gedraaid");
        return;
    };

    let age = now - last;
    if age < 0 {
        report.bad(
            "laatste ronde",
            format!("staat {} minuten in de toekomst — de klok liep terug", -age / 60),
        );
    } else if age > 3600 {
        report.bad(
            "laatste ronde",
            format!(
                "{} uur en {} minuten geleden. Elke ronde schrijft dit weg, ook een die \
                 faalt — dus het programma komt niet eens tot de ronde. Kijk hierboven naar \
                 de configuratie en de database, en hieronder naar de cronjob.",
                age / 3600,
                (age % 3600) / 60
            ),
        );
    } else {
        report.ok("laatste ronde", format!("{} minuten geleden", age / 60));
    }

    if let Some(raw) = database.state("last_round_problems") {
        let problems: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
        if problems.is_empty() {
            report.ok("laatste ronde meldde", "geen problemen");
        } else {
            report.note(
                "laatste ronde meldde",
                format!("{} probleem(en):", problems.len()),
            );
            for problem in problems.iter().take(8) {
                report.note("", format!("· {problem}"));
            }
            if problems.len() > 8 {
                report.note("", format!("· … en nog {} andere", problems.len() - 8));
            }
        }
    }
}

fn check_lock(report: &mut Report, database: &Database, now: i64) {
    let Some(since) = database
        .state("round_running_since")
        .and_then(|value| value.parse::<i64>().ok())
    else {
        report.ok("slot", "vrij");
        return;
    };

    let age = now - since;
    if (0..900).contains(&age) {
        report.ok("slot", format!("bezet sinds {} minuten — er loopt een ronde", age / 60));
    } else {
        report.note(
            "slot",
            format!("staat nog open van {} minuten geleden; dat vervalt vanzelf", age / 60),
        );
    }
}

fn check_terms(report: &mut Report, database: &Database, settings: Option<&config::Settings>) {
    match database.enabled_terms() {
        Ok(terms) if terms.is_empty() => {
            report.bad("zoektermen", "er staat er geen één aan, dus een ronde zoekt niets af")
        }
        Ok(terms) => {
            let bronnen = settings.map(|s| s.sources.len()).unwrap_or(2).max(1);
            let verzoeken = terms.len() * bronnen;
            if verzoeken > config::MAX_REQUESTS_PER_ROUND {
                report.bad(
                    "zoektermen",
                    format!(
                        "{} aan maal {bronnen} bronnen is {verzoeken} verzoeken, boven de grens \
                         van {}. De ronde weigert te starten.",
                        terms.len(),
                        config::MAX_REQUESTS_PER_ROUND
                    ),
                );
            } else {
                report.ok(
                    "zoektermen",
                    format!("{} aan, {verzoeken} zoekverzoeken per ronde", terms.len()),
                );
            }
        }
        Err(error) => report.bad("zoektermen", error),
    }
}

fn check_sources(
    report: &mut Report,
    database: &Database,
    settings: Option<&config::Settings>,
    now: i64,
) {
    let standaard = ["vinted".to_string(), "marktplaats".to_string()];
    let bronnen = settings.map(|s| s.sources.as_slice()).unwrap_or(&standaard);

    for bron in bronnen {
        let tot = database.source_blocked_until(bron);
        let strikes = database.source_strikes(bron);
        if now < tot {
            report.note(
                bron,
                format!(
                    "houdt ons tegen; overgeslagen tot over {} minuten ({strikes} keer op rij)",
                    (tot - now) / 60 + 1
                ),
            );
        } else if strikes > 0 {
            report.note(bron, format!("hield ons {strikes} keer tegen, mag nu weer"));
        } else {
            report.ok(bron, "geen blokkade");
        }
    }
}
