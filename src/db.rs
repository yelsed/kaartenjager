//! De gedeelde database. Het programma schrijft, de Svelte-app leest en schrijft, en Hermes
//! werkt de wachtrij af — alle drie op hetzelfde bestand.
//!
//! WAL laat meerdere lezers naast één schrijver toe, en `busy_timeout` zorgt dat een klik die
//! samenvalt met het wegschrijven van een ronde even wacht in plaats van meteen "database is
//! locked" te geven.

use crate::listing::{Confidence, Delivery, Finding, FindingKind, Listing};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// Opgehoogd bij elke schemawijziging. De Svelte-app controleert dit bij het starten en
/// weigert bij een versie die hij niet kent, in plaats van half te werken op een schema dat
/// hij verkeerd begrijpt.
pub const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = r#"
CREATE TABLE listing (
  key            TEXT PRIMARY KEY,
  source         TEXT NOT NULL,
  listing_id     TEXT NOT NULL,
  title          TEXT NOT NULL,
  url            TEXT NOT NULL,
  description    TEXT NOT NULL DEFAULT '',
  location       TEXT NOT NULL DEFAULT '',
  seller         TEXT NOT NULL DEFAULT '',
  condition      TEXT NOT NULL DEFAULT '',
  delivery       TEXT NOT NULL DEFAULT 'unknown',
  photo_count    INTEGER NOT NULL DEFAULT 0,
  first_seen     INTEGER NOT NULL,
  last_seen      INTEGER NOT NULL,
  found_by_terms TEXT NOT NULL DEFAULT '[]',
  last_checked   INTEGER,
  gone_checks    INTEGER NOT NULL DEFAULT 0,
  gone_since     INTEGER,
  gone_reason    TEXT,                       -- sold | removed
  posted_at      INTEGER                     -- wanneer de verkoper hem plaatste, indien bekend
);

-- Eén regel per waarneming waarin iets veranderde: de prijs, het aantal kijkers of het
-- aantal favorieten. Bij een echt koopje lopen die tellers binnen minuten op, dus wordt de
-- reeks vanzelf dicht waar het spannend is en blijft hij leeg waar niets gebeurt.
CREATE TABLE sighting (
  key             TEXT NOT NULL REFERENCES listing(key) ON DELETE CASCADE,
  seen_at         INTEGER NOT NULL,
  price_cents     INTEGER NOT NULL,
  asking_cents    INTEGER NOT NULL,
  view_count      INTEGER,
  favourite_count INTEGER,
  PRIMARY KEY (key, seen_at)
);

CREATE TABLE finding (
  key                  TEXT PRIMARY KEY REFERENCES listing(key) ON DELETE CASCADE,
  matched_as           TEXT NOT NULL,
  kind                 TEXT NOT NULL,
  confidence           TEXT NOT NULL,
  percent_under_market REAL,
  euros_under_market   REAL,
  reasons              TEXT NOT NULL,
  warnings             TEXT NOT NULL,
  queue_note           TEXT,
  became_a_find_at     INTEGER NOT NULL,
  judged_at            INTEGER NOT NULL,
  still_a_find         INTEGER NOT NULL DEFAULT 1,
  left_find_at_price   INTEGER,
  pushed_at            INTEGER,
  pushed_at_price      INTEGER
);

CREATE TABLE decision (
  key                 TEXT PRIMARY KEY REFERENCES listing(key) ON DELETE CASCADE,
  state               TEXT NOT NULL,
  changed_at          INTEGER NOT NULL,
  price_when_archived INTEGER,
  note                TEXT
);

CREATE TABLE review_request (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  key            TEXT NOT NULL REFERENCES listing(key) ON DELETE CASCADE,
  requested_at   INTEGER NOT NULL,
  taken_at       INTEGER,
  answered_at    INTEGER,
  attempts       INTEGER NOT NULL DEFAULT 0,
  verdict        TEXT,
  recommendation TEXT,
  failed_reason  TEXT
);

CREATE TABLE search_term (
  term      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL,
  enabled   INTEGER NOT NULL DEFAULT 1,
  added_at  INTEGER NOT NULL,
  added_by  TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE app_state (
  name   TEXT PRIMARY KEY,
  value  TEXT NOT NULL
);

CREATE INDEX listing_last_seen ON listing(last_seen);
CREATE INDEX listing_checked   ON listing(last_checked);
CREATE INDEX finding_became    ON finding(became_a_find_at);
CREATE INDEX decision_state    ON decision(state);
CREATE INDEX review_pending    ON review_request(answered_at) WHERE answered_at IS NULL;

CREATE UNIQUE INDEX review_one_open ON review_request(key) WHERE answered_at IS NULL;
"#;

/// Van schema 1 naar 2: waarnemingen in plaats van alleen prijzen, plus wanneer een
/// advertentie geplaatst is en waarom hij weg is.
const MIGRATION_1_TO_2: &str = r#"
ALTER TABLE listing ADD COLUMN gone_reason TEXT;
ALTER TABLE listing ADD COLUMN posted_at INTEGER;

CREATE TABLE sighting (
  key             TEXT NOT NULL REFERENCES listing(key) ON DELETE CASCADE,
  seen_at         INTEGER NOT NULL,
  price_cents     INTEGER NOT NULL,
  asking_cents    INTEGER NOT NULL,
  view_count      INTEGER,
  favourite_count INTEGER,
  PRIMARY KEY (key, seen_at)
);

INSERT INTO sighting (key, seen_at, price_cents, asking_cents)
  SELECT key, seen_at, price_cents, asking_cents FROM price_point;

DROP TABLE price_point;
"#;

/// Een verzoek dat langer dan dit opgepakt is zonder antwoord komt terug in de wachtrij.
pub const STALE_AFTER_SECONDS: i64 = 3600;

/// Zo vaak mag een verzoek opnieuw opgepakt worden voordat het als mislukt geldt. Zonder
/// grens blijft een advertentie waar de agent op stukloopt eeuwig terugkeren, en elke
/// terugkeer kost geld.
pub const MAX_REVIEW_ATTEMPTS: i64 = 3;

pub struct Database {
    connection: Connection,
    pub path: PathBuf,
    /// True wanneer dit bestand net is aangemaakt. Alleen voor de zelftest: de overgang uit
    /// de oude bestanden hangt hier bewust níét aan, want `check` maakt de database ook aan.
    pub freshly_created: bool,
}

/// Wat er naar Discord moet: een vondst die nog nooit gemeld is, of die sinds de melding
/// nog eens tien procent gezakt is.
pub struct PushCandidate {
    pub key: String,
    pub matched_as: String,
    pub title: String,
    pub url: String,
    pub source: String,
    pub delivery: Delivery,
    pub price_euros: f64,
    pub percent_under_market: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReviewRequest {
    pub id: i64,
    pub key: String,
    pub requested_at: i64,
    pub attempts: i64,
    pub title: String,
    pub url: String,
    pub source: String,
    pub price_euros: f64,
    pub matched_as: String,
    pub queue_note: Option<String>,
    pub description: String,
    /// Waarom het programma dit interessant vond, en waar het over twijfelde. Hermes
    /// beoordeelt met dezelfde gegevens waarop het programma zijn oordeel baseerde.
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn default_path() -> PathBuf {
    if let Ok(from_environment) = std::env::var("KAARTENJAGER_DB") {
        return PathBuf::from(from_environment);
    }
    crate::config::home_directory()
        .map(|home| home.join(".local/share/kaartenjager/kaartenjager.db"))
        .unwrap_or_else(|| PathBuf::from("kaartenjager.db"))
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{} kon niet aangemaakt worden: {error}", parent.display()))?;
        }

        let connection = Connection::open(path)
            .map_err(|error| format!("{} kon niet geopend worden: {error}", path.display()))?;

        // journal_mode geeft een rij terug, dus pragma_update werkt hier niet.
        connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get::<_, String>(0))
            .map_err(|error| format!("WAL-modus instellen mislukte: {error}"))?;
        connection
            .execute_batch("PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
            .map_err(|error| format!("Databaseinstellingen mislukten: {error}"))?;

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| format!("Schemaversie niet leesbaar: {error}"))?;

        let freshly_created = version == 0;
        if freshly_created {
            connection
                .execute_batch(SCHEMA)
                .map_err(|error| format!("Schema aanmaken mislukte: {error}"))?;
            connection
                .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
                .map_err(|error| format!("Schemaversie zetten mislukte: {error}"))?;
        } else if version == 1 {
            // In één transactie, zodat een halve migratie geen database achterlaat die
            // nergens meer op lijkt.
            connection
                .execute_batch(&format!(
                    "BEGIN; {MIGRATION_1_TO_2} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
                ))
                .map_err(|error| format!("Migratie naar schema 2 mislukte: {error}"))?;
        } else if version > SCHEMA_VERSION {
            return Err(format!(
                "{} is aangemaakt door een nieuwere versie van kaartenjager (schema {version}, \
                 deze versie kent {SCHEMA_VERSION}). Werk het programma bij.",
                path.display()
            ));
        }

        Ok(Database {
            connection,
            path: path.to_path_buf(),
            freshly_created,
        })
    }

    pub fn open_default() -> Result<Self, String> {
        Self::open(&default_path())
    }

    pub fn begin(&self) -> Result<(), String> {
        self.connection
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|error| format!("Transactie starten mislukte: {error}"))
    }

    pub fn commit(&self) -> Result<(), String> {
        self.connection
            .execute_batch("COMMIT")
            .map_err(|error| format!("Transactie afsluiten mislukte: {error}"))
    }

    pub fn rollback(&self) {
        let _ = self.connection.execute_batch("ROLLBACK");
    }

    // ---------------------------------------------------------------- app_state

    pub fn state(&self, name: &str) -> Option<String> {
        self.connection
            .query_row("SELECT value FROM app_state WHERE name = ?1", params![name], |row| {
                row.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_state(&self, name: &str, value: &str) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO app_state (name, value) VALUES (?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET value = excluded.value",
                params![name, value],
            )
            .map(|_| ())
            .map_err(|error| format!("{name} niet weg te schrijven: {error}"))
    }

    // -------------------------------------------------------------- zoektermen

    /// Vult de tabel één keer met de termen uit TOML. De markering is er zodat het weghalen
    /// van je laatste zoekterm de lijst niet opnieuw uit het bestand terugzet.
    pub fn seed_terms(&self, cards: &[String], parts: &[String], now: i64) -> Result<bool, String> {
        if self.state("terms_seeded").is_some() {
            return Ok(false);
        }
        for (terms, kind) in [(cards, "card"), (parts, "part")] {
            for term in terms {
                self.connection
                    .execute(
                        "INSERT OR IGNORE INTO search_term (term, kind, enabled, added_at, added_by)
                         VALUES (?1, ?2, 1, ?3, 'config')",
                        params![term.to_lowercase(), kind, now],
                    )
                    .map_err(|error| format!("Zoekterm \"{term}\" niet opslaan: {error}"))?;
            }
        }
        self.set_state("terms_seeded", &now.to_string())?;
        Ok(true)
    }

    /// De aanstaande termen, kaarten eerst. Dit is wat een ronde afzoekt.
    pub fn enabled_terms(&self) -> Result<Vec<String>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT term FROM search_term WHERE enabled = 1
                 ORDER BY CASE kind WHEN 'card' THEN 0 ELSE 1 END, term",
            )
            .map_err(|error| format!("Zoektermen niet op te vragen: {error}"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Zoektermen niet te lezen: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Zoektermen niet te lezen: {error}"))
    }

    // ---------------------------------------------------------------- listings

    /// Schrijft de advertentie weg en voegt een prijsregel toe wanneer de prijs veranderd is.
    /// Alleen advertenties die een vondst werden komen hier langs; de rest laat geen spoor na.
    pub fn record_listing(&self, listing: &Listing, terms: &[String], now: i64) -> Result<(), String> {
        let key = listing.key();
        // Aanvullen, niet vervangen: "rtx 4090" en "geforce rtx" vinden dezelfde kaart, en
        // welke van de twee hem deze ronde opleverde zegt niets over de andere.
        let mut all_terms: std::collections::BTreeSet<String> = self
            .connection
            .query_row(
                "SELECT found_by_terms FROM listing WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .and_then(|stored| serde_json::from_str(&stored).ok())
            .unwrap_or_default();
        all_terms.extend(terms.iter().cloned());
        let terms_json = serde_json::to_string(&all_terms).unwrap_or_else(|_| "[]".to_string());

        self.connection
            .execute(
                "INSERT INTO listing (
                    key, source, listing_id, title, url, description, location, seller,
                    condition, delivery, photo_count, first_seen, last_seen, found_by_terms,
                    posted_at
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12,?13,?14)
                 ON CONFLICT(key) DO UPDATE SET
                    title       = excluded.title,
                    url         = excluded.url,
                    description = CASE WHEN excluded.description <> '' THEN excluded.description
                                       ELSE listing.description END,
                    location    = excluded.location,
                    seller      = excluded.seller,
                    condition   = excluded.condition,
                    delivery    = excluded.delivery,
                    photo_count = excluded.photo_count,
                    last_seen   = excluded.last_seen,
                    -- Eenmaal bekend blijft de plaatsingstijd staan: latere waarnemingen
                    -- kunnen hem niet beter weten, en een bron die hem een keer weglaat mag
                    -- hem niet wissen.
                    posted_at   = COALESCE(listing.posted_at, excluded.posted_at),
                    -- Teruggezien betekent: hij bestaat nog. Een eerdere twijfel vervalt.
                    gone_checks = 0,
                    gone_since  = NULL,
                    gone_reason = NULL,
                    found_by_terms = ?13",
                params![
                    key,
                    listing.source,
                    listing.listing_id,
                    listing.title,
                    listing.url,
                    listing.description,
                    listing.location,
                    listing.seller,
                    listing.condition,
                    delivery_name(&listing.delivery),
                    listing.photo_count as i64,
                    now,
                    terms_json,
                    listing.posted_at,
                ],
            )
            .map_err(|error| format!("{key} niet op te slaan: {error}"))?;

        self.record_sighting(listing, now)
    }

    /// Legt vast wat we bij deze waarneming zagen, maar alleen als er iets veranderd is aan
    /// de prijs, het aantal kijkers of het aantal favorieten.
    ///
    /// Elke ronde een regel zou bij rondes van vijf minuten honderden regels per advertentie
    /// per dag opleveren voor niets. Schrijven-bij-verandering maakt de reeks juist dicht
    /// waar het spannend is: bij een echt koopje lopen de tellers binnen minuten op, en bij
    /// een advertentie waar niemand naar kijkt gebeurt er niets.
    pub fn record_sighting(&self, listing: &Listing, now: i64) -> Result<(), String> {
        let key = listing.key();
        let price = to_cents(listing.price_euros);
        let asking = to_cents(listing.asking_price_euros);

        let latest: Option<(i64, Option<i64>, Option<i64>)> = self
            .connection
            .query_row(
                "SELECT price_cents, view_count, favourite_count FROM sighting
                 WHERE key = ?1 ORDER BY seen_at DESC LIMIT 1",
                params![&key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| format!("Waarnemingen van {key} niet leesbaar: {error}"))?;

        // Een teller die de bron deze keer niet meegaf telt niet als verandering; anders
        // zou een hercontrole, die geen tellers oplevert, elke keer een lege regel schrijven.
        if let Some((was_price, was_views, was_favourites)) = latest {
            let same_price = was_price == price;
            let same_views = listing.view_count.is_none() || listing.view_count == was_views;
            let same_favourites =
                listing.favourite_count.is_none() || listing.favourite_count == was_favourites;
            if same_price && same_views && same_favourites {
                return Ok(());
            }
        }

        self.connection
            .execute(
                "INSERT OR REPLACE INTO sighting
                    (key, seen_at, price_cents, asking_cents, view_count, favourite_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    key,
                    now,
                    price,
                    asking,
                    listing.view_count,
                    listing.favourite_count
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("Waarneming van {key} niet op te slaan: {error}"))
    }

    /// Voor de hercontrole, die alleen een prijs terugvindt en geen tellers.
    pub fn record_price(
        &self,
        key: &str,
        price_euros: f64,
        asking_euros: f64,
        now: i64,
    ) -> Result<(), String> {
        let mut listing = Listing {
            source: String::new(),
            listing_id: String::new(),
            price_euros,
            asking_price_euros: asking_euros,
            ..Listing::default()
        };
        // record_sighting leidt de sleutel af uit bron en id; hier hebben we hem al.
        let (source, id) = key.split_once(':').unwrap_or((key, ""));
        listing.source = source.to_string();
        listing.listing_id = id.to_string();
        self.record_sighting(&listing, now)
    }

    // ---------------------------------------------------------------- findings

    /// Werkt de vondst bij zonder de eenmalige velden aan te raken. Een INSERT OR REPLACE zou
    /// `pushed_at` wissen, en dan meldt Discord dezelfde vondst elke ronde opnieuw.
    ///
    /// `became_a_find_at` schuift alleen op wanneer de advertentie eerder uit de vondsten liep
    /// én nu goedkoper is dan toen. Een drempel die door de wekelijkse herziening verschoof
    /// is geen nieuws.
    pub fn record_finding(&self, finding: &Finding, now: i64) -> Result<(), String> {
        let key = finding.listing.key();
        let reasons = serde_json::to_string(&finding.reasons).unwrap_or_else(|_| "[]".to_string());
        let warnings = serde_json::to_string(&finding.warnings).unwrap_or_else(|_| "[]".to_string());
        let price = to_cents(finding.listing.price_euros);

        let existing: Option<(i64, Option<i64>)> = self
            .connection
            .query_row(
                "SELECT still_a_find, left_find_at_price FROM finding WHERE key = ?1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("Vondst {key} niet leesbaar: {error}"))?;

        let renew_became = match existing {
            None => true,
            Some((still_a_find, left_at)) => {
                still_a_find == 0 && left_at.map(|was| price < was).unwrap_or(false)
            }
        };

        self.connection
            .execute(
                "INSERT INTO finding (
                    key, matched_as, kind, confidence, percent_under_market, euros_under_market,
                    reasons, warnings, queue_note, became_a_find_at, judged_at, still_a_find,
                    left_find_at_price
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10,1,NULL)
                 ON CONFLICT(key) DO UPDATE SET
                    matched_as           = excluded.matched_as,
                    kind                 = excluded.kind,
                    confidence           = excluded.confidence,
                    percent_under_market = excluded.percent_under_market,
                    euros_under_market   = excluded.euros_under_market,
                    reasons              = excluded.reasons,
                    warnings             = excluded.warnings,
                    queue_note           = excluded.queue_note,
                    judged_at            = excluded.judged_at,
                    still_a_find         = 1,
                    left_find_at_price   = NULL,
                    became_a_find_at     = CASE WHEN ?11 THEN excluded.became_a_find_at
                                                ELSE finding.became_a_find_at END",
                params![
                    key,
                    finding.matched_as,
                    finding.kind.name(),
                    confidence_name(finding.confidence),
                    finding.percent_under_market,
                    finding.euros_under_market,
                    reasons,
                    warnings,
                    finding.queue_note,
                    now,
                    renew_became,
                ],
            )
            .map_err(|error| format!("Vondst {key} niet op te slaan: {error}"))?;

        // Een nieuwe vondst begint in de inbox; een bestaande beslissing blijft van jou.
        self.connection
            .execute(
                "INSERT OR IGNORE INTO decision (key, state, changed_at) VALUES (?1, 'inbox', ?2)",
                params![key, now],
            )
            .map(|_| ())
            .map_err(|error| format!("Beslissing voor {key} niet op te slaan: {error}"))
    }

    /// De prijs ging omhoog of de tabel veranderde: dit is geen vondst meer. Zonder deze
    /// schrijfactie blijft `still_a_find` eeuwig op 1 staan, want de beoordeling levert bij
    /// een te hoge prijs simpelweg niets op om weg te schrijven.
    pub fn clear_finding(&self, key: &str, price_euros: f64, now: i64) -> Result<bool, String> {
        let changed = self
            .connection
            .execute(
                "UPDATE finding SET still_a_find = 0, left_find_at_price = ?2, judged_at = ?3
                 WHERE key = ?1 AND still_a_find = 1",
                params![key, to_cents(price_euros), now],
            )
            .map_err(|error| format!("Vondst {key} niet bij te werken: {error}"))?;
        Ok(changed > 0)
    }

    /// Wanneer dit voor het eerst een vondst werd. Alleen deze bepaalt wat er in het
    /// nieuw-blok staat; `judged_at` schuift wel elke ronde mee.
    pub fn became_a_find_at(&self, key: &str) -> Result<i64, String> {
        self.connection
            .query_row(
                "SELECT became_a_find_at FROM finding WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .map_err(|error| format!("Vondst {key} niet leesbaar: {error}"))
    }

    /// De beschrijving die we eerder ophaalden. Het zoekresultaat van Vinted draagt er geen,
    /// dus zonder dit zou elke ronde dezelfde detailpagina opnieuw ophalen — bij rondes van
    /// vijf minuten honderden keren per dag voor tekst die al in de database staat.
    pub fn stored_description(&self, key: &str) -> Option<String> {
        self.connection
            .query_row(
                "SELECT description FROM listing WHERE key = ?1 AND description <> ''",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn has_finding(&self, key: &str) -> bool {
        self.connection
            .query_row("SELECT 1 FROM finding WHERE key = ?1", params![key], |_| Ok(()))
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    // --------------------------------------------------------------- Discord

    /// Uitschieters die nog niet gemeld zijn, of die sinds de melding nog eens tien procent
    /// zakten. Eén bericht per advertentie, niet per ronde.
    pub fn findings_to_push(&self, minimum_percent: f64) -> Result<Vec<PushCandidate>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT f.key, f.matched_as, f.percent_under_market, f.pushed_at,
                        f.pushed_at_price, l.title, l.url, l.source, l.delivery,
                        (SELECT price_cents FROM sighting p WHERE p.key = f.key
                         ORDER BY p.seen_at DESC LIMIT 1) AS price_cents
                 FROM finding f
                 JOIN listing l ON l.key = f.key
                 WHERE f.still_a_find = 1
                   AND f.kind = 'card'
                   AND f.percent_under_market >= ?1
                   AND l.gone_since IS NULL",
            )
            .map_err(|error| format!("Uitschieters niet op te vragen: {error}"))?;

        let rows = statement
            .query_map(params![minimum_percent], |row| {
                let pushed_at: Option<i64> = row.get(3)?;
                let pushed_price: Option<i64> = row.get(4)?;
                let price_cents: Option<i64> = row.get(9)?;
                Ok((
                    PushCandidate {
                        key: row.get(0)?,
                        matched_as: row.get(1)?,
                        percent_under_market: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                        title: row.get(5)?,
                        url: row.get(6)?,
                        source: row.get(7)?,
                        delivery: delivery_from_name(&row.get::<_, String>(8)?),
                        price_euros: from_cents(price_cents.unwrap_or(0)),
                    },
                    pushed_at,
                    pushed_price,
                ))
            })
            .map_err(|error| format!("Uitschieters niet te lezen: {error}"))?;

        let mut candidates = Vec::new();
        for row in rows {
            let (candidate, pushed_at, pushed_price) =
                row.map_err(|error| format!("Uitschieters niet te lezen: {error}"))?;
            let worth_repeating = match (pushed_at, pushed_price) {
                (None, _) => true,
                // Nog eens tien procent lager dan bij de vorige melding is echt nieuws.
                (Some(_), Some(was)) => to_cents(candidate.price_euros) as f64 <= was as f64 * 0.9,
                (Some(_), None) => false,
            };
            if worth_repeating {
                candidates.push(candidate);
            }
        }
        Ok(candidates)
    }

    pub fn mark_pushed(&self, key: &str, price_euros: f64, now: i64) -> Result<(), String> {
        self.connection
            .execute(
                "UPDATE finding SET pushed_at = ?2, pushed_at_price = ?3 WHERE key = ?1",
                params![key, now, to_cents(price_euros)],
            )
            .map(|_| ())
            .map_err(|error| format!("Melding van {key} niet vast te leggen: {error}"))
    }

    // ------------------------------------------------- bronnen die ons tegenhouden

    /// Tot wanneer deze bron met rust gelaten wordt, als hij ons heeft tegengehouden.
    pub fn source_blocked_until(&self, source: &str) -> i64 {
        self.state(&format!("blocked_until:{source}"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }

    /// Legt vast dat een bron ons tegenhield, en hoe lang we wegblijven.
    ///
    /// Elke volgende keer verdubbelt de wachttijd, want als een kwartier niet hielp is een
    /// kwartier later opnieuw proberen alleen maar dieper graven. Bij een geslaagde ronde
    /// gaat de teller terug naar nul.
    pub fn note_source_blocked(&self, source: &str, now: i64) -> Result<i64, String> {
        let strikes: i64 = self
            .state(&format!("block_strikes:{source}"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let next = (strikes + 1).min(4);

        // 15, 30, 60, 120 minuten.
        let wait = 900 * 2i64.pow((next - 1) as u32);
        self.set_state(&format!("block_strikes:{source}"), &next.to_string())?;
        self.set_state(&format!("blocked_until:{source}"), &(now + wait).to_string())?;
        Ok(wait)
    }

    /// Hoe vaak deze bron ons al tegenhield. Bepaalt hoeveel lucht hij krijgt.
    pub fn source_strikes(&self, source: &str) -> i64 {
        self.state(&format!("block_strikes:{source}"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }

    pub fn note_source_healthy(&self, source: &str) -> Result<(), String> {
        if self.source_blocked_until(source) == 0
            && self.state(&format!("block_strikes:{source}")).is_none()
        {
            return Ok(());
        }
        self.connection
            .execute(
                "DELETE FROM app_state WHERE name IN (?1, ?2)",
                params![
                    format!("blocked_until:{source}"),
                    format!("block_strikes:{source}")
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("Blokkade van {source} niet op te heffen: {error}"))
    }

    // ------------------------------------------------------------------ het slot

    /// Pakt het slot als er geen ronde loopt. Geeft false als er al eentje bezig is.
    ///
    /// Een ronde die is omgevallen laat het slot staan; daarom vervalt het vanzelf na
    /// `stale_after`. Anders zou één klapper de wachter voorgoed stilzetten, en dat is
    /// precies de stille storing die dit systeem niet mag hebben.
    pub fn take_round_lock(&self, now: i64, stale_after: i64) -> Result<bool, String> {
        let running: Option<i64> = self
            .state("round_running_since")
            .and_then(|value| value.parse().ok());

        if let Some(since) = running {
            // Een stempel uit de toekomst — een klok die terugliep, een handmatige bewerking —
            // zou met een gewone vergelijking altijd "nog bezig" opleveren en de wachter
            // voorgoed stilzetten. Die telt daarom als vervallen.
            let age = now - since;
            if (0..stale_after).contains(&age) {
                return Ok(false);
            }
        }

        self.set_state("round_running_since", &now.to_string())?;
        Ok(true)
    }

    pub fn release_round_lock(&self) -> Result<(), String> {
        self.connection
            .execute(
                "DELETE FROM app_state WHERE name = 'round_running_since'",
                [],
            )
            .map(|_| ())
            .map_err(|error| format!("Slot niet los te laten: {error}"))
    }

    // -------------------------------------------------------------- hercontrole

    /// Actieve advertenties, langst niet gecontroleerd eerst. Afwezigheid in de
    /// zoekresultaten zegt niets — beide bronnen leveren alleen de nieuwste zestig — dus
    /// prijzen volgen en "weg" vaststellen gebeurt via de advertentie zelf.
    /// `fresh_since` is het moment waarvóór een vondst niet meer als vers geldt.
    pub fn due_for_recheck(&self, limit: usize, fresh_since: i64) -> Result<Vec<Listing>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT l.key, l.source, l.listing_id, l.title, l.url, l.description, l.location,
                        l.seller, l.condition, l.delivery, l.photo_count,
                        (SELECT price_cents FROM sighting p WHERE p.key = l.key
                         ORDER BY p.seen_at DESC LIMIT 1),
                        (SELECT asking_cents FROM sighting p WHERE p.key = l.key
                         ORDER BY p.seen_at DESC LIMIT 1)
                 FROM listing l
                 JOIN finding f ON f.key = l.key
                 WHERE l.gone_since IS NULL
                 -- Verse vondsten eerst. Bij een echt koopje wil je weten hoe snel hij weg
                 -- was, en dat is alleen te meten als je er in die eerste uren bovenop zit.
                 ORDER BY (f.became_a_find_at > ?2) DESC,
                          COALESCE(l.last_checked, 0) ASC,
                          l.last_seen ASC
                 LIMIT ?1",
            )
            .map_err(|error| format!("Hercontrolelijst niet op te vragen: {error}"))?;

        let rows = statement
            .query_map(params![limit as i64, fresh_since], |row| {
                Ok(Listing {
                    source: row.get(1)?,
                    listing_id: row.get(2)?,
                    title: row.get(3)?,
                    url: row.get(4)?,
                    description: row.get(5)?,
                    location: row.get(6)?,
                    seller: row.get(7)?,
                    condition: row.get(8)?,
                    delivery: delivery_from_name(&row.get::<_, String>(9)?),
                    photo_count: row.get::<_, i64>(10)? as usize,
                    price_euros: from_cents(row.get::<_, Option<i64>>(11)?.unwrap_or(0)),
                    asking_price_euros: from_cents(row.get::<_, Option<i64>>(12)?.unwrap_or(0)),
                    ..Listing::default()
                })
            })
            .map_err(|error| format!("Hercontrolelijst niet te lezen: {error}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Hercontrolelijst niet te lezen: {error}"))
    }

    /// De advertentie bestaat nog: tijdstempel bij, twijfel weg.
    pub fn note_still_there(&self, key: &str, now: i64) -> Result<(), String> {
        self.connection
            .execute(
                "UPDATE listing SET last_checked = ?2, gone_checks = 0, gone_since = NULL,
                     gone_reason = NULL
                 WHERE key = ?1",
                params![key, now],
            )
            .map(|_| ())
            .map_err(|error| format!("Hercontrole van {key} niet vast te leggen: {error}"))
    }

    /// Ondubbelzinnig weg (404 of 410). Pas de tweede keer op rij telt als verdwenen; één
    /// keer kan een hik zijn.
    pub fn note_gone(&self, key: &str, sold: bool, now: i64) -> Result<bool, String> {
        let reason = if sold { "sold" } else { "removed" };
        self.connection
            .execute(
                "UPDATE listing
                 SET last_checked = ?2,
                     gone_checks  = gone_checks + 1,
                     gone_since   = CASE WHEN gone_checks + 1 >= 2 THEN ?2 ELSE gone_since END,
                     gone_reason  = CASE WHEN gone_checks + 1 >= 2 THEN ?3 ELSE gone_reason END
                 WHERE key = ?1",
                params![key, now, reason],
            )
            .map_err(|error| format!("Verdwijning van {key} niet vast te leggen: {error}"))?;

        let gone: Option<i64> = self
            .connection
            .query_row("SELECT gone_since FROM listing WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| format!("Staat van {key} niet leesbaar: {error}"))?
            .flatten();
        Ok(gone.is_some())
    }

    // ---------------------------------------------------------------- wachtrij

    /// Zet openstaande verzoeken op opgepakt en geeft ze terug. Verzoeken die al te vaak
    /// zijn opgepakt worden eerst als mislukt afgesloten, zodat er een nieuw verzoek voor
    /// dezelfde advertentie kan komen.
    pub fn take_reviews(&self, now: i64) -> Result<Vec<ReviewRequest>, String> {
        self.connection
            .execute(
                "UPDATE review_request SET taken_at = NULL
                 WHERE answered_at IS NULL AND taken_at IS NOT NULL AND taken_at < ?1",
                params![now - STALE_AFTER_SECONDS],
            )
            .map_err(|error| format!("Vastgelopen verzoeken niet vrij te geven: {error}"))?;

        self.connection
            .execute(
                "UPDATE review_request
                 SET answered_at = ?1, failed_reason = ?2
                 WHERE answered_at IS NULL AND attempts >= ?3",
                params![
                    now,
                    format!("{MAX_REVIEW_ATTEMPTS} pogingen mislukt"),
                    MAX_REVIEW_ATTEMPTS
                ],
            )
            .map_err(|error| format!("Mislukte verzoeken niet af te sluiten: {error}"))?;

        // Alleen wat níét al in behandeling is. Zou `open_reviews` hier gebruikt worden, dan
        // kreeg een tweede aanroep hetzelfde verzoek opnieuw — en dan is de pogingengrens
        // binnen een seconde op, alleen omdat Hermes tweemaal gewekt werd.
        let fresh = self.reviews_matching("r.answered_at IS NULL AND r.taken_at IS NULL")?;
        for request in &fresh {
            self.connection
                .execute(
                    "UPDATE review_request SET taken_at = ?2, attempts = attempts + 1 WHERE id = ?1",
                    params![request.id, now],
                )
                .map_err(|error| format!("Verzoek {} niet op te pakken: {error}", request.id))?;
        }
        Ok(fresh)
    }

    /// Alles wat nog geen antwoord heeft, ook wat op dit moment in behandeling is.
    pub fn open_reviews(&self) -> Result<Vec<ReviewRequest>, String> {
        self.reviews_matching("r.answered_at IS NULL")
    }

    fn reviews_matching(&self, condition: &str) -> Result<Vec<ReviewRequest>, String> {
        let mut statement = self
            .connection
            .prepare(
                &format!(
                    "SELECT r.id, r.key, r.requested_at, r.attempts, l.title, l.url, l.source,
                            l.description, f.matched_as, f.queue_note,
                            (SELECT price_cents FROM sighting p WHERE p.key = r.key
                             ORDER BY p.seen_at DESC LIMIT 1),
                            f.reasons, f.warnings
                     FROM review_request r
                     JOIN listing l ON l.key = r.key
                     LEFT JOIN finding f ON f.key = r.key
                     WHERE {condition}
                     ORDER BY r.requested_at"
                ),
            )
            .map_err(|error| format!("Wachtrij niet op te vragen: {error}"))?;

        let rows = statement
            .query_map([], |row| {
                Ok(ReviewRequest {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    requested_at: row.get(2)?,
                    attempts: row.get(3)?,
                    title: row.get(4)?,
                    url: row.get(5)?,
                    source: row.get(6)?,
                    description: row.get(7)?,
                    matched_as: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                    queue_note: row.get(9)?,
                    price_euros: from_cents(row.get::<_, Option<i64>>(10)?.unwrap_or(0)),
                    reasons: json_list(row.get::<_, Option<String>>(11)?),
                    warnings: json_list(row.get::<_, Option<String>>(12)?),
                })
            })
            .map_err(|error| format!("Wachtrij niet te lezen: {error}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Wachtrij niet te lezen: {error}"))
    }

    pub fn answer_review(
        &self,
        id: i64,
        verdict: &str,
        recommendation: &str,
        now: i64,
    ) -> Result<(), String> {
        let changed = self
            .connection
            .execute(
                "UPDATE review_request
                 SET answered_at = ?2, verdict = ?3, recommendation = ?4
                 WHERE id = ?1 AND answered_at IS NULL",
                params![id, now, verdict, recommendation],
            )
            .map_err(|error| format!("Antwoord op verzoek {id} niet op te slaan: {error}"))?;
        if changed == 0 {
            return Err(format!("Verzoek {id} bestaat niet of is al beantwoord."));
        }
        Ok(())
    }

    /// Mislukt is een eindtoestand: ook `answered_at` gaat aan. Zo blijft "open" gelijk aan
    /// `answered_at IS NULL`, en laat de unieke index daarna een nieuw verzoek toe.
    pub fn fail_review(&self, id: i64, reason: &str, now: i64) -> Result<(), String> {
        let changed = self
            .connection
            .execute(
                "UPDATE review_request SET answered_at = ?2, failed_reason = ?3
                 WHERE id = ?1 AND answered_at IS NULL",
                params![id, now, reason],
            )
            .map_err(|error| format!("Verzoek {id} niet af te sluiten: {error}"))?;
        if changed == 0 {
            return Err(format!("Verzoek {id} bestaat niet of is al afgesloten."));
        }
        Ok(())
    }

    /// Voor de app: de knop. De unieke index houdt tweemaal drukken op één verzoek.
    pub fn request_review(&self, key: &str, now: i64) -> Result<i64, String> {
        self.connection
            .execute(
                "INSERT INTO review_request (key, requested_at) VALUES (?1, ?2)
                 ON CONFLICT DO NOTHING",
                params![key, now],
            )
            .map_err(|error| format!("Verzoek voor {key} niet aan te maken: {error}"))?;

        self.connection
            .query_row(
                "SELECT id FROM review_request WHERE key = ?1 AND answered_at IS NULL",
                params![key],
                |row| row.get(0),
            )
            .map_err(|error| format!("Verzoek voor {key} niet terug te vinden: {error}"))
    }

    /// Het vangnet onder het wekbericht: een verzoek dat al te lang wacht hoort zichtbaar te
    /// worden, want anders wacht je op een agent die het bericht nooit gezien heeft.
    pub fn reviews_waiting_longer_than(&self, seconds: i64, now: i64) -> Result<usize, String> {
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM review_request
                 WHERE answered_at IS NULL AND requested_at < ?1",
                params![now - seconds],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count as usize)
            .map_err(|error| format!("Wachtrij niet te tellen: {error}"))
    }

    // ------------------------------------------------------------------ lezen

    pub fn listing(&self, key: &str) -> Option<Listing> {
        self.connection
            .query_row(
                "SELECT l.source, l.listing_id, l.title, l.url, l.description, l.location,
                        l.seller, l.condition, l.delivery, l.photo_count,
                        (SELECT price_cents FROM sighting p WHERE p.key = l.key
                         ORDER BY p.seen_at DESC LIMIT 1),
                        (SELECT asking_cents FROM sighting p WHERE p.key = l.key
                         ORDER BY p.seen_at DESC LIMIT 1)
                 FROM listing l WHERE l.key = ?1",
                params![key],
                |row| {
                    Ok(Listing {
                        source: row.get(0)?,
                        listing_id: row.get(1)?,
                        title: row.get(2)?,
                        url: row.get(3)?,
                        description: row.get(4)?,
                        location: row.get(5)?,
                        seller: row.get(6)?,
                        condition: row.get(7)?,
                        delivery: delivery_from_name(&row.get::<_, String>(8)?),
                        photo_count: row.get::<_, i64>(9)? as usize,
                        price_euros: from_cents(row.get::<_, Option<i64>>(10)?.unwrap_or(0)),
                        asking_price_euros: from_cents(row.get::<_, Option<i64>>(11)?.unwrap_or(0)),
                        ..Listing::default()
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn count(&self, table: &str) -> i64 {
        self.connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap_or(0)
    }
}

fn json_list(stored: Option<String>) -> Vec<String> {
    stored
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn to_cents(euros: f64) -> i64 {
    (euros * 100.0).round() as i64
}

pub fn from_cents(cents: i64) -> f64 {
    cents as f64 / 100.0
}

fn delivery_name(delivery: &Delivery) -> &'static str {
    match delivery {
        Delivery::ShippingAvailable => "shipping",
        Delivery::PickupOnly => "pickup",
        Delivery::Unknown => "unknown",
    }
}

fn delivery_from_name(name: &str) -> Delivery {
    match name {
        "shipping" => Delivery::ShippingAvailable,
        "pickup" => Delivery::PickupOnly,
        _ => Delivery::Unknown,
    }
}

fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Clear => "clear",
        Confidence::NeedsReview => "review",
    }
}

impl FindingKind {
    pub fn name(&self) -> &'static str {
        match self {
            FindingKind::Card => "card",
            FindingKind::Part => "part",
            FindingKind::Unknown => "unknown",
        }
    }
}
