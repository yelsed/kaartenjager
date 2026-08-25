//! Checks that run on the server without touching the network, so a broken install shows up
//! immediately instead of as a watcher that quietly reports nothing.
//!
//! The fixtures are real responses captured on 24 August 2026 and are compiled in, so the
//! binary can verify itself anywhere.

use crate::config::{parse_settings, CardRule, Filters, Settings};
use crate::filter::Sieve;
use crate::listing::{Confidence, Delivery, Listing, Rejection};
use crate::pricing::{stated_memory_gb, stated_watts, PriceTable};
use crate::selfupdate;
use crate::sources::{marktplaats, vinted};
use std::path::Path;

const VINTED_FIXTURE: &str = include_str!("../tests/fixtures/vinted_search.json");
const MARKTPLAATS_FIXTURE: &str = include_str!("../tests/fixtures/marktplaats_search.json");

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
        return Err("geen notitie voor laag twee".into());
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
