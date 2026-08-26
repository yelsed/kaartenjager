//! HTTP with a cookie jar, a politeness gap and bounded retries.

use serde_json::Value;
use std::fmt;
use std::time::{Duration, Instant};

/// Why a request did not produce a page.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// The server said the resource is not there (404 or 410). Unambiguous.
    Gone,
    /// De bron houdt ons tegen (403 of 429). Doorgaan maakt het alleen erger: dertien
    /// zoekopdrachten achter elkaar tegen een dichte deur is precies hoe je een tijdelijke
    /// rem in een lange blokkade verandert.
    Blocked(String),
    /// Anything else: a timeout, a rate limit, a server fault, a broken connection.
    Other(String),
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Gone => write!(formatter, "de pagina bestaat niet meer"),
            Failure::Blocked(reason) => write!(formatter, "{reason}"),
            Failure::Other(reason) => write!(formatter, "{reason}"),
        }
    }
}

/// A real Chrome string. Both endpoints answer a default agent string with a challenge page.
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const ACCEPT_LANGUAGE: &str = "nl-NL,nl;q=0.9,en;q=0.8";

pub struct HttpClient {
    agent: ureq::Agent,
    delay: Duration,
    timeout: Duration,
    max_attempts: u32,
    last_request_finished: Option<Instant>,
    pub requests_made: u32,
}

impl HttpClient {
    pub fn new(delay_ms: u64) -> Self {
        let timeout = Duration::from_secs(20);
        HttpClient {
            agent: build_agent(timeout),
            delay: Duration::from_millis(delay_ms),
            timeout,
            max_attempts: 3,
            last_request_finished: None,
            requests_made: 0,
        }
    }

    /// De tussenruimte bijstellen. Een bron die ons eerder tegenhield krijgt meer lucht: hard
    /// terugkomen op hetzelfde tempo is hoe een korte rem een lange blokkade wordt.
    pub fn set_delay(&mut self, delay_ms: u64) {
        self.delay = Duration::from_millis(delay_ms);
    }

    /// Rebuilding the agent is the surest way to drop every cookie, and it happens at most
    /// once per run.
    pub fn clear_cookies(&mut self) {
        self.agent = build_agent(self.timeout);
    }

    pub fn get_text(&mut self, url: &str) -> Result<String, String> {
        self.get_page(url).map_err(|failure| failure.to_string())
    }

    /// Like `get_text`, but keeps "this listing no longer exists" apart from "something went
    /// wrong". A recheck may only conclude a listing is gone on the first; a network fault or
    /// a rate limit must never read as "sold".
    pub fn get_page(&mut self, url: &str) -> Result<String, Failure> {
        self.get_with_headers(url, None, "text/html,application/xhtml+xml,*/*")
    }

    pub fn get_json(&mut self, url: &str, referer: Option<&str>) -> Result<Value, String> {
        self.get_json_detailed(url, referer)
            .map_err(|failure| failure.to_string())
    }

    /// Zoals `get_json`, maar houdt "tegengehouden" apart van "ging mis".
    pub fn get_json_detailed(
        &mut self,
        url: &str,
        referer: Option<&str>,
    ) -> Result<Value, Failure> {
        let body = self.get_with_headers(url, referer, "application/json, text/plain, */*")?;
        serde_json::from_str(&body)
            .map_err(|error| Failure::Other(format!("{url} gaf geen bruikbare JSON terug: {error}")))
    }

    fn get_with_headers(
        &mut self,
        url: &str,
        referer: Option<&str>,
        accept: &str,
    ) -> Result<String, Failure> {
        let mut last_failure = String::new();

        for attempt in 1..=self.max_attempts {
            self.wait_for_politeness_window();

            let mut request = self
                .agent
                .get(url)
                .header("User-Agent", BROWSER_USER_AGENT)
                .header("Accept-Language", ACCEPT_LANGUAGE)
                .header("Accept", accept);
            if let Some(referer) = referer {
                request = request.header("Referer", referer);
            }

            let outcome = request.call();
            self.last_request_finished = Some(Instant::now());
            self.requests_made += 1;

            match outcome {
                Ok(mut response) => {
                    return response
                        .body_mut()
                        .read_to_string()
                        .map_err(|error| {
                            Failure::Other(format!("{url}: antwoord onleesbaar: {error}"))
                        });
                }
                Err(ureq::Error::StatusCode(code)) => {
                    // Gone means gone: no amount of retrying brings a removed listing back,
                    // and the caller is allowed to act on it.
                    if matches!(code, 404 | 410) {
                        return Err(Failure::Gone);
                    }
                    // Tegengehouden. Niet opnieuw proberen en het meteen zeggen, zodat de
                    // aanroeper deze bron voor even met rust kan laten.
                    if matches!(code, 403 | 429) {
                        return Err(Failure::Blocked(format!("{url}: HTTP {code}")));
                    }
                    last_failure = format!("{url}: HTTP {code}");
                    // Server faults may pass; anything else will not.
                    if !matches!(code, 408 | 500 | 502 | 503 | 504) {
                        break;
                    }
                }
                Err(error) => last_failure = format!("{url}: {error}"),
            }

            if attempt < self.max_attempts {
                std::thread::sleep(Duration::from_secs(2 * attempt as u64));
            }
        }

        Err(Failure::Other(format!(
            "{last_failure} (na {} pogingen)",
            self.max_attempts
        )))
    }

    /// Keeps a gap between requests so a round never looks like a scraper burst.
    fn wait_for_politeness_window(&self) {
        let Some(finished) = self.last_request_finished else {
            return;
        };
        let elapsed = finished.elapsed();
        if elapsed < self.delay {
            std::thread::sleep(self.delay - elapsed);
        }
    }
}

fn build_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// Percent-encodes a search term. Small enough that a crate for it would be overkill.
pub fn url_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push('+'),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}
