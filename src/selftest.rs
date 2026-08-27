//! Checks that run on the server without touching the network, so a broken install shows up
//! immediately instead of as a watcher that quietly reports nothing.
//!
//! The fixtures are real responses captured on 24 August 2026 and are compiled in, so the
//! binary can verify itself anywhere.

use crate::config::{parse_settings, CardRule, Filters, Settings};
use crate::db::Database;
use crate::filter::Sieve;
use crate::listing::{Confidence, Delivery, Finding, FindingKind, Listing, Rejection};
use crate::migrate;
use crate::pricing::{pattern_occurs, stated_memory_gb, stated_watts, PriceTable};
use crate::selfupdate;
use crate::sources::{marktplaats, vinted};
use std::path::{Path, PathBuf};

const VINTED_FIXTURE: &str = include_str!("../tests/fixtures/vinted_search.json");
const MARKTPLAATS_FIXTURE: &str = include_str!("../tests/fixtures/marktplaats_search.json");
const VINTED_ITEM_PAGE: &str = include_str!("../tests/fixtures/vinted_item.html");
const MARKTPLAATS_ITEM_PAGE: &str = include_str!("../tests/fixtures/marktplaats_item.html");
const VINTED_ITEM_SOLD: &str = include_str!("../tests/fixtures/vinted_item_sold.html");

const TEST_CONFIG: &str = r#"
card_search_terms = ["rtx 3090"]
part_search_terms = ["voeding"]

[system]
psu_watts = 850
other_draw_watts = 155
psu_name = "RM850"

[case]
name = "4000D Airflow"
max_gpu_length_mm = 310
max_gpu_length_mm_after_work = 360
work_needed = "eerst de radiator naar boven verplaatsen"
free_slots = 7

[[card]]
name = "RTX 3090 Ti"
patterns = ["3090 ti", "3090ti"]
vram_gb = 24
bandwidth_gbs = 1008
tdp_watt = 450
length_mm_min = 285
length_mm_max = 340
slots_max = 3
used_price_low = 950
used_price_high = 1050
alert_below = 850
suspicious_below = 550

[[card]]
name = "RTX 3090"
patterns = ["3090"]
exclude_patterns = ["3090 ti", "3090ti"]
vram_gb = 24
bandwidth_gbs = 936
tdp_watt = 350
used_price_low = 750
used_price_high = 925
alert_below = 700
suspicious_below = 450

[[card]]
name = "RTX 3060 12GB"
patterns = ["3060"]
exclude_patterns = ["3060 ti"]
vram_gb = 12
tdp_watt = 170
used_price_low = 180
used_price_high = 230
alert_below = 150
suspicious_below = 90
require_memory_in_title = true

[[part]]
name = "Voeding 750W+"
patterns = ["voeding", "psu", "power supply"]
exclude_patterns = ["kabel", "cable", "snoer"]
min_watts = 700
alert_below = 90
suspicious_below = 30
note = "Let op 80 PLUS Gold en maximaal 180 mm lang."

[[part]]
name = "PCIe riser"
patterns = ["pcie riser", "riser kabel", "riser cable", "pci-e verleng"]
exclude_patterns = ["usb", "riser board", "riser card", "proliant", "poweredge"]
alert_below = 25
suspicious_below = 6
note = "Alleen PCIe 4.0 x16 is bruikbaar."
"#;

type Check = (&'static str, fn(&Settings) -> Result<(), String>);

pub fn run() -> bool {
    let settings = match parse_settings(TEST_CONFIG, Path::new("<zelftest>")) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("FOUT  de ingebouwde testconfiguratie laadt niet: {error}");
            return false;
        }
    };
    let mut settings = settings;
    settings.hand_written_cards = settings.cards.iter().map(|card| card.name.clone()).collect();
    let settings = settings;

    if let Err(error) = settings.validate() {
        eprintln!("FOUT  de ingebouwde testconfiguratie is ongeldig: {error}");
        return false;
    }

    let checks: &[Check] = &[
        ("prijstabel kiest de juiste regel", check_matching),
        ("een Ti valt niet in de gewone regel", check_ti_not_plain),
        ("te duur blijft stil", check_too_expensive),
        ("onder de bodem geeft een waarschuwing", check_suspicious),
        ("verkeerde geheugengrootte wordt geweigerd", check_wrong_memory),
        ("ontbrekende geheugengrootte gaat naar de stapel", check_missing_memory),
        ("onbekend maar goedkoop gaat naar de stapel", check_unknown_card),
        ("onbekend met weinig geheugen blijft stil", check_unknown_card_needs_memory),
        ("voeding onder het wattage valt af", check_part_watts),
        ("voedingskabels tellen niet als voeding", check_psu_excludes_cables),
        ("geheugengrootte uit een titel lezen", check_memory_parsing),
        ("wattage uit een titel lezen", check_watts_parsing),
        ("vraagadvertentie wordt geweerd", check_wanted_filter),
        ("verkeerde categorie wordt geweerd", check_category_filter),
        ("toebehoren wordt geweerd", check_accessory_filter),
        ("beschrijving bepaalt het model niet", check_title_only_matching),
        ("ophalen wordt uit de beschrijving gelezen", check_pickup_from_description),
        ("dure ophaal-advertentie wordt gemeld, niet weggegooid", check_expensive_pickup_reported),
        ("kastmaten komen in de redenen", check_case_fit),
        ("kastoordeel klopt in beide richtingen", check_case_fit_reports_only_what_is_true),
        ("ophalen te ver weg wordt geweerd", check_pickup_filter),
        ("Vinted-antwoord ontleden", check_vinted_parsing),
        ("Marktplaats-antwoord ontleden", check_marktplaats_parsing),
        ("Vinted rekent met de totaalprijs", check_vinted_total_price),
        ("prijsherziening laat een kleine stap door", check_review_accepts),
        ("prijsherziening weigert een grote stap", check_review_refuses_jump),
        ("prijsherziening weigert een model zonder bron", check_review_refuses_sourceless),
        ("prijsherziening weigert onlogische drempels", check_review_refuses_illogical),
        ("prijsherziening raakt jouw eigen regels niet aan", check_review_respects_user_file),
        ("automatische tabel groeit aan, vervangt niet", check_auto_table_accumulates),
        ("woordgrenzen bij letters, deelstring bij cijfers", check_word_boundaries),
        ("schema tweemaal openen breekt niets", check_schema_survives_reopening),
        ("een bestaande database migreert naar schema 2", check_migration_to_schema_two),
        ("plaatsingstijd komt uit de foto", check_posted_at_from_photo),
        ("belangstelling wordt bijgehouden", check_interest_is_recorded),
        ("twee rondes tegelijk gaat niet", check_round_lock),
        ("een bron die ons tegenhoudt krijgt rust", check_source_backoff),
        ("403 blijft 403 langs elke weg", check_blocked_survives_every_path),
        ("onleesbare pagina's: verkocht of opmaakwijziging", check_unreadable_verdict),
        ("het wachtrij-vangnet herhaalt zich niet elke ronde", check_nag_does_not_repeat),
        ("waarnemingen alleen bij verandering", check_price_history_only_on_change),
        ("eenmalige velden overleven een ronde", check_one_time_fields_survive),
        ("niet langer interessant, en niet opnieuw nieuw", check_leaving_and_returning),
        ("verdwenen pas na twee hercontroles", check_gone_needs_two_checks),
        ("gereserveerd haalt de vondst uit de inbox", check_reserved_clears_finding),
        ("uitschieterdrempel laat 30% stil en 40% door", check_push_threshold),
        ("een regel die nooit kan melden wordt gemeld", check_silent_rule_is_reported),
        ("het Discord-bericht is vier regels", check_push_message),
        ("oplichterij haalt de drempel maar blijft uit Discord", check_below_floor_stays_quiet),
        ("één melding per advertentie, tien procent lager een tweede", check_push_once),
        ("tweemaal klikken geeft één verzoek", check_one_open_request),
        ("oppakken levert niet tweemaal hetzelfde verzoek", check_take_skips_in_flight),
        ("vastgelopen verzoek komt terug en faalt na drie pogingen", check_review_attempts),
        ("overgang uit de oude bestanden maakt geen herrie", check_migration_is_quiet),
        ("overgang herhalen streept niets weg", check_migration_repeats_safely),
        ("verse installatie begint niet met een lege inbox", check_fresh_install_keeps_first_round),
        ("hercontrole leest prijs en verkocht uit de pagina", check_recheck_parsing),
        ("hercontrole leest een echte Vinted-pagina", check_recheck_vinted_page),
        ("een verkochte Vinted-pagina telt niet als aanwezig", check_sold_vinted_page),
        ("hercontrole leest een echte Marktplaats-pagina", check_recheck_marktplaats_page),
    ];

    let mut failures = 0;
    for (name, check) in checks {
        match check(&settings) {
            Ok(()) => println!("ok    {name}"),
            Err(reason) => {
                failures += 1;
                println!("FOUT  {name}: {reason}");
            }
        }
    }

    if failures == 0 {
        println!("\nAlle {} controles geslaagd.", checks.len());
        true
    } else {
        println!("\n{failures} van de {} controles gefaald.", checks.len());
        false
    }
}

fn listing_with(title: &str, price: f64) -> Listing {
    Listing {
        source: "vinted".to_string(),
        listing_id: "1".to_string(),
        title: title.to_string(),
        price_euros: price,
        asking_price_euros: price,
        url: "https://example.invalid/1".to_string(),
        photo_count: 4,
        delivery: Delivery::ShippingAvailable,
        ..Listing::default()
    }
}

fn check_matching(settings: &Settings) -> Result<(), String> {
    let finding = PriceTable::new(settings)
        .judge(&listing_with("MSI RTX 3090 Ti Gaming X Trio 24GB", 820.0))
        .ok_or("een 3090 Ti van 820 hoort gemeld te worden")?;
    if finding.matched_as != "RTX 3090 Ti" {
        return Err(format!("herkend als {}", finding.matched_as));
    }
    if finding.reasons.len() < 3 {
        return Err(format!("maar {} redenen", finding.reasons.len()));
    }
    Ok(())
}

fn check_ti_not_plain(settings: &Settings) -> Result<(), String> {
    // A Ti at 820 is a bargain; the plain 3090 rule would have stayed silent there, so the
    // exclusion has to hold whatever the order in the file.
    let table = PriceTable::new(settings);
    let ti = table
        .judge(&listing_with("RTX 3090 Ti 24GB", 820.0))
        .ok_or("Ti van 820 werd niet gemeld")?;
    if ti.matched_as != "RTX 3090 Ti" {
        return Err(format!("Ti herkend als {}", ti.matched_as));
    }
    if table.judge(&listing_with("RTX 3090 24GB", 820.0)).is_some() {
        return Err("gewone 3090 van 820 werd gemeld, dat is marktprijs".into());
    }
    Ok(())
}

fn check_too_expensive(settings: &Settings) -> Result<(), String> {
    if PriceTable::new(settings)
        .judge(&listing_with("RTX 3090 Ti 24GB", 999.0))
        .is_some()
    {
        return Err("999 ligt boven de drempel van 850".into());
    }
    Ok(())
}

fn check_suspicious(settings: &Settings) -> Result<(), String> {
    let finding = PriceTable::new(settings)
        .judge(&listing_with("RTX 3090 Ti 24GB", 300.0))
        .ok_or("300 euro hoort gemeld te worden, met waarschuwing")?;
    if finding.confidence != Confidence::NeedsReview {
        return Err("onder de bodem hoort naar de stapel te gaan".into());
    }
    if finding.warnings.is_empty() {
        return Err("geen waarschuwing gegeven".into());
    }
    Ok(())
}

fn check_wrong_memory(settings: &Settings) -> Result<(), String> {
    if PriceTable::new(settings)
        .judge(&listing_with("RTX 3060 8GB", 120.0))
        .is_some()
    {
        return Err("de 8GB-uitvoering is een andere, goedkopere kaart".into());
    }
    if PriceTable::new(settings)
        .judge(&listing_with("RTX 3060 12GB", 120.0))
        .is_none()
    {
        return Err("de 12GB-uitvoering van 120 hoort wel gemeld te worden".into());
    }
    Ok(())
}

fn check_missing_memory(settings: &Settings) -> Result<(), String> {
    let finding = PriceTable::new(settings)
        .judge(&listing_with("RTX 3060 videokaart", 120.0))
        .ok_or("zonder maat in de titel hoort hij alsnog gemeld te worden")?;
    if finding.confidence != Confidence::NeedsReview {
        return Err("hoort met een vlag naar de stapel".into());
    }
    if finding.queue_note.is_none() {
        return Err("geen notitie voor de beoordelaar".into());
    }
    Ok(())
}

fn check_unknown_card(settings: &Settings) -> Result<(), String> {
    let finding = PriceTable::new(settings)
        .judge(&listing_with("Sapphire Radeon RX 6800 XT 16GB", 180.0))
        .ok_or("een goedkope kaart met veel geheugen hoort naar de stapel")?;
    if finding.matched_as != "Onbekend model" {
        return Err(format!("herkend als {}", finding.matched_as));
    }
    if !finding.should_queue() {
        return Err("hoort op de stapel te belanden".into());
    }
    if PriceTable::new(settings)
        .judge(&listing_with("Sapphire Radeon RX 6800 XT 16GB", 400.0))
        .is_some()
    {
        return Err("400 euro is niet opvallend goedkoop voor een onbekend model".into());
    }
    Ok(())
}

fn check_part_watts(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);
    if table
        .judge(&listing_with("Corsair voeding 450W", 60.0))
        .is_some()
    {
        return Err("450 W ligt onder de ingestelde 700 W".into());
    }
    let big = table
        .judge(&listing_with("Corsair RM850 voeding 850W", 60.0))
        .ok_or("een 850 W-voeding van 60 euro hoort gemeld te worden")?;
    if big.matched_as != "Voeding 750W+" {
        return Err(format!("herkend als {}", big.matched_as));
    }
    let vague = table
        .judge(&listing_with("Corsair voeding modulair", 60.0))
        .ok_or("zonder wattage hoort hij naar de stapel, niet weggegooid")?;
    if !vague.should_queue() {
        return Err("zonder wattage hoort hij op de stapel".into());
    }
    Ok(())
}

fn check_psu_excludes_cables(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);
    // A live round reported five power cables as power supplies.
    let cables = [
        "Hama SATA Power Supply Cable - neu OVP",
        "PC voedingskabel",
        "2 Voedingskabels",
    ];
    for title in cables {
        if table.judge(&listing_with(title, 4.0)).is_some() {
            return Err(format!("\"{title}\" werd als voeding gemeld"));
        }
    }
    // A stated wattage below the minimum must be rejected, not treated as unknown.
    if table
        .judge(&listing_with("Witpowly S-1501X 150W Voeding", 53.0))
        .is_some()
    {
        return Err("een 150 W-voeding ligt onder de ingestelde 700 W".into());
    }
    Ok(())
}

fn check_memory_parsing(_settings: &Settings) -> Result<(), String> {
    let cases: [(&str, Option<u32>); 6] = [
        ("RTX 3090 24GB", Some(24)),
        ("RTX 3090 24 GB Trinity", Some(24)),
        ("RTX 3060 8gb", Some(8)),
        ("RTX 3090", None),
        // 3090 is not a capacity, and 2022 is a year.
        ("MSI 3090 uit 2022", None),
        ("RTX 4060 Ti 16GB", Some(16)),
    ];
    for (title, expected) in cases {
        let found = stated_memory_gb(title);
        if found != expected {
            return Err(format!("\"{title}\" gaf {found:?}, verwacht {expected:?}"));
        }
    }
    Ok(())
}

fn check_watts_parsing(_settings: &Settings) -> Result<(), String> {
    let cases: [(&str, Option<u32>); 4] = [
        ("Corsair RM850 850W", Some(850)),
        ("be quiet 1200 W Platinum", Some(1200)),
        ("Corsair voeding modulair", None),
        // 120 is a fan size, not a power supply rating.
        ("ventilator 120mm", None),
    ];
    for (title, expected) in cases {
        let found = stated_watts(title);
        if found != expected {
            return Err(format!("\"{title}\" gaf {found:?}, verwacht {expected:?}"));
        }
    }
    Ok(())
}

fn check_wanted_filter(_settings: &Settings) -> Result<(), String> {
    // Straight out of the live sample: someone who wants a 3090, not one for sale.
    let filters = Filters::default();
    let sieve = Sieve::new(&filters);
    let wanted = Listing {
        title: "RTX4070 Super ruilen tegen RTX3090".to_string(),
        price_euros: 500.0,
        ..listing_with("", 500.0)
    };
    match sieve.check(&wanted) {
        Err(Rejection::WantedAdvertisement(_)) => Ok(()),
        other => Err(format!("werd niet geweerd: {other:?}")),
    }
}

fn check_category_filter(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);

    // A listing Marktplaats files somewhere else is not the card, whatever the title says.
    let elsewhere = Listing {
        categories: vec!["other_computer_and_software".to_string()],
        ..listing_with("RTX 3090 Ti 24GB", 800.0)
    };
    if table.judge(&elsewhere).is_some() {
        return Err("verkeerde categorie werd als kaart gemeld".into());
    }

    let right = Listing {
        categories: vec!["graphic_cards".to_string()],
        ..listing_with("RTX 3090 Ti 24GB", 800.0)
    };
    if table.judge(&right).is_none() {
        return Err("juiste categorie werd ten onrechte geweerd".into());
    }

    // Vinted sends no categories at all; absence must not mean rejection.
    if table
        .judge(&listing_with("RTX 3090 Ti 24GB", 800.0))
        .is_none()
    {
        return Err("zonder categorie hoort hij door te mogen".into());
    }

    // A power supply is filed elsewhere by definition, and the earlier version of this rule
    // threw away every part on Marktplaats.
    let psu = Listing {
        categories: vec!["computer_components".to_string()],
        ..listing_with("Corsair RM850 voeding 850W", 60.0)
    };
    if table.judge(&psu).is_none() {
        return Err("een voeding werd geweerd omdat hij geen videokaart is".into());
    }
    Ok(())
}

fn check_accessory_filter(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);

    // Every one of these came out of a live round as a false positive: they carry a model
    // number in the title but are a water block, an empty box, a dead card or a statue.
    let rejects = [
        "Wasserkühlung für die Grafikkarte GeForce RTX 3090",
        "Watercool waterblock HeatKiller V Pro-Ultra RTX 4090 StriX",
        "Boite vide / Caja vacia Asus Rog Strix RTX 4090",
        "2x RTX 4090 HS Pour pièces",
        "Bloques Barrow para RTX 4090",
        "Disipador / Cooler Nvidia GeForce RTX 5090FE",
        "Miniatura modellino PC Gaming Rog Hyperion rtx 5090",
        "Nvidia GeForce RTX 5090 Founders Edition - Replica 1:1 (Statue)",
        "Rtx 4090 GeForce kfa2 senza chip in buono stato",
        "Original cooling system from an Asus Rog Strix GeForce RTX 4090",
        "Asus rog xg Mobile RTX 4090 16 Go GDDR6 (GC33Y)",
        "RTX 4090 Laptop GPU 16GB",
    ];
    for title in rejects {
        if table.judge(&listing_with(title, 100.0)).is_some() {
            return Err(format!("\"{title}\" werd als kaart gemeld"));
        }
    }

    // A real card must still pass.
    if table
        .judge(&listing_with("MSI RTX 3090 Ti Gaming X Trio 24GB", 800.0))
        .is_none()
    {
        return Err("een echte kaart werd geweerd".into());
    }

    // A riser cable is supposed to say "cable", so the part rules must survive the filter.
    if table
        .judge(&listing_with("TT Gaming Riser Cable PCI-E 3.0 X16 200mm", 16.0))
        .is_none()
    {
        return Err("een riserkabel werd geweerd door het toebehoren-filter".into());
    }
    Ok(())
}

fn check_title_only_matching(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);

    // A live round reported a "PNY GeForce RTX 5080" as a 5090 because its description
    // compared the two. The description decides nothing about what is being sold.
    let confusing = Listing {
        title: "PNY GeForce RTX 5080 16GB Triple Fan Videokaart".to_string(),
        description: "Sneller dan een RTX 3090, bijna een 3090 ti waardig".to_string(),
        price_euros: 600.0,
        asking_price_euros: 600.0,
        ..listing_with("", 600.0)
    };
    if let Some(finding) = table.judge(&confusing) {
        if finding.matched_as.contains("3090") {
            return Err(format!(
                "beschrijving bepaalde het model: herkend als {}",
                finding.matched_as
            ));
        }
    }
    Ok(())
}

fn check_unknown_card_needs_memory(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);

    // Without a memory floor every old card at its ordinary price becomes a notification;
    // a live round produced 112 of them.
    if table.judge(&listing_with("GTX 1050ti", 42.0)).is_some() {
        return Err("een 1050 Ti van 42 euro is gewone marktprijs, geen vondst".into());
    }
    if table.judge(&listing_with("GTX 1660", 79.0)).is_some() {
        return Err("een 1660 zonder geheugenmaat hoort stil te blijven".into());
    }
    if table.judge(&listing_with("Rtx 2060 6GB", 158.0)).is_some() {
        return Err("6 GB ligt onder de vloer van 12 GB".into());
    }
    // Plenty of memory for little money is exactly what should reach the queue.
    if table
        .judge(&listing_with("Radeon RX 6800 XT 16GB", 180.0))
        .is_none()
    {
        return Err("16 GB voor 180 euro hoort wel op de stapel".into());
    }
    Ok(())
}

fn check_pickup_from_description(_settings: &Settings) -> Result<(), String> {
    let words = Filters::default().pickup_words;

    // Straight out of the live listing that started this: a card that looked shippable on
    // Vinted and said "remise en main propre" in its own description.
    let cases = [
        ("Remise en main propre refuse toute autre offre", true),
        ("Alleen ophalen in Amsterdam", true),
        ("Nur Abholung, kein Versand", true),
        ("Solo ritiro a mano", true),
        ("Pickup only please", true),
        ("Carte vendue soigneusement emballée, envoi rapide", false),
        ("Wordt netjes verpakt verstuurd", false),
    ];
    for (description, expected) in cases {
        let mut listing = listing_with("RTX 4090", 1200.0);
        listing.description = description.to_string();
        let found = crate::detail::apply_pickup(&mut listing, &words).is_some();
        if found != expected {
            return Err(format!("\"{description}\" gaf {found}, verwacht {expected}"));
        }
        if expected && !matches!(listing.delivery, Delivery::PickupOnly) {
            return Err(format!("\"{description}\" zette de bezorgwijze niet om"));
        }
    }
    Ok(())
}

fn check_expensive_pickup_reported(_settings: &Settings) -> Result<(), String> {
    let filters = Filters::default();
    let sieve = Sieve::new(&filters);

    // A cheap part far away is genuinely useless.
    let cheap = Listing {
        delivery: Delivery::PickupOnly,
        distance_km: Some(700.0),
        ..listing_with("Computer voeding", 20.0)
    };
    if !matches!(sieve.check(&cheap), Err(Rejection::PickupTooFar(_))) {
        return Err("een voeding van 20 euro op 700 km hoort te vervallen".into());
    }

    // A card worth over a thousand is worth knowing about even if collecting it is awkward.
    let valuable = Listing {
        delivery: Delivery::PickupOnly,
        distance_km: Some(700.0),
        ..listing_with("RTX 4090", 1200.0)
    };
    if sieve.check(&valuable).is_err() {
        return Err("een kaart van 1200 euro hoort gemeld te worden, ook bij alleen ophalen".into());
    }
    Ok(())
}

fn check_case_fit(settings: &Settings) -> Result<(), String> {
    let table = PriceTable::new(settings);

    // The test case takes 310 mm today and 360 mm once the radiator moves. A 3090 Ti runs
    // 285–340 mm, so the short variants fit now and the long ones do not: the report has to
    // say that rather than claim it fits on the strength of work nobody has done.
    let finding = table
        .judge(&listing_with("MSI RTX 3090 Ti Gaming X Trio 24GB", 800.0))
        .ok_or("verwachtte een vondst")?;
    let joined = finding.reasons.join(" | ");

    if !joined.contains("mm") {
        return Err(format!("geen maatregel in de redenen: {joined}"));
    }
    if !joined.contains("VRAAG WELK MODEL") {
        return Err(format!(
            "285-340 mm tegen 310 mm nu hoort om het model te vragen: {joined}"
        ));
    }
    if !joined.contains("310") || !joined.contains("360") {
        return Err(format!("beide maten horen genoemd te worden: {joined}"));
    }
    if !joined.contains("radiator") {
        return Err(format!("het benodigde werk hoort erbij te staan: {joined}"));
    }
    Ok(())
}

fn check_case_fit_reports_only_what_is_true(settings: &Settings) -> Result<(), String> {
    let mut narrow = settings.clone();
    // A case that already takes everything must simply say so.
    if let Some(profile) = narrow.computer_case.as_mut() {
        profile.max_gpu_length_mm = 400;
        profile.max_gpu_length_mm_after_work = 400;
    }
    let finding = PriceTable::new(&narrow)
        .judge(&listing_with("MSI RTX 3090 Ti Gaming X Trio 24GB", 800.0))
        .ok_or("verwachtte een vondst")?;
    let joined = finding.reasons.join(" | ");
    if joined.contains("VRAAG WELK MODEL") || joined.contains("PAST NIET") {
        return Err(format!("400 mm neemt elke uitvoering; geen twijfel nodig: {joined}"));
    }

    // And a case too small for even the shortest variant must say that plainly.
    let mut tiny = settings.clone();
    if let Some(profile) = tiny.computer_case.as_mut() {
        profile.max_gpu_length_mm = 200;
        profile.max_gpu_length_mm_after_work = 220;
    }
    let finding = PriceTable::new(&tiny)
        .judge(&listing_with("MSI RTX 3090 Ti Gaming X Trio 24GB", 800.0))
        .ok_or("verwachtte een vondst")?;
    if !finding.reasons.join(" | ").contains("PAST NIET") {
        return Err("220 mm neemt geen 285 mm; dat hoort er hard te staan".into());
    }
    Ok(())
}

fn check_pickup_filter(_settings: &Settings) -> Result<(), String> {
    let filters = Filters::default();
    let sieve = Sieve::new(&filters);

    let far = Listing {
        delivery: Delivery::PickupOnly,
        distance_km: Some(180.0),
        ..listing_with("PCIe riser kabel", 20.0)
    };
    if !matches!(sieve.check(&far), Err(Rejection::PickupTooFar(_))) {
        return Err("180 km ophalen hoort geweerd te worden".into());
    }

    let near = Listing {
        delivery: Delivery::PickupOnly,
        distance_km: Some(12.0),
        ..listing_with("PCIe riser kabel", 20.0)
    };
    if sieve.check(&near).is_err() {
        return Err("12 km ophalen hoort door te mogen".into());
    }
    Ok(())
}

fn check_vinted_parsing(_settings: &Settings) -> Result<(), String> {
    let body: serde_json::Value =
        serde_json::from_str(VINTED_FIXTURE).map_err(|error| error.to_string())?;
    let listings = vinted::parse_search(&body, "www.vinted.nl");

    if listings.is_empty() {
        return Err("geen enkele advertentie uit het opgeslagen antwoord".into());
    }
    for listing in &listings {
        if listing.title.is_empty() {
            return Err("advertentie zonder titel".into());
        }
        if listing.price_euros <= 0.0 {
            return Err(format!("\"{}\" heeft geen prijs", listing.title));
        }
        if !listing.url.starts_with("https://") {
            return Err(format!("\"{}\" heeft geen bruikbare URL", listing.title));
        }
        if !listing.key().starts_with("vinted:") {
            return Err("sleutel mist de bron".into());
        }
    }
    Ok(())
}

fn check_marktplaats_parsing(_settings: &Settings) -> Result<(), String> {
    let body: serde_json::Value =
        serde_json::from_str(MARKTPLAATS_FIXTURE).map_err(|error| error.to_string())?;
    let listings = marktplaats::parse_search(&body);

    if listings.is_empty() {
        return Err("geen enkele advertentie uit het opgeslagen antwoord".into());
    }

    let with_description = listings
        .iter()
        .filter(|listing| !listing.description.is_empty())
        .count();
    if with_description == 0 {
        return Err("geen enkele beschrijving meegekomen, terwijl die er hoort te zijn".into());
    }

    let with_category = listings
        .iter()
        .filter(|listing| listing.categories.iter().any(|name| name == "graphic_cards"))
        .count();
    if with_category == 0 {
        return Err("geen enkele advertentie kreeg de categorie graphic_cards".into());
    }

    let with_delivery = listings
        .iter()
        .filter(|listing| !matches!(listing.delivery, Delivery::Unknown))
        .count();
    if with_delivery == 0 {
        return Err("bezorgwijze werd nergens gelezen".into());
    }

    for listing in &listings {
        if listing.price_euros <= 0.0 {
            return Err(format!("\"{}\" kwam door met prijs 0", listing.title));
        }
        if !listing.url.starts_with("https://www.marktplaats.nl/") {
            return Err(format!("\"{}\" heeft een rare URL", listing.title));
        }
    }
    Ok(())
}

fn check_vinted_total_price(_settings: &Settings) -> Result<(), String> {
    let body: serde_json::Value =
        serde_json::from_str(VINTED_FIXTURE).map_err(|error| error.to_string())?;
    let listings = vinted::parse_search(&body, "www.vinted.nl");

    // Buyer protection is not optional, so the total is the only number comparable to a
    // Marktplaats price. Getting this wrong flatters every Vinted listing by a few percent.
    let with_fees = listings.iter().find(|listing| listing.has_fees());
    let Some(listing) = with_fees else {
        return Err("geen enkele advertentie had kopersbescherming, verwacht wel".into());
    };
    if listing.price_euros <= listing.asking_price_euros {
        return Err("totaalprijs ligt niet boven de vraagprijs".into());
    }
    Ok(())
}

fn sample_card() -> CardRule {
    CardRule {
        name: "RTX 3090 Ti".to_string(),
        patterns: vec!["3090 ti".to_string()],
        exclude_patterns: Vec::new(),
        vram_gb: 24.0,
        bandwidth_gbs: 1008,
        tdp_watt: 450,
        used_price_low: 950.0,
        used_price_high: 1050.0,
        alert_below: 850.0,
        suspicious_below: 550.0,
        require_memory_in_title: false,
        length_mm_min: 0,
        length_mm_max: 0,
        slots_max: 0,
        source: None,
    }
}

fn check_review_accepts(settings: &Settings) -> Result<(), String> {
    // Run against a table the user did not hand-write, so the merge rules do not mask the
    // thing being tested.
    let mut open_settings = settings.clone();
    open_settings.hand_written_cards.clear();

    let mut proposal = sample_card();
    proposal.used_price_low = 890.0; // -6%
    proposal.used_price_high = 990.0;
    proposal.alert_below = 800.0;

    let review = selfupdate::review(&open_settings, &[proposal]);
    if !review.refused.is_empty() {
        return Err(format!("geweigerd: {:?}", review.refused));
    }
    if review.applied.len() != 3 {
        return Err(format!("{} wijzigingen, verwacht 3", review.applied.len()));
    }
    Ok(())
}

fn check_review_refuses_jump(settings: &Settings) -> Result<(), String> {
    let mut open_settings = settings.clone();
    open_settings.hand_written_cards.clear();
    let settings = &open_settings;

    let mut proposal = sample_card();
    proposal.used_price_low = 600.0; // -37%
    proposal.used_price_high = 700.0;

    let review = selfupdate::review(settings, &[proposal]);
    if review.refused.is_empty() {
        return Err("een stap van -37% hoort geweigerd te worden".into());
    }
    if !review.applied.is_empty() {
        return Err("er werd toch iets toegepast".into());
    }
    Ok(())
}

fn check_review_refuses_sourceless(settings: &Settings) -> Result<(), String> {
    let newcomer = CardRule {
        name: "RX 6800 XT".to_string(),
        patterns: vec!["6800 xt".to_string()],
        used_price_low: 300.0,
        used_price_high: 380.0,
        alert_below: 260.0,
        suspicious_below: 150.0,
        vram_gb: 16.0,
        source: None,
        ..sample_card()
    };

    let review = selfupdate::review(settings, &[newcomer.clone()]);
    if review.refused.is_empty() {
        return Err("een nieuw model zonder bron hoort geweigerd te worden".into());
    }

    let with_source = CardRule {
        source: Some("14 advertenties op Marktplaats en Vinted, mediaan 340".to_string()),
        ..newcomer
    };
    let review = selfupdate::review(settings, &[with_source]);
    if review.added.is_empty() {
        return Err(format!("mét bron alsnog geweigerd: {:?}", review.refused));
    }
    Ok(())
}

fn check_review_respects_user_file(settings: &Settings) -> Result<(), String> {
    // A hand-written card must be reported as untouched, never as applied: saying "applied"
    // for a change that the merge then discards is worse than saying nothing.
    let mut proposal = sample_card();
    proposal.used_price_low = 890.0;
    proposal.alert_below = 800.0;

    let review = selfupdate::review(settings, &[proposal]);
    if !review.applied.is_empty() {
        return Err("een handgeschreven regel werd als gewijzigd gemeld".into());
    }
    if review.user_owned.is_empty() {
        return Err("er werd niet gemeld dat de regel van de gebruiker is".into());
    }
    Ok(())
}

fn check_auto_table_accumulates(settings: &Settings) -> Result<(), String> {
    // A week that proposes nothing about a model must leave it standing. Writing only the
    // accepted rules of the current week silently dropped everything earlier weeks learned.
    let directory = std::env::temp_dir().join(format!(
        "kaartenjager-selftest-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

    let first = CardRule {
        name: "RX 6800 XT".to_string(),
        patterns: vec!["6800 xt".to_string()],
        vram_gb: 16.0,
        used_price_low: 300.0,
        used_price_high: 380.0,
        alert_below: 260.0,
        suspicious_below: 150.0,
        source: Some("14 advertenties, mediaan 340".to_string()),
        ..sample_card()
    };
    let second = CardRule {
        name: "RTX 2070 Super".to_string(),
        patterns: vec!["2070 super".to_string()],
        vram_gb: 8.0,
        used_price_low: 150.0,
        used_price_high: 200.0,
        alert_below: 130.0,
        suspicious_below: 70.0,
        source: Some("18 advertenties, mediaan 175".to_string()),
        ..sample_card()
    };

    let mut table = settings.clone();
    table.hand_written_cards = table.cards.iter().map(|card| card.name.clone()).collect();

    let review = selfupdate::review(&table, &[first.clone()]);
    selfupdate::apply(&directory, &table, &[first.clone()], &review, "2026-08-25")
        .map_err(|error| error.to_string())?;

    // Week two loads what week one wrote, exactly as the real merge does.
    table.cards.push(first.clone());
    let review = selfupdate::review(&table, &[second.clone()]);
    selfupdate::apply(&directory, &table, &[second.clone()], &review, "2026-09-01")
        .map_err(|error| error.to_string())?;

    let written = std::fs::read_to_string(directory.join("cards.auto.toml"))
        .map_err(|error| error.to_string())?;
    let _ = std::fs::remove_dir_all(&directory);

    if !written.contains("RX 6800 XT") {
        return Err("het model van week 1 is verdwenen na week 2".into());
    }
    if !written.contains("RTX 2070 Super") {
        return Err("het model van week 2 is niet weggeschreven".into());
    }
    Ok(())
}

fn check_review_refuses_illogical(settings: &Settings) -> Result<(), String> {
    let mut open_settings = settings.clone();
    open_settings.hand_written_cards.clear();
    let settings = &open_settings;

    let mut proposal = sample_card();
    // A threshold above the market price would report ordinary prices as bargains.
    proposal.alert_below = 1000.0;

    let review = selfupdate::review(settings, &[proposal]);
    if review.refused.is_empty() {
        return Err("alert_below boven used_price_low hoort geweigerd te worden".into());
    }
    Ok(())
}


// ---------------------------------------------------------------------------------------
// De database. Alles hieronder draait op een tijdelijk bestand, zonder netwerk.
// ---------------------------------------------------------------------------------------

fn scratch_directory(name: &str) -> Result<PathBuf, String> {
    let directory = std::env::temp_dir().join(format!(
        "kaartenjager-selftest-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

fn scratch_database(name: &str) -> Result<(Database, PathBuf), String> {
    let directory = scratch_directory(name)?;
    let database = Database::open(&directory.join("kaartenjager.db"))?;
    Ok((database, directory))
}

fn sample_listing(id: &str, price: f64) -> Listing {
    Listing {
        source: "vinted".to_string(),
        listing_id: id.to_string(),
        title: "RTX 4090 Gigabyte Windforce".to_string(),
        url: format!("https://www.vinted.nl/items/{id}"),
        price_euros: price,
        asking_price_euros: price,
        ..Listing::default()
    }
}

fn sample_finding(id: &str, price: f64, percent_under_market: f64) -> Finding {
    Finding {
        listing: sample_listing(id, price),
        matched_as: "RTX 4090".to_string(),
        kind: FindingKind::Card,
        confidence: Confidence::Clear,
        percent_under_market: Some(percent_under_market),
        euros_under_market: Some(1800.0 - price),
        reasons: vec!["onder de markt".to_string()],
        warnings: Vec::new(),
        queue_note: None,
    }
}

fn store(database: &Database, finding: &Finding, now: i64) -> Result<(), String> {
    database.record_listing(&finding.listing, &["rtx 4090".to_string()], now)?;
    database.record_finding(finding, now)
}

/// "borstvoeding" bevat "voeding", en daardoor kwam er een boek over borstvoeding in de
/// resultaten. Modelnummers hebben juist de omgekeerde regel nodig.
fn check_word_boundaries(_settings: &Settings) -> Result<(), String> {
    let letters_need_boundaries = [
        ("borstvoeding 850w", "voeding", false),
        ("kattenvoeding 3kg", "voeding", false),
        ("voedingssupplement", "voeding", false),
        ("corsair voeding 850w", "voeding", true),
        ("nieuwe psu 750w", "psu", true),
        ("psufan vervanger", "psu", false),
        ("pcie riser cable 300mm", "riser", true),
        // Meervouden zijn hetzelfde woord. Sneuvelen die, dan mist hij echte voedingen en —
        // erger — vuren de uitsluitingen niet meer op "kabels" en "adapters".
        ("twee voedingen te koop", "voeding", true),
        ("psu kabels set", "kabel", true),
        ("originele adapters", "adapter", true),
        ("losse snoeren", "snoer", true),
        ("voedingskabel 8-pins", "voeding", false),
    ];
    for (title, pattern, expected) in letters_need_boundaries {
        if pattern_occurs(title, pattern) != expected {
            return Err(format!(
                "\"{pattern}\" in \"{title}\" hoorde {expected} te geven"
            ));
        }
    }

    let digits_stay_substrings = [
        ("rtx3090ti 24gb", "3090 ti", false),
        ("rtx3090ti 24gb", "3090ti", true),
        ("rtx4090 gaming", "4090", true),
        ("msi 3090 ti suprim", "3090 ti", true),
    ];
    for (title, pattern, expected) in digits_stay_substrings {
        if pattern_occurs(title, pattern) != expected {
            return Err(format!(
                "\"{pattern}\" in \"{title}\" hoorde {expected} te geven"
            ));
        }
    }
    Ok(())
}

fn check_schema_survives_reopening(_settings: &Settings) -> Result<(), String> {
    let directory = scratch_directory("schema")?;
    let path = directory.join("kaartenjager.db");

    let first = Database::open(&path)?;
    if !first.freshly_created {
        return Err("een lege map hoort een nieuw schema op te leveren".into());
    }
    store(&first, &sample_finding("1", 1200.0, 33.0), 1_000)?;
    drop(first);

    let again = Database::open(&path)?;
    if again.freshly_created {
        return Err("een bestaande database hoort niet opnieuw aangemaakt te worden".into());
    }
    if again.count("finding") != 1 {
        return Err("de vondst uit de eerste sessie is verdwenen".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

fn check_price_history_only_on_change(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("prijzen")?;

    store(&database, &sample_finding("2", 1200.0, 33.0), 1_000)?;
    store(&database, &sample_finding("2", 1200.0, 33.0), 2_000)?;
    if database.count("sighting") != 1 {
        return Err("dezelfde prijs tweemaal hoort één regel te geven".into());
    }

    store(&database, &sample_finding("2", 1100.0, 39.0), 3_000)?;
    if database.count("sighting") != 2 {
        return Err("een andere prijs hoort een tweede regel te geven".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// Een INSERT OR REPLACE per ronde zou `pushed_at` wissen, en dan meldt Discord dezelfde
/// vondst vijftien keer per dag.
fn check_one_time_fields_survive(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("eenmalig")?;
    let finding = sample_finding("3", 1200.0, 33.0);

    store(&database, &finding, 1_000)?;
    database.mark_pushed("vinted:3", 1200.0, 1_000)?;

    // Vier rondes later, zelfde advertentie, zelfde prijs.
    for round in 1..=4 {
        store(&database, &finding, 1_000 + round * 3_600)?;
    }

    let again = database.findings_to_push(30.0)?;
    if !again.is_empty() {
        return Err("een al gemelde vondst hoort stil te blijven".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// De prijs gaat omhoog: geen vondst meer. Komt hij later terug zonder dat de prijs zakte,
/// dan is dat de drempel die bewoog en geen nieuws.
fn check_leaving_and_returning(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("verlaten")?;

    store(&database, &sample_finding("4", 1200.0, 33.0), 1_000)?;
    if !database.clear_finding("vinted:4", 1900.0, 2_000)? {
        return Err("een te dure advertentie hoort still_a_find op 0 te zetten".into());
    }
    if database.clear_finding("vinted:4", 1900.0, 3_000)? {
        return Err("tweemaal wegstrepen hoort maar één keer te tellen".into());
    }

    // Terug tegen dezelfde prijs als waarop hij eruit liep: de tabel bewoog, niet de markt.
    store(&database, &sample_finding("4", 1900.0, 5.0), 4_000)?;
    let became = database.became_a_find_at("vinted:4")?;
    if became != 1_000 {
        return Err(format!(
            "zonder prijsdaling hoort became_a_find_at op 1000 te blijven, stond op {became}"
        ));
    }

    // Nu wél goedkoper dan toen hij eruit liep: echt nieuws.
    database.clear_finding("vinted:4", 1900.0, 5_000)?;
    store(&database, &sample_finding("4", 1400.0, 22.0), 6_000)?;
    let became = database.became_a_find_at("vinted:4")?;
    if became != 6_000 {
        return Err(format!(
            "een echte prijsdaling hoort became_a_find_at te vernieuwen, stond op {became}"
        ));
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

fn check_gone_needs_two_checks(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("verdwenen")?;
    store(&database, &sample_finding("5", 1200.0, 33.0), 1_000)?;

    if database.note_gone("vinted:5", false, 2_000)? {
        return Err("één keer niet gevonden hoort nog niet verdwenen te zijn".into());
    }
    if !database.note_gone("vinted:5", false, 3_000)? {
        return Err("twee keer op rij hoort wel verdwenen te zijn".into());
    }

    // Toch weer teruggezien: dan bestaat hij nog.
    store(&database, &sample_finding("5", 1200.0, 33.0), 4_000)?;
    if database.due_for_recheck(10, 0)?.is_empty() {
        return Err("een teruggeziene advertentie hoort weer meegecontroleerd te worden".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

fn check_push_threshold(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("drempel")?;

    store(&database, &sample_finding("6", 1260.0, 30.0), 1_000)?;
    if !database.findings_to_push(35.0)?.is_empty() {
        return Err("30% onder de markt hoort bij een grens van 35% stil te blijven".into());
    }

    store(&database, &sample_finding("7", 1080.0, 40.0), 1_000)?;
    let pushes = database.findings_to_push(35.0)?;
    if pushes.len() != 1 || pushes[0].key != "vinted:7" {
        return Err("40% onder de markt hoorde wel gemeld te worden".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

fn check_push_once(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("melden")?;

    store(&database, &sample_finding("8", 1080.0, 40.0), 1_000)?;
    if database.findings_to_push(35.0)?.len() != 1 {
        return Err("de eerste melding hoort te gaan".into());
    }
    database.mark_pushed("vinted:8", 1080.0, 1_000)?;

    if !database.findings_to_push(35.0)?.is_empty() {
        return Err("dezelfde advertentie hoort geen tweede bericht te geven".into());
    }

    // Vijf procent lager is nog geen nieuws.
    store(&database, &sample_finding("8", 1026.0, 43.0), 2_000)?;
    if !database.findings_to_push(35.0)?.is_empty() {
        return Err("vijf procent lager hoort nog stil te blijven".into());
    }

    // Tien procent lager wel.
    store(&database, &sample_finding("8", 972.0, 46.0), 3_000)?;
    if database.findings_to_push(35.0)?.len() != 1 {
        return Err("tien procent lager hoort een tweede bericht te geven".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

fn check_one_open_request(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("wachtrij")?;
    store(&database, &sample_finding("9", 1200.0, 33.0), 1_000)?;

    let first = database.request_review("vinted:9", 1_000)?;
    let second = database.request_review("vinted:9", 1_010)?;
    if first != second {
        return Err("tweemaal drukken hoort hetzelfde verzoek op te leveren".into());
    }
    if database.open_reviews()?.len() != 1 {
        return Err("er hoort één verzoek open te staan, geen twee".into());
    }

    let taken = database.take_reviews(1_020)?;
    if taken.len() != 1 || taken[0].attempts != 0 {
        return Err("oppakken hoort het openstaande verzoek terug te geven".into());
    }
    database.answer_review(first, "ziet er echt uit", "kijken", 1_030)?;
    if !database.open_reviews()?.is_empty() {
        return Err("een beantwoord verzoek hoort niet meer open te staan".into());
    }

    // Beantwoord: dan mag er een nieuw verzoek voor dezelfde advertentie komen.
    let third = database.request_review("vinted:9", 1_040)?;
    if third == first {
        return Err("na een antwoord hoort er een nieuw verzoek te ontstaan".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// Een verzoek dat opgepakt wordt maar nooit beantwoord blijft anders eeuwig terugkomen, en
/// elke terugkeer kost een agent-beurt.
fn check_review_attempts(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("pogingen")?;
    store(&database, &sample_finding("10", 1200.0, 33.0), 1_000)?;
    database.request_review("vinted:10", 1_000)?;

    let mut clock = 2_000;
    for poging in 1..=3 {
        let taken = database.take_reviews(clock)?;
        if taken.len() != 1 {
            return Err(format!("poging {poging} hoorde het verzoek terug te geven"));
        }
        // Een uur verder zonder antwoord: het verzoek komt vanzelf terug in de wachtrij.
        clock += crate::db::STALE_AFTER_SECONDS + 1;
    }

    let taken = database.take_reviews(clock)?;
    if !taken.is_empty() {
        return Err("na drie pogingen hoort het verzoek als mislukt afgesloten te zijn".into());
    }
    if !database.open_reviews()?.is_empty() {
        return Err("een mislukt verzoek hoort niet meer open te staan".into());
    }

    // Mislukt is een eindtoestand, dus de knop werkt weer.
    database.request_review("vinted:10", clock + 10)?;
    if database.open_reviews()?.len() != 1 {
        return Err("na een mislukking hoort een nieuw verzoek te kunnen".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// Zonder de stempels zou alles wat overkomt als "nieuw" gelden en zou Discord tweehonderd
/// oude bekenden opnieuw melden.
fn check_migration_is_quiet(_settings: &Settings) -> Result<(), String> {
    let directory = scratch_directory("overgang")?;
    let finding = sample_finding("11", 1080.0, 40.0);
    let line = serde_json::to_string(&finding).map_err(|error| error.to_string())?;
    std::fs::write(directory.join("recent.jsonl"), format!("{line}\n"))
        .map_err(|error| error.to_string())?;
    // Een halve regel mag de rest niet kosten.
    std::fs::write(directory.join("queue.jsonl"), "{ kapot\n")
        .map_err(|error| error.to_string())?;

    let database = Database::open(&directory.join("kaartenjager.db"))?;
    let outcome = migrate::from_files(&database, &directory, 9_000)?;
    if outcome.findings != 1 {
        return Err(format!("er hoorde één vondst over te komen, het waren er {}", outcome.findings));
    }
    if !database.findings_to_push(35.0)?.is_empty() {
        return Err("een overgezette vondst hoort niet opnieuw naar Discord te gaan".into());
    }
    if database.state("last_visit").as_deref() != Some("9000") {
        return Err("de overgang hoort het laatste bezoek te stempelen, anders is alles nieuw".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// De hercontrole leunt op het schema.org-blok dat beide bronnen in hun advertentiepagina
/// zetten. Een verkochte advertentie telt als verdwenen: kopen kun je hem toch niet meer.
fn check_recheck_parsing(_settings: &Settings) -> Result<(), String> {
    let page = |body: &str| {
        format!(r#"<html><head><script type="application/ld+json">{body}</script></head></html>"#)
    };

    let for_sale = page(
        r#"{"@type":"Product","name":"RTX 4090","description":"Nette kaart",
            "offers":{"@type":"Offer","price":1260.70,"priceCurrency":"EUR",
                      "availability":"https://schema.org/InStock"}}"#,
    );
    match crate::detail::read_page(&for_sale) {
        crate::detail::PageState::Present {
            price_euros: Some(price),
            description,
        } => {
            if (price - 1260.70).abs() > 0.01 {
                return Err(format!("prijs werd {price} in plaats van 1260,70"));
            }
            if description.as_deref() != Some("Nette kaart") {
                return Err("de beschrijving werd niet meegelezen".into());
            }
        }
        _ => return Err("een advertentie die te koop staat hoort een prijs op te leveren".into()),
    }

    // Een prijs als tekst, met een komma, komt ook voor.
    let as_text = page(
        r#"{"@type":"Product","offers":[{"@type":"Offer","price":"945,70"}]}"#,
    );
    match crate::detail::read_page(&as_text) {
        crate::detail::PageState::Present {
            price_euros: Some(price),
            ..
        } if (price - 945.70).abs() < 0.01 => {}
        _ => return Err("een prijs als tekst met komma hoorde gelezen te worden".into()),
    }

    let sold = page(
        r#"{"@type":"Product","offers":{"@type":"Offer","price":1260.70,
            "availability":"https://schema.org/SoldOut"}}"#,
    );
    match crate::detail::read_page(&sold) {
        // Verkocht is iets anders dan verwijderd, en dat verschil hoort bewaard te blijven:
        // binnen het uur verkocht is een koopje dat iemand anders zag.
        crate::detail::PageState::Gone { sold: true } => {}
        crate::detail::PageState::Gone { sold: false } => {
            return Err("een verkochte advertentie hoorde als verkocht te tellen, niet als verwijderd".into())
        }
        _ => return Err("een verkochte advertentie hoort als verdwenen te tellen".into()),
    }

    // Een pagina zonder blok is niet "bestaat nog": op Vinted is dat juist hoe een verkochte
    // advertentie eruitziet. Gemeten op 26 augustus 2026: levende advertenties leveren het
    // blok met InStock, een verkochte levert een pagina van bijna twee megabyte zonder blok.
    // De ronde beslist wat ermee gebeurt, want één onleesbare pagina is iets anders dan
    // allemaal tegelijk.
    if !matches!(
        crate::detail::read_page("<html><body>iets anders</body></html>"),
        crate::detail::PageState::Unreadable
    ) {
        return Err("een pagina zonder blok hoort als onleesbaar te tellen, niet als aanwezig".into());
    }

    Ok(())
}


/// Het schema.org-blok uit een echte advertentiepagina, opgehaald op 25 augustus 2026.
/// Beide bronnen schrijven de prijs anders op: Vinted als getal, Marktplaats als tekst.
fn check_recheck_vinted_page(_settings: &Settings) -> Result<(), String> {
    match crate::detail::read_page(VINTED_ITEM_PAGE) {
        crate::detail::PageState::Present {
            price_euros: Some(price),
            description,
        } => {
            if (price - 15.0).abs() > 0.01 {
                return Err(format!("prijs werd {price} in plaats van 15"));
            }
            if description.is_none() {
                return Err("de beschrijving hoorde meegelezen te worden".into());
            }
            Ok(())
        }
        _ => Err("de Vinted-pagina hoorde een prijs op te leveren".into()),
    }
}

/// Op Marktplaats is het Product-blok niet het eerste: er staat een BreadcrumbList voor.
/// Dat is precies waar de lus overheen moet lopen.
fn check_recheck_marktplaats_page(_settings: &Settings) -> Result<(), String> {
    match crate::detail::read_page(MARKTPLAATS_ITEM_PAGE) {
        crate::detail::PageState::Present {
            price_euros: Some(price),
            ..
        } => {
            if (price - 1199.0).abs() > 0.01 {
                return Err(format!("prijs werd {price} in plaats van 1199"));
            }
            Ok(())
        }
        _ => Err("de Marktplaats-pagina hoorde een prijs op te leveren".into()),
    }
}


/// Vier regels: wat, hoeveel onder, welke uitvoering, waar. De rest staat in de app, en dat
/// was het hele punt van de verhuizing weg van Discord.
fn check_push_message(settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("bericht")?;

    let mut finding = sample_finding("12", 620.0, 38.0);
    finding.matched_as = "RTX 3090".to_string();
    finding.listing.title = "RTX 3090 Founders Edition".to_string();
    finding.listing.delivery = Delivery::PickupOnly;
    store(&database, &finding, 1_000)?;

    let pushes = database.findings_to_push(35.0)?;
    if pushes.len() != 1 {
        return Err("38% onder de markt hoorde gemeld te worden".into());
    }

    let message = crate::report::render_round(&pushes, 0, settings);
    let lines: Vec<&str> = message.trim_end().lines().collect();
    if lines.len() != 4 {
        return Err(format!("het bericht heeft {} regels in plaats van 4:\n{message}", lines.len()));
    }
    if !lines[0].contains("RTX 3090") || !lines[0].contains("620") {
        return Err(format!("regel 1 hoort het model en de prijs te noemen: {}", lines[0]));
    }
    // Het marktbereik komt uit de tabel, niet uit de database.
    if !lines[1].contains("38% onder de markt") || !lines[1].contains("750") {
        return Err(format!("regel 2 hoort het percentage en het marktbereik te noemen: {}", lines[1]));
    }
    if !lines[2].contains("alleen ophalen") || !lines[2].contains("Vinted") {
        return Err(format!("regel 3 hoort de uitvoering en de bron te noemen: {}", lines[2]));
    }
    if !lines[3].starts_with("https://") {
        return Err(format!("regel 4 hoort de link te zijn: {}", lines[3]));
    }

    // En het vangnet onder het wekbericht, als de wachtrij blijft staan.
    let with_queue = crate::report::render_round(&pushes, 2, settings);
    if !with_queue.contains("2 beoordelingen wachten") {
        return Err("een wachtrij die blijft staan hoort zichtbaar te worden".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// Een advertentie die gereserveerd raakt is niet meer te koop. De zeef weert hem, en zonder
/// een schrijfactie zou hij daardoor juist eeuwig als levende vondst blijven staan: de
/// beoordeling ziet hem niet meer, en de hercontrole slaat hem over omdat hij deze ronde
/// wél in de resultaten stond.
fn check_reserved_clears_finding(settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("gereserveerd")?;
    store(&database, &sample_finding("13", 1080.0, 40.0), 1_000)?;

    let mut reserved = sample_listing("13", 1080.0);
    reserved.reserved = true;
    let sieve = Sieve::new(&settings.filters);
    if sieve.check(&reserved).is_ok() {
        return Err("een gereserveerde advertentie hoort geweerd te worden".into());
    }

    // Wat de ronde met een geweerde advertentie doet.
    database.clear_finding("vinted:13", reserved.price_euros, 2_000)?;
    if !database.findings_to_push(35.0)?.is_empty() {
        return Err("een gereserveerde advertentie hoort niet meer gemeld te worden".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// Twee wekberichten kort na elkaar, of een handmatige `reviews take` ernaast: zonder deze
/// regel krijgt de tweede aanroep hetzelfde verzoek opnieuw, en is de pogingengrens binnen
/// een seconde op.
fn check_take_skips_in_flight(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("dubbel-oppakken")?;
    store(&database, &sample_finding("14", 1200.0, 33.0), 1_000)?;
    database.request_review("vinted:14", 1_000)?;

    if database.take_reviews(1_010)?.len() != 1 {
        return Err("het eerste oppakken hoort het verzoek te geven".into());
    }
    if !database.take_reviews(1_020)?.is_empty() {
        return Err("een tweede aanroep hoort niets te geven zolang het eerste nog loopt".into());
    }
    // Maar `pending` toont hem wel: hij staat immers nog open.
    if database.open_reviews()?.len() != 1 {
        return Err("een opgepakt verzoek hoort nog wel open te staan".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// De overgang is te herhalen. Dan mag hij niet alsnog wegstrepen wat je nog niet gezien hebt.
fn check_migration_repeats_safely(_settings: &Settings) -> Result<(), String> {
    let directory = scratch_directory("overgang-herhaald")?;
    let finding = sample_finding("15", 1080.0, 40.0);
    let line = serde_json::to_string(&finding).map_err(|error| error.to_string())?;
    std::fs::write(directory.join("recent.jsonl"), format!("{line}\nkapotte regel\n"))
        .map_err(|error| error.to_string())?;

    let database = Database::open(&directory.join("kaartenjager.db"))?;
    let first = migrate::from_files(&database, &directory, 9_000)?;
    if first.skipped != 1 {
        return Err(format!(
            "een onleesbare regel hoort geteld te worden, skipped stond op {}",
            first.skipped
        ));
    }

    // Doe alsof de app sindsdien een bezoek heeft vastgelegd.
    database.set_state("last_visit", "12345")?;
    migrate::from_files(&database, &directory, 20_000)?;
    if database.state("last_visit").as_deref() != Some("12345") {
        return Err("een herhaalde overgang hoort het laatste bezoek met rust te laten".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// De hoogste kortingspercentages zijn vrijwel altijd oplichting: een kaart op een kwart van
/// de marktprijs is per definitie de luidste melding van de dag. Zonder deze zeef bestaat het
/// Discord-kanaal vooral uit nepadvertenties.
fn check_below_floor_stays_quiet(settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("bodem")?;

    // De testtabel zet de bodem van de 3090 Ti op €550 en de markt op €950.
    let mut oplichterij = sample_finding("16", 300.0, 68.0);
    oplichterij.matched_as = "RTX 3090 Ti".to_string();
    store(&database, &oplichterij, 1_000)?;

    let pushes = crate::hunt::decide_pushes(&database, settings, 1_000)?;
    if !pushes.is_empty() {
        return Err(format!(
            "een vondst onder de bodem hoorde niet gemeld te worden, er gingen er {} uit",
            pushes.len()
        ));
    }

    // Wel gestempeld, anders komt hij elke ronde opnieuw langs en duwt één prijsstijging hem
    // alsnog het kanaal in.
    if !database.findings_to_push(35.0)?.is_empty() {
        return Err("een gefilterde vondst hoort toch zijn stempel te krijgen".into());
    }

    // En een geloofwaardige vondst gaat wél door: boven de bodem, ruim onder de markt.
    let mut echt = sample_finding("17", 600.0, 36.8);
    echt.matched_as = "RTX 3090 Ti".to_string();
    store(&database, &echt, 2_000)?;
    let pushes = crate::hunt::decide_pushes(&database, settings, 2_000)?;
    if pushes.len() != 1 || pushes[0].key != "vinted:17" {
        return Err("een geloofwaardige vondst boven de bodem hoorde wel gemeld te worden".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// Zonder oude bestanden valt er niets weg te strepen. Stempelen zou hier averechts werken:
/// de overgang draait vlak vóór de eerste ronde, dus alles wat die ronde vindt krijgt
/// hetzelfde tijdstempel en zou meteen als "al gezien" gelden — een lege eerste inbox.
fn check_fresh_install_keeps_first_round(_settings: &Settings) -> Result<(), String> {
    let directory = scratch_directory("verse-installatie")?;
    let database = Database::open(&directory.join("kaartenjager.db"))?;

    let outcome = migrate::from_files(&database, &directory, 9_000)?;
    if outcome.findings != 0 {
        return Err("er stonden geen oude bestanden, dus er hoorde niets over te komen".into());
    }
    if database.state("last_visit").is_some() {
        return Err("zonder overgezette vondsten hoort er geen bezoek gestempeld te worden".into());
    }
    // De markering zelf wél, anders probeert elke ronde het opnieuw.
    if database.state("migrated_from_files_at").is_none() {
        return Err("de overgang hoort zichzelf wel af te vinken".into());
    }

    // En dan telt de eerste vondst gewoon als nieuw.
    store(&database, &sample_finding("18", 1200.0, 33.0), 9_000)?;
    let visited: i64 = database.state("last_visit").map_or(0, |value| value.parse().unwrap_or(0));
    if database.became_a_find_at("vinted:18")? <= visited {
        return Err("de eerste vondst van een verse installatie hoort nieuw te zijn".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// De database die al draait staat op schema 1. Die moet meekunnen zonder dat er iets
/// verloren gaat — anders is bijwerken hetzelfde als opnieuw beginnen.
fn check_migration_to_schema_two(_settings: &Settings) -> Result<(), String> {
    let directory = scratch_directory("migratie-2")?;
    let path = directory.join("kaartenjager.db");

    // Een database zoals versie 1 hem achterliet: prijzen in price_point, geen sighting.
    let oud = rusqlite::Connection::open(&path).map_err(|error| error.to_string())?;
    oud.execute_batch(
        r#"
        PRAGMA user_version = 1;
        CREATE TABLE listing (
          key TEXT PRIMARY KEY, source TEXT NOT NULL, listing_id TEXT NOT NULL,
          title TEXT NOT NULL, url TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
          location TEXT NOT NULL DEFAULT '', seller TEXT NOT NULL DEFAULT '',
          condition TEXT NOT NULL DEFAULT '', delivery TEXT NOT NULL DEFAULT 'unknown',
          photo_count INTEGER NOT NULL DEFAULT 0, first_seen INTEGER NOT NULL,
          last_seen INTEGER NOT NULL, found_by_terms TEXT NOT NULL DEFAULT '[]',
          last_checked INTEGER, gone_checks INTEGER NOT NULL DEFAULT 0, gone_since INTEGER
        );
        CREATE TABLE price_point (
          key TEXT NOT NULL, seen_at INTEGER NOT NULL, price_cents INTEGER NOT NULL,
          asking_cents INTEGER NOT NULL, PRIMARY KEY (key, seen_at)
        );
        CREATE TABLE finding (
          key TEXT PRIMARY KEY, matched_as TEXT NOT NULL, kind TEXT NOT NULL,
          confidence TEXT NOT NULL, percent_under_market REAL, euros_under_market REAL,
          reasons TEXT NOT NULL, warnings TEXT NOT NULL, queue_note TEXT,
          became_a_find_at INTEGER NOT NULL, judged_at INTEGER NOT NULL,
          still_a_find INTEGER NOT NULL DEFAULT 1, left_find_at_price INTEGER,
          pushed_at INTEGER, pushed_at_price INTEGER
        );
        CREATE TABLE decision (
          key TEXT PRIMARY KEY, state TEXT NOT NULL, changed_at INTEGER NOT NULL,
          price_when_archived INTEGER, note TEXT
        );
        CREATE TABLE review_request (
          id INTEGER PRIMARY KEY AUTOINCREMENT, key TEXT NOT NULL,
          requested_at INTEGER NOT NULL, taken_at INTEGER, answered_at INTEGER,
          attempts INTEGER NOT NULL DEFAULT 0, verdict TEXT, recommendation TEXT,
          failed_reason TEXT
        );
        CREATE TABLE search_term (
          term TEXT PRIMARY KEY, kind TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1,
          added_at INTEGER NOT NULL, added_by TEXT NOT NULL DEFAULT 'app'
        );
        CREATE TABLE app_state (name TEXT PRIMARY KEY, value TEXT NOT NULL);

        INSERT INTO listing (key, source, listing_id, title, url, first_seen, last_seen)
          VALUES ('vinted:20', 'vinted', '20', 'oude kaart', 'https://x', 100, 200);
        INSERT INTO price_point (key, seen_at, price_cents, asking_cents)
          VALUES ('vinted:20', 100, 50000, 50000), ('vinted:20', 200, 45000, 45000);
        "#,
    )
    .map_err(|error| error.to_string())?;
    drop(oud);

    let database = Database::open(&path)?;
    if database.count("sighting") != 2 {
        return Err("de prijsgeschiedenis is niet meegekomen naar sighting".into());
    }
    if database.listing("vinted:20").is_none() {
        return Err("de advertentie is de migratie niet doorgekomen".into());
    }
    // Nog een keer openen mag niets opnieuw proberen.
    drop(database);
    let opnieuw = Database::open(&path)?;
    if opnieuw.count("sighting") != 2 {
        return Err("tweemaal openen na de migratie ging mis".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// Vinted noemt geen plaatsingstijd, maar de foto draagt zijn uploadmoment mee. Zonder dat
/// weet je alleen wanneer wíj hem zagen, en niet hoe lang hij er al stond.
fn check_posted_at_from_photo(_settings: &Settings) -> Result<(), String> {
    let listings = vinted::parse_search(
        &serde_json::from_str(VINTED_FIXTURE).map_err(|error| error.to_string())?,
        "www.vinted.nl",
    );
    let first = listings.first().ok_or("geen advertenties in het testbestand")?;

    match first.posted_at {
        Some(stamp) if stamp > 1_700_000_000 => {}
        other => return Err(format!("plaatsingstijd werd {other:?}")),
    }
    if first.favourite_count.is_none() {
        return Err("het aantal favorieten hoorde meegelezen te worden".into());
    }
    if first.view_count.is_none() {
        return Err("het aantal kijkers hoorde meegelezen te worden".into());
    }
    Ok(())
}

/// Bij een echt koopje lopen de tellers binnen minuten op. Dat is de enige maat voor
/// belangstelling die we hebben, dus een verandering daarin hoort een regel op te leveren —
/// ook als de prijs gelijk blijft.
fn check_interest_is_recorded(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("belangstelling")?;

    let mut finding = sample_finding("21", 620.0, 38.0);
    finding.listing.view_count = Some(3);
    finding.listing.favourite_count = Some(0);
    store(&database, &finding, 1_000)?;

    // Zelfde prijs, zelfde belangstelling: niets nieuws.
    store(&database, &finding, 2_000)?;
    if database.count("sighting") != 1 {
        return Err("niets veranderd hoort geen tweede regel te geven".into());
    }

    // Zelfde prijs, meer kijkers: dat is wél nieuws.
    finding.listing.view_count = Some(41);
    finding.listing.favourite_count = Some(6);
    store(&database, &finding, 3_000)?;
    if database.count("sighting") != 2 {
        return Err("oplopende belangstelling hoorde vastgelegd te worden".into());
    }

    // Een hercontrole levert geen tellers op; die mag geen lege regel schrijven.
    database.record_price("vinted:21", 620.0, 620.0, 4_000)?;
    if database.count("sighting") != 2 {
        return Err("een hercontrole zonder tellers hoorde niets toe te voegen".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}

/// Bij rondes van vijf minuten kan een trage ronde de volgende inhalen. Twee tegelijk
/// verdubbelen alleen het aantal verzoeken aan Vinted en Marktplaats.
fn check_round_lock(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("slot")?;

    if !database.take_round_lock(1_000, 900)? {
        return Err("het eerste slot hoorde te lukken".into());
    }
    if database.take_round_lock(1_100, 900)? {
        return Err("een tweede ronde hoorde niet te mogen starten".into());
    }

    database.release_round_lock()?;
    if !database.take_round_lock(1_200, 900)? {
        return Err("na loslaten hoorde het weer te mogen".into());
    }

    // Een ronde die omviel laat het slot staan. Dat mag de wachter niet voorgoed stilzetten.
    if !database.take_round_lock(1_200 + 901, 900)? {
        return Err("een vastgelopen slot hoort vanzelf te vervallen".into());
    }

    // En een stempel uit de toekomst, van een klok die terugliep, mag hem al helemaal niet
    // voorgoed stilzetten — dat is precies een storing die je nooit ziet aankomen.
    database.set_state("round_running_since", "999999999999")?;
    if !database.take_round_lock(2_000, 900)? {
        return Err("een slot uit de toekomst hoort te vervallen, niet eeuwig te blokkeren".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// Bij rondes van vijf minuten zou een wachtrij die blijft staan elke ronde hetzelfde bericht
/// naar Discord sturen. Dat is precies de ruis waar dit ontwerp vanaf moest.
fn check_nag_does_not_repeat(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("vangnet")?;
    store(&database, &sample_finding("22", 1200.0, 33.0), 1_000)?;
    database.request_review("vinted:22", 1_000)?;

    // Meteen na het verzoek is er nog niets te melden.
    if database.reviews_waiting_longer_than(900, 1_100)? != 0 {
        return Err("een vers verzoek hoort nog niet gemeld te worden".into());
    }
    // Een kwartier later wel.
    if database.reviews_waiting_longer_than(900, 2_000)? != 1 {
        return Err("een verzoek dat een kwartier wacht hoort gemeld te worden".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// Een verkochte Vinted-advertentie geeft HTTP 200 zonder schema.org-blok. Wie dat als
/// "bestaat nog" leest, houdt hem eeuwig als levende vondst in de inbox — en dat is precies
/// de advertentie waarvan je wilde weten hoe snel hij wegging.
fn check_sold_vinted_page(_settings: &Settings) -> Result<(), String> {
    if !matches!(
        crate::detail::read_page(VINTED_ITEM_SOLD),
        crate::detail::PageState::Unreadable
    ) {
        return Err("een verkochte pagina hoort onleesbaar te heten, niet aanwezig".into());
    }
    // En de val: "Verkocht" staat wél in de pagina, maar alleen in de taalbestanden.
    if !VINTED_ITEM_SOLD.contains("Verkocht") {
        return Err("het testbestand mist juist het woord waarop je niet moet toetsen".into());
    }
    Ok(())
}


/// Vinted blokkeerde de server nadat de scan naar elke vijf minuten ging. Elke ronde daarna
/// dertien zoekopdrachten tegen een dichte deur gooien is precies hoe je een korte rem in een
/// lange blokkade verandert.
fn check_source_backoff(_settings: &Settings) -> Result<(), String> {
    let (database, directory) = scratch_database("blokkade")?;

    if database.source_blocked_until("vinted") != 0 {
        return Err("een onbekende bron hoort niet geblokkeerd te heten".into());
    }

    // Eerste keer: een kwartier rust.
    let wait = database.note_source_blocked("vinted", 10_000)?;
    if wait != 900 {
        return Err(format!("de eerste rustperiode werd {wait} seconden, verwacht 900"));
    }
    if database.source_blocked_until("vinted") != 10_900 {
        return Err("de rustperiode staat niet op het juiste moment".into());
    }

    // Hielp niet: dan verdubbelen, want opnieuw hetzelfde proberen is dieper graven.
    if database.note_source_blocked("vinted", 20_000)? != 1_800 {
        return Err("een tweede blokkade hoort de rustperiode te verdubbelen".into());
    }
    if database.source_strikes("vinted") != 2 {
        return Err("de teller loopt niet mee".into());
    }

    // En de andere bron blijft er los van staan.
    if database.source_blocked_until("marktplaats") != 0 {
        return Err("een blokkade bij de ene bron mag de andere niet raken".into());
    }

    // Een geslaagde ronde zet alles terug.
    database.note_source_healthy("vinted")?;
    if database.source_blocked_until("vinted") != 0 || database.source_strikes("vinted") != 0 {
        return Err("na een geslaagde ronde hoort de blokkade vergeten te zijn".into());
    }

    let _ = std::fs::remove_dir_all(&directory);
    Ok(())
}


/// Een inbox vol oude vondsten is grotendeels verkocht, en die horen gemarkeerd te worden.
/// Maar als de bron zijn opmaak verandert lijkt álles verkocht, en dat zou de inbox
/// onherstelbaar leegvegen — wat verdwenen heet wordt immers niet meer gecontroleerd.
fn check_unreadable_verdict(_settings: &Settings) -> Result<(), String> {
    use crate::hunt::source_markup_looks_broken;

    // Gemengd: de lezer werkt, dus onleesbaar betekent verkocht. Ook als het er veel zijn.
    for (leesbaar, onleesbaar) in [(1usize, 1usize), (1, 29), (5, 25), (10, 0)] {
        if source_markup_looks_broken(leesbaar, onleesbaar) {
            return Err(format!(
                "{onleesbaar} onleesbaar naast {leesbaar} leesbaar hoort gewoon verkocht te heten"
            ));
        }
    }

    // Niets leesbaar: dan valt het onderscheid niet te maken en doen we niets.
    for (leesbaar, onleesbaar) in [(0usize, 1usize), (0, 30)] {
        if !source_markup_looks_broken(leesbaar, onleesbaar) {
            return Err(format!(
                "{onleesbaar} onleesbaar zonder één leesbare pagina hoort verdacht te heten"
            ));
        }
    }

    // Niets onleesbaar is nooit verdacht.
    if source_markup_looks_broken(0, 0) {
        return Err("een ronde zonder onleesbare pagina's hoort niet verdacht te heten".into());
    }
    Ok(())
}


/// De meldgrens is een percentage onder de markt, de oplichtingsbodem een vast bedrag.
/// Kruisen die elkaar, dan is elke advertentie die de meldgrens haalt al als oplichterij
/// weggezet en blijft het kanaal stil — een fout die je pas na weken opvalt.
fn check_silent_rule_is_reported(settings: &Settings) -> Result<(), String> {
    // De testtabel: 3090 Ti met markt 950 en bodem 550. Bij 35% is de grens 617,50, dus die
    // regel kan melden.
    if !settings.cards_that_can_never_notify(35.0).is_empty() {
        return Err("bij 35% hoort geen enkele regel stil te staan in deze tabel".into());
    }

    // Bij 45% zakt de grens naar 522,50 en komt hij onder de bodem van 550 te liggen.
    let stil = settings.cards_that_can_never_notify(45.0);
    if !stil.iter().any(|(naam, _, _)| *naam == "RTX 3090 Ti") {
        return Err("bij 45% hoort de 3090 Ti als stille regel gemeld te worden".into());
    }
    Ok(())
}


/// Een 403 moet als `Blocked` bij de aanroeper aankomen, langs élke weg: de sessie-aanvraag,
/// het zoeken, de hercontrole en het ophalen van een beschrijving.
///
/// Dit is geen theoretische zorg. Tweemaal is precies hier een fout ingeslopen: een
/// `map_err` die `Blocked` platsloeg tot een gewone fout, waardoor de terugvalregeling niet
/// vuurde en de ronde vrolijk dertien zoektermen en dertig hercontroles tegen een dichte deur
/// bleef gooien. Een luisteraar op localhost die niets anders doet dan 403 teruggeven vangt
/// dat, zonder ook maar één keer het echte internet aan te raken.
fn check_blocked_survives_every_path(_settings: &Settings) -> Result<(), String> {
    use crate::detail;
    use crate::http::{Failure, HttpClient};
    use crate::sources::{vinted::Vinted, Source};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();

    // Losgelaten, niet afgewacht: hoeveel verbindingen de cliënt precies maakt hangt af van
    // hergebruik, en wachten op een aantal dat nooit komt zou de zelftest laten hangen.
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut weg = [0u8; 1024];
            let _ = stream.read(&mut weg);
            let _ = stream.write_all(
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });

    let domain = format!("http://127.0.0.1:{port}");
    let mut client = HttpClient::new(0);

    // 1. Zoeken. Hier valt ook de sessie-aanvraag onder: die gaat als eerste naar de
    //    voorpagina, en juist daar hield Vinted ons tegen.
    let mut bron = Vinted::new(&mut client, &domain);
    match bron.search("rtx 3090", 5) {
        Err(Failure::Blocked(_)) => {}
        Err(other) => return Err(format!("zoeken gaf {other} in plaats van tegengehouden")),
        Ok(_) => return Err("zoeken hoorde niet te lukken tegen een dichte deur".into()),
    }
    drop(bron);

    let advertentie = Listing {
        source: "vinted".to_string(),
        listing_id: "1".to_string(),
        url: format!("{domain}/items/1"),
        ..Listing::default()
    };

    // 2. De hercontrole.
    match detail::recheck(&advertentie, &mut client) {
        Err(Failure::Blocked(_)) => {}
        Err(other) => return Err(format!("hercontrole gaf {other} in plaats van tegengehouden")),
        Ok(_) => return Err("een 403 hoort geen uitspraak over de advertentie te zijn".into()),
    }

    // 3. Het ophalen van een beschrijving.
    let mut kopie = advertentie.clone();
    match detail::enrich(&mut kopie, &mut client) {
        Err(Failure::Blocked(_)) => {}
        Err(other) => return Err(format!("beschrijving gaf {other} in plaats van tegengehouden")),
        Ok(()) => return Err("een 403 hoort geen beschrijving op te leveren".into()),
    }

    Ok(())
}
