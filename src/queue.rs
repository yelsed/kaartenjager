//! The handoff to layer two: append-only, and emptied only once layer two says it is done.

use crate::listing::Finding;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Queue {
    pending: PathBuf,
    taken: PathBuf,
}

impl Queue {
    pub fn new(directory: &Path) -> Self {
        Queue {
            pending: directory.join("queue.jsonl"),
            taken: directory.join("queue.taken.jsonl"),
        }
    }

    /// Appending a whole line is atomic on Linux, so a run killed mid-write cannot leave
    /// half a record behind.
    pub fn push(&self, finding: &Finding) -> std::io::Result<()> {
        if let Some(parent) = self.pending.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(finding)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.pending)?;
        writeln!(file, "{line}")
    }

    /// Moves everything pending into the taken file and returns the lot. Layer two crashing
    /// after this leaves the work in place for the next round rather than losing it.
    pub fn take(&self) -> std::io::Result<Vec<Finding>> {
        let mut all = read_findings(&self.taken)?;
        let fresh = read_findings(&self.pending)?;

        if fresh.is_empty() {
            return Ok(all);
        }

        all.extend(fresh);
        write_findings(&self.taken, &all)?;
        if self.pending.exists() {
            std::fs::remove_file(&self.pending)?;
        }
        Ok(all)
    }

    pub fn peek(&self) -> std::io::Result<Vec<Finding>> {
        let mut all = read_findings(&self.taken)?;
        all.extend(read_findings(&self.pending)?);
        Ok(all)
    }

    pub fn mark_done(&self) -> std::io::Result<usize> {
        let taken = read_findings(&self.taken)?;
        if self.taken.exists() {
            std::fs::remove_file(&self.taken)?;
        }
        Ok(taken.len())
    }
}

fn read_findings(path: &Path) -> std::io::Result<Vec<Finding>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path)?;
    let mut findings = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // One unreadable line must not cost the rest of the queue.
        if let Ok(finding) = serde_json::from_str::<Finding>(line) {
            findings.push(finding);
        }
    }
    Ok(findings)
}

fn write_findings(path: &Path, findings: &[Finding]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for finding in findings {
        text.push_str(&serde_json::to_string(finding)?);
        text.push('\n');
    }
    let temporary = path.with_extension("jsonl.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)
}
