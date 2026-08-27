mod config;
mod db;
mod detail;
mod doctor;
mod dossier;
mod filter;
mod http;
mod hunt;
mod listing;
mod migrate;
mod money;
mod pricing;
mod report;
mod selfupdate;
mod selftest;
mod sources;

use config::Settings;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
kaartenjager — houdt Vinted en Marktplaats in de gaten

  kaartenjager run                 Eén ronde: zoeken, melden, onthouden
  kaartenjager run --dry-run       Zelfde ronde, maar niets onthouden of melden
  kaartenjager check               Configuratie controleren en stoppen
  kaartenjager doctor              Alles nalopen als de wachter stilstaat
  kaartenjager selftest            Ingebouwde controles, zonder netwerk

  kaartenjager reviews pending     Toon de wachtrij zonder hem op te pakken
  kaartenjager reviews take        Pak de openstaande verzoeken op (JSON)
  kaartenjager reviews answer <id> --recommendation <kijken|overslaan|oplichterij>
                                   Het oordeel zelf gaat via stdin
  kaartenjager reviews fail <id> --reason <tekst>
  kaartenjager reviews request <sleutel>       Een verzoek in de wachtrij zetten

  kaartenjager migrate --from-files            Oude bestanden alsnog overzetten

  kaartenjager dossier <sleutel>   Plakblok voor één advertentie
                                   sleutel is bijvoorbeeld vinted:7005251780

  kaartenjager config apply --from <bestand>   Voorstel keuren en toepassen
  kaartenjager config apply --from <bestand> --check   Alleen tonen wat er zou gebeuren
  kaartenjager config rollback [--to JJJJ-MM-DD]

Opties:
  --config <pad>   Ander configuratiebestand
  --verbose        Meer uitleg op stderr
";

const EXIT_CONFIG_ERROR: u8 = 2;
const EXIT_RUN_ERROR: u8 = 3;

struct Arguments {
    command: String,
    subcommand: Option<String>,
    positional: Vec<String>,
    config: Option<PathBuf>,
    from: Option<PathBuf>,
    to: Option<String>,
    reason: Option<String>,
    recommendation: Option<String>,
    from_files: bool,
    dry_run: bool,
    check_only: bool,
    verbose: bool,
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() || raw[0] == "--help" || raw[0] == "-h" {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if raw[0] == "--version" {
        println!("kaartenjager {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let arguments = match parse_arguments(&raw) {
        Ok(arguments) => arguments,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(EXIT_CONFIG_ERROR);
        }
    };

    match arguments.command.as_str() {
        "selftest" => {
            if selftest::run() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_RUN_ERROR)
            }
        }
        "run" => command_run(&arguments),
        "check" => command_check(&arguments),
        "doctor" => {
            if doctor::run(arguments.config.as_deref(), now_seconds()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_RUN_ERROR)
            }
        }
        "reviews" => command_reviews(&arguments),
        "migrate" => command_migrate(&arguments),
        "dossier" => command_dossier(&arguments),
        "config" => command_config(&arguments),
        other => {
            eprintln!("Onbekende opdracht \"{other}\".\n\n{USAGE}");
            ExitCode::from(EXIT_CONFIG_ERROR)
        }
    }
}

fn parse_arguments(raw: &[String]) -> Result<Arguments, String> {
    let mut arguments = Arguments {
        command: raw[0].clone(),
        subcommand: None,
        positional: Vec::new(),
        config: None,
        from: None,
        to: None,
        reason: None,
        recommendation: None,
        from_files: false,
        dry_run: false,
        check_only: false,
        verbose: false,
    };

    let mut index = 1;
    while index < raw.len() {
        match raw[index].as_str() {
            "--dry-run" => arguments.dry_run = true,
            "--from-files" => arguments.from_files = true,
            "--reason" => {
                index += 1;
                arguments.reason = Some(raw.get(index).ok_or("--reason mist een tekst")?.clone());
            }
            "--recommendation" | "--aanbeveling" => {
                index += 1;
                arguments.recommendation =
                    Some(raw.get(index).ok_or("--recommendation mist een waarde")?.clone());
            }
            "--check" => arguments.check_only = true,
            "--verbose" => arguments.verbose = true,
            "--config" => {
                index += 1;
                arguments.config = Some(PathBuf::from(
                    raw.get(index).ok_or("--config mist een pad")?,
                ));
            }
            "--from" => {
                index += 1;
                arguments.from = Some(PathBuf::from(raw.get(index).ok_or("--from mist een pad")?));
            }
            "--to" | "--naar" => {
                index += 1;
                arguments.to = Some(raw.get(index).ok_or("--to mist een datum")?.clone());
            }
            other if other.starts_with("--") => {
                return Err(format!("Onbekende optie \"{other}\""))
            }
            other => {
                if arguments.subcommand.is_none() && arguments.positional.is_empty() {
                    arguments.subcommand = Some(other.to_string());
                }
                arguments.positional.push(other.to_string());
            }
        }
        index += 1;
    }

    Ok(arguments)
}

fn load_settings(arguments: &Arguments) -> Result<(Settings, PathBuf), ExitCode> {
    match config::load(arguments.config.as_deref()) {
        Ok(loaded) => Ok(loaded),
        Err(error) => {
            eprintln!("{error}");
            Err(ExitCode::from(EXIT_CONFIG_ERROR))
        }
    }
}

fn data_directory() -> PathBuf {
    if let Ok(from_environment) = std::env::var("KAARTENJAGER_DATA") {
        return PathBuf::from(from_environment);
    }
    config::home_directory()
        .map(|home| home.join(".local/share/kaartenjager"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

fn today() -> String {
    let seconds = now_seconds();
    time::OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|moment| {
            moment
                .format(&time::macros::format_description!(
                    "[year]-[month]-[day]"
                ))
                .ok()
        })
        .unwrap_or_else(|| "onbekend".to_string())
}

/// Opens the database, creating and seeding it on first use.
fn open_database(settings: &Settings, now: i64, allow_migration: bool) -> Result<db::Database, String> {
    let database = db::Database::open_default()?;

    // Bij de allereerste start komen de termen uit TOML. De markering zorgt dat het weghalen
    // van je laatste zoekterm de lijst daarna niet opnieuw terugzet.
    database.seed_terms(
        &settings.card_search_terms,
        &settings.part_search_terms,
        now,
    )?;

    // Niet aan "de database is net aangemaakt" ophangen: `check` en `run --dry-run` maken hem
    // ook aan, en dan zou de overgang uit de oude bestanden er nooit meer van komen. De
    // markering die de overgang zelf zet is het enige betrouwbare teken.
    let never_migrated = database.state("migrated_from_files_at").is_none();

    if never_migrated && allow_migration {
        match migrate::from_files(&database, &data_directory(), now) {
            Ok(outcome) if outcome.findings > 0 => eprintln!(
                "Overgezet uit de oude bestanden: {} vondsten, waarvan {} om uit te zoeken.",
                outcome.findings, outcome.needing_review
            ),
            Ok(_) => {}
            Err(error) => eprintln!("Overgang uit de oude bestanden mislukte: {error}"),
        }
    }

    Ok(database)
}

fn command_run(arguments: &Arguments) -> ExitCode {
    let (settings, _path) = match load_settings(arguments) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let now = now_seconds();

    let database = match open_database(&settings, now, !arguments.dry_run) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("Database niet bruikbaar: {error}");
            return ExitCode::from(EXIT_RUN_ERROR);
        }
    };

    let outcome = match hunt::run_round(
        &settings,
        &database,
        now,
        arguments.dry_run,
        arguments.verbose,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            // Ook een ronde die niet eens begint hoort zichtbaar te zijn in de app, anders
            // ziet een stilgevallen wachter eruit als een markt zonder koopjes.
            let _ = database.set_state("last_round_at", &now.to_string());
            let _ = database.set_state(
                "last_round_problems",
                &serde_json::to_string(&[&error]).unwrap_or_else(|_| "[]".to_string()),
            );
            eprintln!("Ronde mislukt: {error}");
            return ExitCode::from(EXIT_RUN_ERROR);
        }
    };

    if arguments.verbose || arguments.dry_run {
        eprintln!(
            "{} advertenties bekeken, {} vondsten, {} niet langer interessant, \
             {} hercontroles waarvan {} verdwenen, {} verzoeken",
            outcome.listings_seen,
            outcome.findings,
            outcome.no_longer_finds,
            outcome.rechecked,
            outcome.newly_gone,
            outcome.requests_made
        );
        for (reason, count) in &outcome.rejections {
            eprintln!("  {count:5} geweerd: {reason}");
        }
    }

    // Problemen gaan naar de app, niet naar Discord.
    //
    // Ook niet naar stderr: de cronjob levert bij Hermes álle uitvoer af, stdout en stderr,
    // en bij rondes van vijf minuten werd dat een muur meldingen per etmaal. Ze staan in de
    // app onder de hartslag, en dat is de plek waar je ze leest. Met --verbose kun je ze hier
    // alsnog zien.
    if arguments.verbose {
        for problem in &outcome.problems {
            eprintln!("probleem: {problem}");
        }
    }

    let message = report::render_round(&outcome.pushes, outcome.reviews_waiting, &settings);
    if !message.is_empty() {
        print!("{message}");
    }

    // Both sources down is a broken watcher, and the cron should show it as failed rather
    // than as a quiet round with nothing to report.
    if outcome.every_source_failed {
        // Dit is geen ruis maar een kapotte wachter, en de cron hoort hem als mislukt te
        // tonen. Eén regel, geen lijst.
        eprintln!("Alle bronnen faalden deze ronde. Kijk in de app voor de reden.");
        return ExitCode::from(EXIT_RUN_ERROR);
    }
    ExitCode::SUCCESS
}

fn command_check(arguments: &Arguments) -> ExitCode {
    let (settings, path) = match load_settings(arguments) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };

    println!("Configuratie in orde: {}", path.display());
    println!("  bronnen            {}", settings.sources.join(", "));
    println!(
        "  zoektermen         {} voor kaarten, {} voor onderdelen",
        settings.card_search_terms.len(),
        settings.part_search_terms.len()
    );
    println!(
        "  regels             {} kaarten, {} onderdelen",
        settings.cards.len(),
        settings.parts.len()
    );
    println!(
        "  zoekverzoeken      {} (grens {})",
        settings.requests_per_round(),
        config::MAX_REQUESTS_PER_ROUND
    );
    println!(
        "  uitschieter        meer dan {:.0}% onder de markt gaat naar Discord, \
         behalve onder suspicious_below",
        settings.notify.push_below_market_percent
    );
    let stille_regels =
        settings.cards_that_can_never_notify(settings.notify.push_below_market_percent);
    if !stille_regels.is_empty() {
        println!(
            "  LET OP             {} kaartregel(s) kunnen nooit een Discord-bericht geven,",
            stille_regels.len()
        );
        println!("                     want de bodem ligt boven de meldgrens:");
        for (naam, grens, bodem) in &stille_regels {
            println!(
                "                       {naam}: meldgrens {} maar bodem {}",
                money::euros(*grens),
                money::euros(*bodem)
            );
        }
        println!(
            "                     Verlaag push_below_market_percent of suspicious_below."
        );
    }

    // Ook `check` zaait de termen: bij de allereerste start is dit het moment waarop de
    // lijst uit TOML in de database komt, en dan hoort het overzicht dat te tonen.
    match open_database(&settings, now_seconds(), false) {
        Ok(database) => {
            let terms = database.enabled_terms().unwrap_or_default();
            println!("  database           {}", database.path.display());
            let searches = terms.len() * settings.sources.len();
            let followed = database.count("finding").min(hunt::RECHECKS_PER_ROUND as i64);
            println!("  zoektermen aan     {} ({searches} zoekverzoeken)", terms.len());
            // De grens hierboven gaat alleen over zoekverzoeken. Hercontroles en
            // beschrijvingen komen daar bovenop, en dat hoort zichtbaar te zijn in plaats
            // van pas op te vallen als een bron gaat weigeren.
            println!(
                "  ronde in totaal    ~{} verzoeken ({searches} zoeken + {} hercontroles \
                 + hooguit {} beschrijvingen)",
                searches as i64 + followed + settings.detail_lookups_per_round as i64,
                followed,
                settings.detail_lookups_per_round
            );
            println!(
                "  hercontrole        volledig elke {} minuten, verse vondsten elke ronde \
                 tot {} uur oud",
                settings.scan.recheck_every_minutes, settings.scan.close_watch_hours
            );
            // Bij rondes van vijf minuten telt dit hard aan; het hoort zichtbaar te zijn in
            // plaats van pas op te vallen als een bron gaat weigeren.
            for (naam, per_dag) in [("elk uur", 15i64), ("elke 5 min", 180)] {
                let zoeken = searches as i64 * per_dag;
                let hercontroles = 30 * (14 * 60 / settings.scan.recheck_every_minutes).max(1)
                    + 5 * per_dag;
                println!(
                    "  {naam:<17} ~{} verzoeken per dag, {} per bron",
                    zoeken + hercontroles,
                    (zoeken + hercontroles) / settings.sources.len().max(1) as i64
                );
            }
            println!(
                "  in de database     {} advertenties, {} vondsten, {} openstaande verzoeken",
                database.count("listing"),
                database.count("finding"),
                database.open_reviews().map(|open| open.len()).unwrap_or(0)
            );
            match database.state("last_round_at") {
                Some(stamp) => println!("  laatste ronde      {stamp} (unix)"),
                None => println!("  laatste ronde      nog geen enkele"),
            }
        }
        Err(error) => println!("  database           NIET BRUIKBAAR: {error}"),
    }
    match &settings.system {
        Some(system) => println!(
            "  machine            {} W voeding, {} W overig verbruik",
            system.psu_watts, system.other_draw_watts
        ),
        None => println!("  machine            niet ingesteld, \"past dit\"-regels blijven weg"),
    }
    if settings.filters.postcode.is_empty() {
        println!("  postcode           LEEG — Marktplaats geeft dan geen afstand terug");
    }
    ExitCode::SUCCESS
}

fn open_database_for_command(arguments: &Arguments) -> Result<db::Database, ExitCode> {
    let (settings, _path) = load_settings(arguments)?;
    open_database(&settings, now_seconds(), false).map_err(|error| {
        eprintln!("Database niet bruikbaar: {error}");
        ExitCode::from(EXIT_RUN_ERROR)
    })
}

fn command_reviews(arguments: &Arguments) -> ExitCode {
    let database = match open_database_for_command(arguments) {
        Ok(database) => database,
        Err(code) => return code,
    };
    let now = now_seconds();
    let action = arguments.subcommand.as_deref().unwrap_or("pending");

    match action {
        // Hermes begint hiermee. Een lege lijst betekent: klaar, niets melden.
        "take" => match database.take_reviews(now) {
            Ok(open) => {
                println!("{}", serde_json::to_string_pretty(&open).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Wachtrij niet op te pakken: {error}");
                ExitCode::from(EXIT_RUN_ERROR)
            }
        },
        "pending" => match database.open_reviews() {
            Ok(open) => {
                println!("{}", serde_json::to_string_pretty(&open).unwrap_or_default());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Wachtrij niet leesbaar: {error}");
                ExitCode::from(EXIT_RUN_ERROR)
            }
        },
        "answer" => {
            let Some(id) = review_id(arguments) else {
                return ExitCode::from(EXIT_CONFIG_ERROR);
            };
            let Some(recommendation) = arguments.recommendation.as_deref() else {
                eprintln!(
                    "reviews answer heeft --recommendation <kijken|overslaan|oplichterij> nodig."
                );
                return ExitCode::from(EXIT_CONFIG_ERROR);
            };
            if !["kijken", "overslaan", "oplichterij"].contains(&recommendation) {
                eprintln!(
                    "\"{recommendation}\" is geen aanbeveling. Kies kijken, overslaan of oplichterij."
                );
                return ExitCode::from(EXIT_CONFIG_ERROR);
            }

            // Het oordeel komt via stdin: een meerregelige tekst door een shell-argument
            // persen vraagt om aanhaalfouten, en die tekst komt van een agent.
            let mut verdict = String::new();
            if let Err(error) = std::io::stdin().read_to_string(&mut verdict) {
                eprintln!("Oordeel niet van stdin te lezen: {error}");
                return ExitCode::from(EXIT_RUN_ERROR);
            }
            if verdict.trim().is_empty() {
                eprintln!("Het oordeel is leeg. Geef de tekst via stdin mee.");
                return ExitCode::from(EXIT_CONFIG_ERROR);
            }

            match database.answer_review(id, verdict.trim(), recommendation, now) {
                Ok(()) => {
                    println!("Verzoek {id} beantwoord.");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(EXIT_RUN_ERROR)
                }
            }
        }
        "fail" => {
            let Some(id) = review_id(arguments) else {
                return ExitCode::from(EXIT_CONFIG_ERROR);
            };
            let reason = arguments.reason.as_deref().unwrap_or("geen reden opgegeven");
            match database.fail_review(id, reason, now) {
                Ok(()) => {
                    println!("Verzoek {id} als mislukt afgesloten.");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(EXIT_RUN_ERROR)
                }
            }
        }
        "request" => {
            let Some(key) = arguments.positional.get(1) else {
                eprintln!("reviews request heeft een sleutel nodig, bijvoorbeeld vinted:7005251780");
                return ExitCode::from(EXIT_CONFIG_ERROR);
            };
            match database.request_review(key, now) {
                Ok(id) => {
                    println!("Verzoek {id} staat in de wachtrij voor {key}.");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(EXIT_RUN_ERROR)
                }
            }
        }
        other => {
            eprintln!(
                "reviews kent pending, take, answer, fail en request — niet \"{other}\"."
            );
            ExitCode::from(EXIT_CONFIG_ERROR)
        }
    }
}

fn review_id(arguments: &Arguments) -> Option<i64> {
    match arguments.positional.get(1).and_then(|raw| raw.parse().ok()) {
        Some(id) => Some(id),
        None => {
            eprintln!("Geef het nummer van het verzoek mee, bijvoorbeeld: reviews answer 12 ...");
            None
        }
    }
}

fn command_migrate(arguments: &Arguments) -> ExitCode {
    if !arguments.from_files {
        eprintln!("migrate kent alleen --from-files.");
        return ExitCode::from(EXIT_CONFIG_ERROR);
    }
    let database = match open_database_for_command(arguments) {
        Ok(database) => database,
        Err(code) => return code,
    };

    match migrate::from_files(&database, &data_directory(), now_seconds()) {
        Ok(outcome) => {
            println!(
                "Overgezet: {} vondsten, waarvan {} om uit te zoeken. {} regels overgeslagen.",
                outcome.findings, outcome.needing_review, outcome.skipped
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Overgang mislukt: {error}");
            ExitCode::from(EXIT_RUN_ERROR)
        }
    }
}

fn command_dossier(arguments: &Arguments) -> ExitCode {
    let Some(key) = arguments.positional.first() else {
        eprintln!("dossier heeft een sleutel nodig, bijvoorbeeld vinted:7005251780");
        return ExitCode::from(EXIT_CONFIG_ERROR);
    };

    let (settings, _path) = match load_settings(arguments) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };

    let database = match open_database(&settings, now_seconds(), false) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("Database niet bruikbaar: {error}");
            return ExitCode::from(EXIT_RUN_ERROR);
        }
    };

    match hunt::find_listing(&database, key) {
        Some(listing) => {
            print!("{}", dossier::render(&listing, &settings));
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("Geen advertentie met sleutel \"{key}\" in de database.");
            ExitCode::from(EXIT_RUN_ERROR)
        }
    }
}

fn command_config(arguments: &Arguments) -> ExitCode {
    let action = arguments.subcommand.as_deref().unwrap_or("check");
    let (settings, config_path) = match load_settings(arguments) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let config_dir: &Path = config_path.parent().unwrap_or(Path::new("."));

    match action {
        "apply" => {
            let Some(from) = &arguments.from else {
                eprintln!("config apply heeft --from <bestand> nodig.");
                return ExitCode::from(EXIT_CONFIG_ERROR);
            };
            let proposed = match selfupdate::read_proposal(from) {
                Ok(proposed) => proposed,
                Err(error) => {
                    eprintln!("Voorstel geweigerd: {error}");
                    return ExitCode::from(EXIT_CONFIG_ERROR);
                }
            };

            let review = selfupdate::review(&settings, &proposed);
            print!("{}", review.render());

            if arguments.check_only {
                println!("\n(alleen gecontroleerd, niets weggeschreven)");
                return ExitCode::SUCCESS;
            }
            if review.applied.is_empty() && review.added.is_empty() {
                return ExitCode::SUCCESS;
            }
            match selfupdate::apply(config_dir, &settings, &proposed, &review, &today()) {
                Ok(path) => {
                    println!("\nWeggeschreven naar {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Toepassen mislukt: {error}");
                    ExitCode::from(EXIT_RUN_ERROR)
                }
            }
        }
        "rollback" => match selfupdate::rollback(config_dir, arguments.to.as_deref()) {
            Ok(restored) => {
                println!("Teruggezet vanaf {}", restored.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(EXIT_RUN_ERROR)
            }
        },
        "check" => command_check(arguments),
        other => {
            eprintln!("config kent apply, rollback en check — niet \"{other}\".");
            ExitCode::from(EXIT_CONFIG_ERROR)
        }
    }
}
