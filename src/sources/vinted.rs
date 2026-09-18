//! Vinted. De catalogus komt uit een echte browser, de artikelpagina nog gewoon over HTTP.
//!
//! Tot september 2026 gaf `/api/v2/catalog/items` netjes JSON terug. Nu geeft hij 403 en is er
//! geen weg omheen die geen omzeiling is. De gewone cataloguspagina werkt wel, en die staat er
//! al door de server ingevuld -- er hoeft niets nagebootst te worden, alleen gelezen.
//!
//! De ontleder houdt zich uitsluitend vast aan `data-testid`. De klassenamen zijn gehutselde
//! bouwnamen als `ItemBox-module-scss-module__NoC3Da__new-item-box__container` en veranderen bij
//! elke uitrol van Vinted; wie daarop zoekt is binnen een week stuk.

use std::time::{Duration, Instant};

use super::Source;
use crate::browser::Browser;
use crate::http::Failure;
use crate::listing::{Delivery, Listing};

/// Het raster telt er zesennegentig per pagina, dus met de standaardlimiet van zestig komt er
/// nooit een tweede pagina aan te pas. De lus is er voor wie de limiet omhoog zet.
const ROWS_PER_PAGE: usize = 96;

pub struct Vinted<'browser> {
    browser: &'browser mut Browser,
    domain: String,
    /// Al opgerekt door de strikes van de vorige blokkades. Zonder dat door te geven werkt de
    /// terugvalregeling nog wel voor Marktplaats maar niet meer voor Vinted, en juist voor Vinted
    /// is hij geschreven.
    between_pages: Duration,
    page_timeout: Duration,
    max_pages: u32,
    /// De noodrem op de klok. Dertien zoektermen die allemaal op hun eigen timeout wachten duren
    /// samen langer dan het rondeslot van een kwartier; dan staat de wachter stil zonder dat er
    /// iets stuk is.
    deadline: Instant,
    /// Zodra de tijd op is komt er één probleemregel, niet dertien.
    out_of_time: bool,
}

impl<'browser> Vinted<'browser> {
    pub fn new(
        browser: &'browser mut Browser,
        domain: &str,
        between_pages: Duration,
        page_timeout: Duration,
        max_pages: u32,
        budget: Duration,
    ) -> Self {
        Vinted {
            browser,
            domain: domain.to_string(),
            between_pages,
            page_timeout,
            max_pages: max_pages.max(1),
            deadline: Instant::now() + budget,
            out_of_time: false,
        }
    }

    /// Het adres van de bron. Draagt `vinted_domain` al een schema, dan blijft dat staan -- zonder
    /// dat is deze adapter niet tegen een testluisteraar te zetten, en juist hier zijn tweemaal
    /// fouten ingeslopen die alleen met een echt antwoord zichtbaar worden.
    fn base(&self) -> String {
        if self.domain.starts_with("http://") || self.domain.starts_with("https://") {
            self.domain.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", self.domain)
        }
    }

    fn catalog_url(&self, term: &str, page: u32) -> String {
        let mut url = format!(
            "{}/catalog?search_text={}&order=newest_first",
            self.base(),
            crate::http::url_encode(term)
        );
        if page > 1 {
            url.push_str(&format!("&page={page}"));
        }
        url
    }
}

impl Source for Vinted<'_> {
    fn search(&mut self, term: &str, limit: u32) -> Result<Vec<Listing>, Failure> {
        if self.out_of_time {
            return Ok(Vec::new());
        }
        if Instant::now() >= self.deadline {
            self.out_of_time = true;
            return Err(Failure::Other(
                "de tijd voor Vinted in deze ronde is op; de resterende zoektermen zijn \
                 overgeslagen"
                    .to_string(),
            ));
        }

        let mut collected: Vec<Listing> = Vec::new();
        let mut known: Vec<String> = Vec::new();

        for page in 1..=self.max_pages {
            if page > 1 && !self.between_pages.is_zero() {
                std::thread::sleep(self.between_pages);
            }

            let url = self.catalog_url(term, page);
            let loaded = match self.browser.load(&url, self.page_timeout) {
                Ok(loaded) => loaded,
                // Een browser die niet laadt is geen bron die ons tegenhoudt. Dat onderscheid
                // bepaalt of de terugvaltrap gaat lopen, en die hoort hier stil te blijven.
                Err(error) => return Err(Failure::Other(error)),
            };

            let (listings, rows_seen) = parse_catalog(&loaded.html, &self.domain);
            match classify(&loaded.html, loaded.status, listings, rows_seen) {
                Outcome::Items(found) => {
                    let before = collected.len();
                    for listing in found {
                        if known.iter().any(|seen| *seen == listing.listing_id) {
                            continue;
                        }
                        known.push(listing.listing_id.clone());
                        collected.push(listing);
                    }
                    // Niets nieuws, of een pagina die niet vol stond: verder bladeren levert
                    // hetzelfde nog eens op.
                    if collected.len() == before
                        || collected.len() >= limit as usize
                        || rows_seen < ROWS_PER_PAGE
                    {
                        break;
                    }
                }
                // Op de eerste pagina is leeg een antwoord; op een volgende is het het einde.
                Outcome::NoResults => break,
                Outcome::HttpBlocked(status) => {
                    return Err(Failure::Blocked(format!("{url}: HTTP {status}")))
                }
                Outcome::Challenge(what) => {
                    return Err(Failure::Blocked(format!("{url}: {what}")))
                }
                Outcome::Changed(what) => return Err(Failure::Other(format!("{url}: {what}"))),
            }
        }

        collected.truncate(limit as usize);
        Ok(collected)
    }
}

// ───────────────────────────── ontleden, zonder netwerk en zonder browser ─────────────────────

/// Wat er van één cataloguspagina te zeggen valt.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Items(Vec<Listing>),
    NoResults,
    /// Een controlepagina. Wachten helpt, doorgaan niet.
    Challenge(String),
    HttpBlocked(u16),
    /// De opmaak klopt niet meer. Wachten helpt hier juist níét: er moet iemand naar kijken.
    Changed(String),
}

/// Kenmerken van een controlepagina. Allemaal hostnamen en paden, en dat is met opzet: die staan
/// nooit in een woordenboekbestand, en juist daar zit de valkuil.
///
/// Zoek nooit op het woord "datadome". Dat staat zesmaal op een pagina waar niets aan de hand is
/// -- `DATADOME_CLIENT_SIDE_KEY`, `web_datadome_script`, `datadome_script_source` en een
/// Nederlandse foutmelding in het woordenboek. Wie daarop afgaat meldt elke geslaagde ronde als
/// blokkade, en de terugvaltrap zet Vinted dan permanent stil.
const CHALLENGE_MARKERS: [&str; 4] = [
    "geo.captcha-delivery.com",
    "captcha-delivery.com",
    "/interstitial/",
    "dd-captcha",
];

/// Ankers die op élke cataloguspagina staan, ook op een lege. Staan die er niet, dan kijken we
/// niet naar een cataloguspagina.
const PAGE_MARKERS: [&str; 2] = ["<title>Artikelen | Vinted</title>", "Zoekresultaten"];

const ROW_MARKER: &str = "data-testid=\"grid-item\"";

/// Beoordeelt één opgehaalde pagina. Puur: geen netwerk, geen browser, alles met een vast bestand
/// na te spelen. Dat is bewust -- `selftest` draait ook als installatiepoort, en die mag geen
/// browser nodig hebben.
pub fn classify(
    html: &str,
    status: Option<u16>,
    listings: Vec<Listing>,
    rows_seen: usize,
) -> Outcome {
    // De statuscode eerst: DataDome antwoordt met 403, en dan hoeft er niets geraden te worden.
    match status {
        Some(403) | Some(429) => return Outcome::HttpBlocked(status.unwrap_or(403)),
        _ => {}
    }
    if let Some(marker) = CHALLENGE_MARKERS.iter().find(|marker| html.contains(**marker)) {
        return Outcome::Challenge(format!("een controlepagina ({marker})"));
    }
    if let Some(code) = status {
        if code != 200 {
            return Outcome::Changed(format!("HTTP {code}"));
        }
    }
    if !listings.is_empty() {
        // Wel rijen, maar de helft onleesbaar: dat is een verbouwing die halverwege begonnen is.
        // Zonder deze grens zakt de opbrengst stilletjes weg tot iemand het toevallig merkt.
        if listings.len() * 2 < rows_seen {
            return Outcome::Changed(format!(
                "{} van de {rows_seen} advertenties niet te lezen",
                rows_seen - listings.len()
            ));
        }
        return Outcome::Items(listings);
    }
    if rows_seen > 0 {
        return Outcome::Changed(format!("{rows_seen} advertenties, geen enkele te lezen"));
    }
    // Geen rijen, maar de pagina is verder in orde. Zeldzaam -- Vinted valt bij een zoekterm die
    // nergens op slaat terug op een willekeurige feed in plaats van op een lege pagina -- maar de
    // tak moet blijven staan: zonder hem zou elke lege pagina als opmaakwijziging gelden, en dan
    // is die melding niets meer waard.
    if PAGE_MARKERS.iter().all(|marker| html.contains(marker)) {
        return Outcome::NoResults;
    }
    Outcome::Changed("geen enkel bekend anker op de pagina".to_string())
}

/// Leest alle advertenties van één cataloguspagina. Geeft ook terug hoeveel rijen er stonden, want
/// het verschil tussen "gezien" en "gelezen" is het enige signaal dat een verbouwing verraadt.
///
/// Een rij die niet te lezen is wordt overgeslagen en niet gemeld: één kapotte advertentie hoort
/// de andere vijfennegentig niet mee te nemen.
pub fn parse_catalog(html: &str, domain: &str) -> (Vec<Listing>, usize) {
    let host = domain
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0usize;
    while let Some(found) = html[from..].find(ROW_MARKER) {
        let at = from + found;
        // Terug naar het begin van de tag waar dit attribuut in staat.
        starts.push(html[..at].rfind('<').unwrap_or(at));
        from = at + ROW_MARKER.len();
    }

    let mut listings = Vec::new();
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(html.len());
        if let Some(listing) = parse_row(&html[*start..end], host) {
            listings.push(listing);
        }
    }
    (listings, starts.len())
}

fn parse_row(row: &str, host: &str) -> Option<Listing> {
    let id = row_id(row)?;

    let link = tag_containing(row, &format!("product-item-id-{id}--overlay-link"));
    let image = tag_containing(row, &format!("product-item-id-{id}--image--img"));
    // De samenvatting zit tweemaal op de pagina: als `title` op de link en als `alt` op de foto.
    // Eén van de twee is genoeg, en ze bevestigen elkaar.
    let summary = link
        .and_then(|tag| attribute(tag, "title"))
        .or_else(|| image.and_then(|tag| attribute(tag, "alt")))
        .map(unescape)?;
    let (title, condition) = read_summary(&summary);
    if title.is_empty() {
        return None;
    }

    let asking = text_of(row, &format!("product-item-id-{id}--price-text"))
        .and_then(|text| euros(&text))
        .or_else(|| summary_prices(&summary).map(|(asking, _)| asking))?;
    let total = text_of(row, "total-combined-price")
        .and_then(|text| euros(&text))
        .or_else(|| summary_prices(&summary).map(|(_, total)| total))
        .unwrap_or(asking);

    let url = link
        .and_then(|tag| attribute(tag, "href"))
        .map(|href| unescape(href))
        .map(|href| {
            let path = href.split('?').next().unwrap_or(&href).to_string();
            if path.starts_with("http") {
                path
            } else {
                format!("https://{host}{path}")
            }
        })
        .unwrap_or_else(|| format!("https://{host}/items/{id}"));

    Some(Listing {
        source: "vinted".to_string(),
        listing_id: id,
        title,
        // Kopersbescherming is op Vinted niet optioneel, dus het totaal is wat de koper betaalt en
        // het enige getal dat met een Marktplaats-prijs te vergelijken valt.
        price_euros: total,
        asking_price_euros: asking,
        url,
        condition,
        favourite_count: text_of(row, "favourite-count-text")
            .and_then(|text| text.trim().parse::<i64>().ok()),
        // Het raster toont één foto per advertentie, dus tellen levert altijd één op. Nul betekent
        // hier "niet bekend", net als bij Marktplaats; `detail::enrich` vult het echte aantal aan.
        photo_count: 0,
        // Vinted zegt in het raster niets over bezorgen. Onbekend laten en niet gokken: een Franse
        // kaart die alleen op te halen was is eerder doorgeglipt omdat er "verzendt" bij stond.
        delivery: Delivery::Unknown,
        ..Listing::default()
    })
}

/// Het advertentienummer. Staat in `product-item-id-<nummer>` zonder achtervoegsel -- dat laatste
/// is de reden dat er op het afsluitende aanhalingsteken gecontroleerd wordt, want dezelfde
/// voorloop zit ook in `--overlay-link` en `--image--img`.
fn row_id(row: &str) -> Option<String> {
    let needle = "data-testid=\"product-item-id-";
    let mut from = 0usize;
    while let Some(found) = row[from..].find(needle) {
        let at = from + found + needle.len();
        let digits: String = row[at..].chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() && row[at + digits.len()..].starts_with('"') {
            return Some(digits);
        }
        from = at;
    }
    None
}

/// De titel en de staat uit de samenvatting.
///
/// De samenvatting ziet eruit als `"RTX 3090 (hs), Merk: MSI, Staat: Goed, 450.00 €, 473.20 €"`,
/// maar `Merk:` ontbreekt zodra de verkoper geen merk invulde en er kan een `Maat:` tussen staan.
/// Vandaar dat titel en staat hiervandaan komen en niet uit `--description-title`: dat element
/// draagt het merk als er een merk is, en anders de titel. Wie dat voor een titel aanziet, leest
/// bij de helft van de advertenties "NVIDIA".
fn read_summary(summary: &str) -> (String, String) {
    const LABELS: [&str; 3] = [", Merk: ", ", Staat: ", ", Maat: "];
    let title_end = LABELS
        .iter()
        .filter_map(|label| summary.find(label))
        .min()
        .unwrap_or_else(|| strip_price_tail(summary));
    let title = summary[..title_end].trim().to_string();

    let condition = summary
        .find(", Staat: ")
        .map(|at| {
            let rest = &summary[at + ", Staat: ".len()..];
            rest.split(',').next().unwrap_or(rest).trim().to_string()
        })
        .unwrap_or_default();

    (title, condition)
}

/// Waar de twee prijzen aan het eind beginnen. Alleen nodig als er helemaal geen label in staat.
fn strip_price_tail(summary: &str) -> usize {
    let mut end = summary.len();
    for _ in 0..2 {
        let Some(at) = summary[..end].rfind(", ") else {
            return end;
        };
        if !summary[at..end].trim_end().ends_with('€') {
            return end;
        }
        end = at;
    }
    end
}

/// De twee bedragen uit de staart van de samenvatting, als terugval wanneer de prijselementen er
/// niet zijn. Let op: hier staat een punt als decimaalteken en in de opmaak een komma.
fn summary_prices(summary: &str) -> Option<(f64, f64)> {
    let mut amounts: Vec<f64> = summary
        .rsplit(", ")
        .take(2)
        .filter(|part| part.trim_end().ends_with('€'))
        .filter_map(euros)
        .collect();
    if amounts.len() != 2 {
        return None;
    }
    amounts.reverse();
    Some((amounts[0], amounts[1]))
}

/// Een bedrag uit tekst. Moet met beide schrijfwijzen overweg: `€ 278,95` uit de opmaak en
/// `278.95 €` uit de samenvatting, allebei met een vaste spatie ertussen. Vinted schrijft duizenden
/// zonder scheidingsteken (`€ 3780,70`), maar dat is niets om op te bouwen.
pub fn euros(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == ',' || *character == '.')
        .collect();
    if cleaned.is_empty() {
        return None;
    }

    let comma = cleaned.rfind(',');
    let dot = cleaned.rfind('.');
    let normalised = match (comma, dot) {
        // Allebei: de laatste van de twee is het decimaalteken, de andere scheidt duizenden.
        (Some(last_comma), Some(last_dot)) if last_comma > last_dot => {
            cleaned.replace('.', "").replace(',', ".")
        }
        (Some(_), Some(_)) => cleaned.replace(',', ""),
        (Some(_), None) => cleaned.replace(',', "."),
        // Eén punt met twee cijfers erachter is een decimaalteken; al het andere scheidt duizenden.
        (None, Some(last_dot)) => {
            if cleaned.matches('.').count() == 1 && cleaned.len() - last_dot - 1 == 2 {
                cleaned
            } else {
                cleaned.replace('.', "")
            }
        }
        (None, None) => cleaned,
    };

    normalised
        .parse::<f64>()
        .ok()
        .filter(|amount| amount.is_finite() && *amount > 0.0)
}

/// De hele tag waar `needle` in staat, inclusief punthaken. Zo maakt de volgorde van de attributen
/// niet uit -- `href` staat vóór `data-testid` en `title` erachter.
fn tag_containing<'html>(html: &'html str, needle: &str) -> Option<&'html str> {
    let at = html.find(needle)?;
    let start = html[..at].rfind('<')?;
    let end = html[at..].find('>')? + at;
    Some(&html[start..=end])
}

fn attribute<'tag>(tag: &'tag str, name: &str) -> Option<&'tag str> {
    let needle = format!(" {name}=\"");
    let at = tag.find(&needle)? + needle.len();
    let end = tag[at..].find('"')? + at;
    Some(&tag[at..end])
}

/// De tekst in het element dat dit `data-testid` draagt.
fn text_of(html: &str, testid: &str) -> Option<String> {
    let at = html.find(&format!("data-testid=\"{testid}\""))?;
    let opens = html[at..].find('>')? + at + 1;
    let closes = html[opens..].find('<')? + opens;
    Some(unescape(&html[opens..closes]))
}

fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&nbsp;", "\u{a0}")
}
