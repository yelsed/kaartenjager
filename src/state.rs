//! Remembers which listings were already reported, so a run stays quiet about them.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredHistory {
    #[serde(default)]
    first_seen: BTreeMap<String, i64>,
}

pub struct History {
    path: PathBuf,
    forget_after_seconds: i64,
    first_seen: BTreeMap<String, i64>,
    now: i64,
}

impl History {
    /// A corrupt file is a warning, not a crash: the run continues with an empty history,
    /// which costs one noisy round rather than every round.
    pub fn load(path: &Path, forget_after_days: i64, now: i64) -> (Self, Option<String>) {
        let mut warning = None;
        let mut first_seen = BTreeMap::new();

        if path.is_file() {
            match std::fs::read_to_string(path) {
                Ok(text) => match serde_json::from_str::<StoredHistory>(&text) {
                    Ok(stored) => first_seen = stored.first_seen,
                    Err(error) => {
                        warning = Some(format!(
                            "{} kon niet gelezen worden ({error}); begonnen met een lege geschiedenis",
                            path.display()
                        ))
                    }
                },
                Err(error) => {
                    warning = Some(format!(
                        "{} kon niet geopend worden ({error}); begonnen met een lege geschiedenis",
                        path.display()
                    ))
                }
            }
        }

        (
            History {
                path: path.to_path_buf(),
                forget_after_seconds: forget_after_days * SECONDS_PER_DAY,
                first_seen,
                now,
            },
            warning,
        )
    }

    pub fn is_new(&self, key: &str) -> bool {
        !self.first_seen.contains_key(key)
    }

    pub fn remember(&mut self, key: &str) {
        self.first_seen.entry(key.to_string()).or_insert(self.now);
    }

    pub fn len(&self) -> usize {
        self.first_seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.first_seen.is_empty()
    }

    /// Written to a temporary file and moved into place, so a run killed halfway cannot leave
    /// a truncated file that makes every listing look new again.
    pub fn save(&mut self) -> std::io::Result<()> {
        self.forget_old_entries();

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let stored = StoredHistory {
            first_seen: self.first_seen.clone(),
        };
        let text = serde_json::to_string(&stored)?;

        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, &self.path)
    }

    fn forget_old_entries(&mut self) {
        let cutoff = self.now - self.forget_after_seconds;
        self.first_seen.retain(|_, first_seen| *first_seen >= cutoff);
    }
}
