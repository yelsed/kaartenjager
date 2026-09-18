//! Een echte Chromium, aangestuurd over de DevTools-pijp.
//!
//! Waarom een browser: Vinted beantwoordt `/api/v2/catalog/items` sinds september 2026 met 403.
//! De gewone cataloguspagina werkt wel, maar die is pas compleet als de pagina echt geladen is.
//!
//! Waarom een pijp en geen poort: `--remote-debugging-pipe` praat over bestandsbeschrijving 3 en 4
//! in plaats van over TCP. Dat scheelt een WebSocket-bibliotheek, en er staat niets open waar een
//! ander bij kan. De berichten zijn JSON, afgesloten met een nulbyte -- niet met een regeleinde.
//! Wie hier op regels leest blijft hangen op het eerste antwoord, en dat merk je pas als de ronde
//! na een kwartier op zijn slot stukloopt.

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// De namen waaronder Chromium op de verschillende distributies staat. Arch noemt hem `chromium`,
/// Debian en Ubuntu soms `chromium-browser`, en op een GitHub-loper staat alleen Chrome.
const EXECUTABLE_NAMES: [&str; 5] = [
    "chromium",
    "chromium-browser",
    "google-chrome-stable",
    "google-chrome",
    "chrome",
];

/// Eén lijst, zodat een vlag die Chromium ooit niet meer accepteert één regel is om te vinden.
///
/// `--headless=new` staat er voluit: de oude headless is sinds Chrome 132 weg, en een distributie
/// die nog een oudere bouw meelevert moet daar niet stilletjes in terugvallen.
/// `--user-data-dir` is geen netheid maar een eis -- Chromium weigert remote debugging op het
/// standaardprofiel met "DevTools remote debugging requires a non-default data directory".
const FLAGS: [&str; 13] = [
    "--headless=new",
    "--remote-debugging-pipe",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-component-update",
    "--disable-background-networking",
    "--disable-sync",
    "--disable-breakpad",
    "--metrics-recording-only",
    "--disable-dev-shm-usage",
    "--disable-gpu",
    "--lang=nl-NL",
];

/// Het raster laadt lui: een klein venster levert minder rijen op dan er zijn.
const WINDOW: &str = "--window-size=1280,2400";

/// Vast, want niemand stelt dit ooit bij. Duurt de handdruk langer, dan is er iets grondig mis en
/// is wachten zinloos.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Een enkel bericht met de hele pagina erin is al gauw negen megabyte. Hierboven gaat er iets mis
/// en is stoppen beter dan het geheugen laten vollopen.
const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;

pub struct Page {
    /// Leeg als er geen antwoord op de hoofdpagina te zien was. Ontbreken is geen storing: het
    /// betekent alleen dat we de uitkomst niet aan een code kunnen ophangen.
    pub status: Option<u16>,
    pub html: String,
}

pub struct Browser {
    child: Child,
    to_browser: File,
    from_browser: File,
    session: String,
    target: String,
    next_id: u64,
    session_product: String,
    buffer: Vec<u8>,
    events: Vec<Value>,
    /// Na een pijpfout is elk volgend bericht zinloos. Dan liever één duidelijke fout dan een
    /// reeks vage.
    broken: bool,
}

/// Zoekt het uitvoerbare bestand. Een ingevuld pad wint altijd, ook als het niet bestaat -- dan
/// hoort de melding over dát pad te gaan en niet over een toevallige andere browser.
pub fn find_executable(configured: &str) -> Result<PathBuf, String> {
    if !configured.trim().is_empty() {
        let path = PathBuf::from(configured.trim());
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!("browser.executable wijst naar {configured}, en daar staat niets"))
        };
    }
    if let Some(from_environment) = std::env::var_os("KAARTENJAGER_BROWSER") {
        let path = PathBuf::from(&from_environment);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "KAARTENJAGER_BROWSER wijst naar {}, en daar staat niets",
            path.display()
        ));
    }
    for name in EXECUTABLE_NAMES {
        if let Some(found) = in_path(name) {
            return Ok(found);
        }
    }
    Err(format!(
        "geen Chromium gevonden (gezocht naar {}). Installeer er een, of zet browser.executable",
        EXECUTABLE_NAMES.join(", ")
    ))
}

fn in_path(name: &str) -> Option<PathBuf> {
    let directories = std::env::var_os("PATH")?;
    std::env::split_paths(&directories)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

impl Browser {
    /// Start Chromium en doet de handdruk. Lukt dat niet, dan komt er een zin uit die zegt wát er
    /// misging -- de aanroeper meldt Vinted als onbeschikbaar en laat Marktplaats doorlopen.
    pub fn launch(
        executable: &Path,
        profile: &Path,
        no_sandbox: bool,
    ) -> Result<Browser, String> {
        std::fs::create_dir_all(profile)
            .map_err(|error| format!("profielmap {} niet aan te maken: {error}", profile.display()))?;
        // Een Chromium die is omgevallen laat dit slot staan, en de volgende start weigert dan.
        let _ = std::fs::remove_file(profile.join("SingletonLock"));

        let (parent_writes, child_reads) = make_pipe()?;
        let (child_writes, parent_reads) = make_pipe()?;

        let mut command = Command::new(executable);
        command.args(FLAGS);
        command.arg(WINDOW);
        command.arg(format!("--user-data-dir={}", profile.display()));
        if no_sandbox {
            command.arg("--no-sandbox");
        }
        command.arg("about:blank");
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        // Niet weggooien: hier staat "No usable sandbox!" in, en dat is precies wat iemand in een
        // container moet lezen.
        command.stderr(Stdio::piped());

        // Alles hieronder draait tussen fork en exec. Daar mag niets gebeuren dat kan wachten of
        // geheugen vraagt; `dup2` en `prctl` mogen het allebei.
        unsafe {
            command.pre_exec(move || {
                // Eerst allebei hoog wegzetten, dan pas op 3 en 4. Rechtstreeks `dup2` naar 3 gaat
                // mis zodra de pijp toevallig al op 3 staat: `dup2(3, 3)` doet per definitie niets
                // en laat `O_CLOEXEC` dus staan, waarna Chromium klaagt dat de pijp niet open is.
                // `F_DUPFD` -- en níét `F_DUPFD_CLOEXEC` -- geeft een kopie zonder die vlag.
                let high_read = libc::fcntl(child_reads, libc::F_DUPFD, 10);
                let high_write = libc::fcntl(child_writes, libc::F_DUPFD, 10);
                if high_read < 0 || high_write < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(high_read, 3) < 0 || libc::dup2(high_write, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::fcntl(3, libc::F_SETFD, 0);
                libc::fcntl(4, libc::F_SETFD, 0);
                // Wordt kaartenjager hardhandig afgebroken -- en dat is precies wat een wachthond
                // met een vastgelopen ronde doet -- dan gaat Chromium mee. Zonder dit blijft er per
                // afgebroken ronde een proces van een paar honderd megabyte achter, en een cron van
                // vijf minuten draait er bijna driehonderd per dag.
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }

        let child = command
            .spawn()
            .map_err(|error| format!("{} start niet: {error}", executable.display()))?;

        // De kinderkanten horen bij het kind. Blijven ze hier open, dan ziet deze kant nooit een
        // einde als Chromium omvalt.
        close_fd(child_reads);
        close_fd(child_writes);

        let mut browser = Browser {
            child,
            to_browser: unsafe { File::from_raw_fd(parent_writes) },
            from_browser: unsafe { File::from_raw_fd(parent_reads) },
            session: String::new(),
            target: String::new(),
            next_id: 0,
            session_product: String::new(),
            buffer: Vec::with_capacity(1 << 20),
            events: Vec::new(),
            broken: false,
        };

        // `Browser.getVersion` is de handdruk. Hij bewijst dat de pijp goed staat en levert meteen
        // de versie die `doctor` laat zien -- bij een volgende verbouwing van Chromium staat er dan
        // een versienummer naast de klacht in plaats van een gok.
        let version = browser.call("Browser.getVersion", json!({}), None, HANDSHAKE_TIMEOUT);
        let version = match version {
            Ok(value) => value,
            Err(error) => {
                let tail = browser.stderr_tail();
                browser.shutdown();
                return Err(if tail.is_empty() {
                    format!("Chromium antwoordt niet: {error}")
                } else {
                    format!("Chromium antwoordt niet: {error} ({tail})")
                });
            }
        };
        let product = version
            .get("product")
            .and_then(Value::as_str)
            .unwrap_or("onbekende versie")
            .to_string();

        if let Err(error) = browser.open_target() {
            let tail = browser.stderr_tail();
            browser.shutdown();
            return Err(if tail.is_empty() {
                error
            } else {
                format!("{error} ({tail})")
            });
        }

        browser.session_product = product;
        Ok(browser)
    }

    fn open_target(&mut self) -> Result<(), String> {
        let created = self.call(
            "Target.createTarget",
            json!({"url": "about:blank"}),
            None,
            HANDSHAKE_TIMEOUT,
        )?;
        self.target = created
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or("Chromium gaf geen targetId terug")?
            .to_string();

        let attached = self.call(
            "Target.attachToTarget",
            json!({"targetId": self.target, "flatten": true}),
            None,
            HANDSHAKE_TIMEOUT,
        )?;
        self.session = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or("Chromium gaf geen sessionId terug")?
            .to_string();

        let session = self.session.clone();
        self.call("Page.enable", json!({}), Some(&session), HANDSHAKE_TIMEOUT)?;
        self.call("Network.enable", json!({}), Some(&session), HANDSHAKE_TIMEOUT)?;
        Ok(())
    }

    /// Laadt één pagina en geeft de opmaak terug zoals de browser hem uiteindelijk heeft staan.
    pub fn load(&mut self, url: &str, timeout: Duration) -> Result<Page, String> {
        let deadline = Instant::now() + timeout;
        let session = self.session.clone();
        self.events.clear();

        let navigated = self.call("Page.navigate", json!({"url": url}), Some(&session), timeout)?;
        if let Some(problem) = navigated.get("errorText").and_then(Value::as_str) {
            return Err(format!("{url}: {problem}"));
        }
        let frame = navigated
            .get("frameId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        self.wait_for_load(&session, deadline)
            .map_err(|error| format!("{url}: {error}"))?;

        let status = self.document_status(&session, &frame);

        let evaluated = self.call(
            "Runtime.evaluate",
            json!({
                "expression": "document.documentElement.outerHTML",
                "returnByValue": true,
            }),
            Some(&session),
            remaining(deadline).max(Duration::from_secs(5)),
        )?;
        let html = evaluated
            .pointer("/result/value")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{url}: de browser gaf geen opmaak terug"))?
            .to_string();

        Ok(Page { status, html })
    }

    fn wait_for_load(&mut self, session: &str, deadline: Instant) -> Result<(), String> {
        loop {
            if self.events.iter().any(|event| {
                event.get("method").and_then(Value::as_str) == Some("Page.loadEventFired")
                    && event.get("sessionId").and_then(Value::as_str) == Some(session)
            }) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("de pagina was niet op tijd geladen".to_string());
            }
            let frame = self.read_frame(deadline)?;
            self.file(frame);
        }
    }

    /// De statuscode van het hoofddocument, uit de gebeurtenissen die tijdens het laden langskwamen.
    fn document_status(&self, session: &str, frame: &str) -> Option<u16> {
        self.events
            .iter()
            .rev()
            .find(|event| {
                event.get("method").and_then(Value::as_str) == Some("Network.responseReceived")
                    && event.get("sessionId").and_then(Value::as_str) == Some(session)
                    && event.pointer("/params/type").and_then(Value::as_str) == Some("Document")
                    && (frame.is_empty()
                        || event.pointer("/params/frameId").and_then(Value::as_str) == Some(frame))
            })
            .and_then(|event| event.pointer("/params/response/status"))
            .and_then(Value::as_u64)
            .map(|status| status as u16)
    }

    /// De versie die de handdruk opleverde. `doctor` zet hem in het rapport.
    pub fn product(&self) -> &str {
        &self.session_product
    }

    fn call(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, String> {
        if self.broken {
            return Err("de verbinding met de browser is stuk".to_string());
        }
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({"id": id, "method": method, "params": params});
        if let Some(session) = session {
            message["sessionId"] = Value::String(session.to_string());
        }

        let mut raw = serde_json::to_vec(&message)
            .map_err(|error| format!("{method} niet op te schrijven: {error}"))?;
        raw.push(0);
        if let Err(error) = self.to_browser.write_all(&raw).and_then(|()| self.to_browser.flush()) {
            self.broken = true;
            return Err(format!("{method} niet te versturen: {error}"));
        }

        let deadline = Instant::now() + timeout;
        loop {
            let frame = self.read_frame(deadline)?;
            if frame.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(problem) = frame.pointer("/error/message").and_then(Value::as_str) {
                    return Err(format!("{method}: {problem}"));
                }
                return Ok(frame.get("result").cloned().unwrap_or(Value::Null));
            }
            self.file(frame);
        }
    }

    /// Gebeurtenissen komen tussen de antwoorden door binnen. Bewaren, niet weggooien: de
    /// statuscode van het hoofddocument zit erin.
    fn file(&mut self, frame: Value) {
        if frame.get("method").is_some() {
            // Ruim genoeg voor één paginalading, en het loopt niet vol als er een tijdje niets
            // wordt opgehaald.
            if self.events.len() < 4096 {
                self.events.push(frame);
            }
        }
    }

    /// Leest tot de eerstvolgende nulbyte. Berichten zijn groot -- de opmaak van één
    /// cataloguspagina is negen megabyte aan ontsnapte JSON -- en Chromium blijft hangen zolang
    /// deze kant niet leegtrekt. Dus doorlezen, en nooit per regel.
    fn read_frame(&mut self, deadline: Instant) -> Result<Value, String> {
        loop {
            if let Some(end) = self.buffer.iter().position(|byte| *byte == 0) {
                let frame: Vec<u8> = self.buffer.drain(..=end).collect();
                return serde_json::from_slice(&frame[..end])
                    .map_err(|error| format!("de browser stuurde iets onleesbaars: {error}"));
            }
            if self.buffer.len() > MAX_FRAME_BYTES {
                self.broken = true;
                return Err("de browser stuurde een bericht dat nergens op slaat".to_string());
            }
            self.fill(deadline)?;
        }
    }

    fn fill(&mut self, deadline: Instant) -> Result<(), String> {
        let left = remaining(deadline);
        if left.is_zero() {
            return Err("de browser antwoordde niet op tijd".to_string());
        }
        if !readable(&self.from_browser, left)? {
            return Err("de browser antwoordde niet op tijd".to_string());
        }
        let mut chunk = [0u8; 64 * 1024];
        match self.from_browser.read(&mut chunk) {
            Ok(0) => {
                self.broken = true;
                Err("de browser is ermee opgehouden".to_string())
            }
            Ok(read) => {
                self.buffer.extend_from_slice(&chunk[..read]);
                Ok(())
            }
            Err(error) => {
                self.broken = true;
                Err(format!("niets meer van de browser te lezen: {error}"))
            }
        }
    }

    fn stderr_tail(&mut self) -> String {
        let Some(mut stream) = self.child.stderr.take() else {
            return String::new();
        };
        let mut text = String::new();
        let _ = stream.read_to_string(&mut text);
        text.lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .chars()
            .take(200)
            .collect()
    }

    fn shutdown(&mut self) {
        if !self.broken {
            let _ = self.call("Browser.close", json!({}), None, Duration::from_secs(5));
        }
        // De schrijfkant sluiten is het tweede net: Chromium stopt vanzelf als de pijp dichtgaat.
        let _ = self.to_browser.flush();
        let stop = Instant::now() + Duration::from_secs(5);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < stop => std::thread::sleep(Duration::from_millis(50)),
                _ => break,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// Wacht tot er iets te lezen valt, of tot de tijd om is. Zonder dit zou een browser die niets
/// meer zegt de hele ronde laten hangen tot het rondeslot verloopt.
fn readable(file: &File, within: Duration) -> Result<bool, String> {
    use std::os::fd::AsRawFd;
    let mut poll = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let milliseconds = within.as_millis().min(i32::MAX as u128) as i32;
    let ready = unsafe { libc::poll(&mut poll, 1, milliseconds) };
    if ready < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(format!("wachten op de browser mislukte: {error}"));
    }
    Ok(ready > 0)
}

/// Beide kanten met `O_CLOEXEC`, want alles boven bestandsbeschrijving 2 hoort een `exec` niet te
/// overleven. `dup2` haalt die vlag er in het kind weer af, en juist die twee moeten wél mee.
fn make_pipe() -> Result<(RawFd, RawFd), String> {
    let mut ends = [0 as RawFd; 2];
    let made = unsafe { libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC) };
    if made < 0 {
        return Err(format!(
            "pijp naar de browser niet aan te leggen: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok((ends[1], ends[0]))
}

fn close_fd(fd: RawFd) {
    unsafe { libc::close(fd) };
}

/// Het standaardpad voor het browserprofiel, naast de database.
pub fn default_profile() -> PathBuf {
    crate::config::home_directory()
        .map(|home| home.join(".local/share/kaartenjager/chromium-profiel"))
        .unwrap_or_else(|| std::env::temp_dir().join("kaartenjager-chromium"))
}

/// Gooit het profiel weg. Alleen na een controlepagina: een vergiftigd koekje is het enige geval
/// waarin schoon beginnen helpt.
pub fn forget_profile(profile: &Path) {
    let _ = std::fs::remove_dir_all(profile);
}
