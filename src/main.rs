mod config;
mod dossier;
mod filter;
mod http;
mod hunt;
mod listing;
mod money;
mod pricing;
mod queue;
mod report;
mod selfupdate;
mod selftest;
mod sources;
mod state;

use config::Settings;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
kaartenjager — houdt Vinted en Marktplaats in de gaten

  kaartenjager run                 Eén ronde: zoeken, melden, onthouden
  kaartenjager run --dry-run       Zelfde ronde, maar niets onthouden of melden
  kaartenjager check               Configuratie controleren en stoppen
  kaartenjager selftest            Ingebouwde controles, zonder netwerk

  kaartenjager queue peek          Toon de stapel voor laag twee
  kaartenjager queue take          Pak de stapel op (en zet hem apart)
  kaartenjager queue done          Meld de opgepakte stapel als afgehandeld

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
        "queue" => command_queue(&arguments),
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
        dry_run: false,
        check_only: false,
        verbose: false,
    };

    let mut index = 1;
    while index < raw.len() {
        match raw[index].as_str() {
            "--dry-run" => arguments.dry_run = true,
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

fn command_run(arguments: &Arguments) -> ExitCode {
    let (settings, _path) = match load_settings(arguments) {
        Ok(loaded) => loaded,
        Err(code) => return code,
    };
    let data_dir = data_directory();

    let outcome = match hunt::run_round(
        &settings,
        &data_dir,
        now_seconds(),
        arguments.dry_run,
        arguments.verbose,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("Ronde mislukt: {error}");
            return ExitCode::from(EXIT_RUN_ERROR);
        }
    };

    if arguments.verbose || arguments.dry_run {
        eprintln!(
            "{} advertenties bekeken, {} nieuw, {} vondsten, {} verzoeken",
            outcome.listings_seen,
            outcome.listings_new,
            outcome.findings.len(),
            outcome.requests_made
        );
        for (reason, count) in &outcome.rejections {
            eprintln!("  {count:5} geweerd: {reason}");
        }
    }

    let message = report::render_round(&outcome.findings, &outcome.problems);
    if !message.is_empty() {
        print!("{message}");
    }

    // Both sources down is a broken watcher, and the cron should show it as failed rather
    // than as a quiet round with nothing to report.
    if outcome.every_source_failed {
        eprintln!("Alle bronnen faalden deze ronde.");
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
        "  verzoeken/ronde    {} (grens {})",
        settings.requests_per_round(),
        config::MAX_REQUESTS_PER_ROUND
    );
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

fn command_queue(arguments: &Arguments) -> ExitCode {
    let data_dir = data_directory();
    let queue = queue::Queue::new(&data_dir);
    let action = arguments.subcommand.as_deref().unwrap_or("peek");

    match action {
        "peek" | "take" => {
            let findings = if action == "take" {
                queue.take()
            } else {
                queue.peek()
            };
            match findings {
                Ok(findings) if findings.is_empty() => {
                    println!("De stapel is leeg.");
                    ExitCode::SUCCESS
                }
                Ok(findings) => {
                    let settings = config::load(arguments.config.as_deref()).ok();
                    for finding in &findings {
                        println!("{}", report::render_one(finding));
                        if let Some(note) = &finding.queue_note {
                            println!("UITZOEKEN\n· {note}\n");
                        }
                        if let Some((settings, _)) = &settings {
                            print!("{}", dossier::render(&finding.listing, settings));
                        }
                        println!("{}", "─".repeat(60));
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Stapel niet leesbaar: {error}");
                    ExitCode::from(EXIT_RUN_ERROR)
                }
            }
        }
        "done" => match queue.mark_done() {
            Ok(count) => {
                println!("{count} van de stapel afgehandeld.");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Stapel niet op te ruimen: {error}");
                ExitCode::from(EXIT_RUN_ERROR)
            }
        },
        other => {
            eprintln!("queue kent peek, take en done — niet \"{other}\".");
            ExitCode::from(EXIT_CONFIG_ERROR)
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

    match hunt::find_listing(&data_directory(), key) {
        Some(listing) => {
            print!("{}", dossier::render(&listing, &settings));
            ExitCode::SUCCESS
        }
        None => {
            eprintln!(
                "Geen advertentie met sleutel \"{key}\" in de recente vondsten of op de stapel."
            );
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
