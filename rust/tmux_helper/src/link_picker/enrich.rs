//! Parallel `gh` enrichment with a shared deadline and on-disk cache.

use crate::link_picker::detect::{EnrichedTitle, GhState};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_VERSION: u32 = 1;
const TTL_SECONDS: u64 = 3600; // 1 hour

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheFile {
    pub version: u32,
    pub entries: HashMap<String, CacheEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheEntry {
    pub fetched_at: u64, // unix seconds
    pub title: String,
    pub state: GhState,
    pub author: Option<String>,
}

impl CacheFile {
    pub fn empty() -> Self {
        Self { version: CACHE_VERSION, entries: HashMap::new() }
    }

    pub fn path() -> Option<PathBuf> {
        let base = dirs::cache_dir()?;
        Some(base.join("rmux_helper").join("gh-links.json"))
    }

    /// Load from disk. Returns `CacheFile::empty()` on any failure (missing, corrupt, wrong version).
    pub fn load_or_reset() -> Self {
        let Some(path) = Self::path() else { return Self::empty() };
        let Ok(bytes) = std::fs::read(&path) else { return Self::empty() };
        let parsed: Result<CacheFile, _> = serde_json::from_slice(&bytes);
        match parsed {
            Ok(cf) if cf.version == CACHE_VERSION => cf,
            _ => {
                // Delete corrupt or wrong-version file silently.
                let _ = std::fs::remove_file(&path);
                Self::empty()
            }
        }
    }

    /// Atomic write via temp-file + rename.
    pub fn save(&self) -> Result<()> {
        let Some(path) = Self::path() else { return Ok(()) };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("create cache dir")?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&tmp, json).context("write cache tmp")?;
        std::fs::rename(&tmp, &path).context("rename cache tmp")?;
        Ok(())
    }

    /// Look up a cache hit for `canonical_url`. Returns `None` if missing or past TTL.
    pub fn lookup(&self, canonical_url: &str) -> Option<EnrichedTitle> {
        let entry = self.entries.get(canonical_url)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        if now.saturating_sub(entry.fetched_at) > TTL_SECONDS {
            return None;
        }
        Some(EnrichedTitle {
            title: entry.title.clone(),
            state: entry.state,
            author: entry.author.clone(),
        })
    }

    pub fn insert(&mut self, canonical_url: String, enrichment: &EnrichedTitle) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.entries.insert(
            canonical_url,
            CacheEntry {
                fetched_at: now,
                title: enrichment.title.clone(),
                state: enrichment.state,
                author: enrichment.author.clone(),
            },
        );
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn roundtrip_empty_cache() {
        let cf = CacheFile::empty();
        let json = serde_json::to_string(&cf).unwrap();
        let back: CacheFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, CACHE_VERSION);
        assert!(back.entries.is_empty());
    }

    #[test]
    fn lookup_respects_ttl() {
        let mut cf = CacheFile::empty();
        let url = "https://github.com/a/b/pull/1".to_string();
        let enrichment = EnrichedTitle {
            title: "test".into(),
            state: GhState::Open,
            author: Some("alice".into()),
        };
        cf.insert(url.clone(), &enrichment);
        assert!(cf.lookup(&url).is_some());

        // Force-expire by mutating fetched_at
        cf.entries.get_mut(&url).unwrap().fetched_at = 0;
        assert!(cf.lookup(&url).is_none());
    }

    #[test]
    fn version_mismatch_resets() {
        let bad = r#"{"version":999,"entries":{}}"#;
        let parsed: Result<CacheFile, _> = serde_json::from_str(bad);
        assert!(parsed.is_ok());
        assert_ne!(parsed.unwrap().version, CACHE_VERSION);
    }
}
