//! My Activity: an append-only JSON-lines log of what PenCleaner did. One line per
//! action, so a crash mid-write can only lose that line, and old entries never change.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Unix seconds.
    pub at: i64,
    /// Which module did it: "cleanup", "clutter", "applications", "spacelens", "performance".
    pub module: String,
    /// What happened, for people: "Cleaned junk", "Ran maintenance".
    pub action: String,
    /// Space freed or moved to the Trash.
    pub bytes: u64,
    pub items: u32,
    pub detail: Option<String>,
}

impl Event {
    pub fn now(module: &str, action: &str, bytes: u64, items: u32, detail: Option<String>) -> Self {
        let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        Event { at, module: module.into(), action: action.into(), bytes, items, detail }
    }
}

pub struct Log {
    path: PathBuf,
}

impl Log {
    pub fn new(dir: &Path) -> Self {
        Log { path: dir.join("activity.jsonl") }
    }

    pub fn append(&self, e: &Event) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut f = OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{}", serde_json::to_string(e).map_err(std::io::Error::other)?)
    }

    /// Oldest first. Unreadable lines are skipped rather than failing the whole history.
    pub fn load(&self) -> Vec<Event> {
        let Ok(f) = fs::File::open(&self.path) else { return vec![] };
        BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str(&l).ok()).collect()
    }

    pub fn clear(&self) -> std::io::Result<()> {
        match fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_loads_and_survives_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::new(&dir.path().join("nested"));
        assert!(log.load().is_empty());
        let a = Event::now("cleanup", "Cleaned junk", 1_000, 3, Some("system".into()));
        let b = Event::now("performance", "Ran maintenance", 0, 2, None);
        log.append(&a).unwrap();
        fs::OpenOptions::new().append(true).open(dir.path().join("nested/activity.jsonl")).unwrap().write_all(b"{oops\n").unwrap();
        log.append(&b).unwrap();
        assert_eq!(log.load(), vec![a, b]);
        log.clear().unwrap();
        assert!(log.load().is_empty());
        log.clear().unwrap();
    }
}
