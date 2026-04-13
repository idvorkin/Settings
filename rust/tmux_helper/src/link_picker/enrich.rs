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

use tokio::process::Command;

#[derive(Debug, Deserialize)]
struct GhPrJson {
    title: String,
    state: String,
    #[serde(default)]
    author: Option<GhUser>,
    #[serde(rename = "isDraft", default)]
    is_draft: bool,
}

#[derive(Debug, Deserialize)]
struct GhIssueJson {
    title: String,
    state: String,
    #[serde(default)]
    author: Option<GhUser>,
}

#[derive(Debug, Deserialize)]
struct GhUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GhCommitJson {
    title: String,
    #[serde(default)]
    author: Option<String>,
}

/// Parse a GitHub canonical URL of the form `https://github.com/OWNER/REPO/(pull|issues|commit)/ID`.
pub(crate) fn parse_gh_target(url: &str) -> Option<(String, String, GhKind, String)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.splitn(4, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    let kind_str = parts.next()?;
    let id = parts.next()?.to_string();
    let kind = match kind_str {
        "pull" => GhKind::Pr,
        "issues" => GhKind::Issue,
        "commit" => GhKind::Commit,
        _ => return None,
    };
    Some((owner, repo, kind, id))
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum GhKind { Pr, Issue, Commit }

/// Call `gh` for one canonical URL. Returns `Ok(None)` on any recoverable failure.
pub(crate) async fn call_gh(canonical_url: &str) -> Result<Option<EnrichedTitle>> {
    let Some((owner, repo, kind, id)) = parse_gh_target(canonical_url) else {
        return Ok(None);
    };
    let owner_repo = format!("{owner}/{repo}");

    let output = match kind {
        GhKind::Pr => Command::new("gh")
            .args(["pr", "view", &id, "-R", &owner_repo, "--json", "title,state,author,isDraft"])
            .output()
            .await,
        GhKind::Issue => Command::new("gh")
            .args(["issue", "view", &id, "-R", &owner_repo, "--json", "title,state,author"])
            .output()
            .await,
        GhKind::Commit => Command::new("gh")
            .args([
                "api",
                &format!("repos/{owner_repo}/commits/{id}"),
                "--jq",
                "{title: (.commit.message | split(\"\\n\")[0]), author: .commit.author.name}",
            ])
            .output()
            .await,
    };

    let output = match output {
        Ok(o) => o,
        Err(_) => return Ok(None), // gh not on PATH
    };
    if !output.status.success() {
        return Ok(None); // 404, auth failure, etc.
    }

    match kind {
        GhKind::Pr => {
            let Ok(p) = serde_json::from_slice::<GhPrJson>(&output.stdout) else {
                return Ok(None);
            };
            let state = if p.is_draft {
                GhState::Draft
            } else {
                match p.state.as_str() {
                    "OPEN" => GhState::Open,
                    "MERGED" => GhState::MergedPr,
                    "CLOSED" => GhState::Closed,
                    _ => GhState::Open,
                }
            };
            Ok(Some(EnrichedTitle {
                title: p.title,
                state,
                author: p.author.map(|u| u.login),
            }))
        }
        GhKind::Issue => {
            let Ok(i) = serde_json::from_slice::<GhIssueJson>(&output.stdout) else {
                return Ok(None);
            };
            let state = match i.state.as_str() {
                "OPEN" => GhState::Open,
                "CLOSED" => GhState::Closed,
                _ => GhState::Open,
            };
            Ok(Some(EnrichedTitle {
                title: i.title,
                state,
                author: i.author.map(|u| u.login),
            }))
        }
        GhKind::Commit => {
            let Ok(c) = serde_json::from_slice::<GhCommitJson>(&output.stdout) else {
                return Ok(None);
            };
            Ok(Some(EnrichedTitle {
                title: c.title,
                state: GhState::Commit,
                author: c.author,
            }))
        }
    }
}

#[cfg(test)]
mod gh_call_tests {
    use super::*;

    #[test]
    fn parse_gh_target_pr() {
        let (o, r, k, id) = parse_gh_target("https://github.com/a/b/pull/42").unwrap();
        assert_eq!((o.as_str(), r.as_str(), id.as_str()), ("a", "b", "42"));
        assert!(matches!(k, GhKind::Pr));
    }

    #[test]
    fn parse_gh_target_rejects_non_gh_host() {
        assert!(parse_gh_target("https://gitlab.com/a/b/pull/1").is_none());
    }

    #[test]
    fn parse_gh_target_rejects_repo_home() {
        assert!(parse_gh_target("https://github.com/a/b").is_none());
    }
}

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const MAX_CONCURRENT: usize = 8;

/// Enrich rows in place. Synchronous wrapper around the tokio runtime.
/// Returns the rows with `enriched` filled where possible.
pub fn enrich_rows(mut rows: Vec<crate::link_picker::detect::Row>, deadline_ms: u64) -> Vec<crate::link_picker::detect::Row> {
    if deadline_ms == 0 {
        return rows;
    }

    // Load cache synchronously (fast, no async needed).
    let mut cache = CacheFile::load_or_reset();

    // Apply cache hits immediately.
    let mut needs_fetch: Vec<usize> = Vec::new();
    for (idx, row) in rows.iter_mut().enumerate() {
        if parse_gh_target(&row.canonical).is_none() {
            continue; // Not an enrichable category
        }
        if let Some(hit) = cache.lookup(&row.canonical) {
            row.enriched = Some(hit);
        } else {
            needs_fetch.push(idx);
        }
    }

    if needs_fetch.is_empty() {
        return rows;
    }

    // Build a single-thread tokio runtime and fan out.
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return rows, // Runtime build failed; degrade silently
    };

    // Pre-collect URLs by index so the spawn closures only capture owned strings.
    let fetch_list: Vec<(usize, String)> = needs_fetch
        .iter()
        .map(|&idx| (idx, rows[idx].canonical.clone()))
        .collect();

    let deadline = Duration::from_millis(deadline_ms);
    let fetched: Vec<(usize, EnrichedTitle)> = rt.block_on(async move {
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let mut set: JoinSet<Option<(usize, EnrichedTitle)>> = JoinSet::new();
        for (idx, url) in fetch_list {
            let sem = sem.clone();
            set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                call_gh(&url).await.ok().flatten().map(|e| (idx, e))
            });
        }
        let drain = async {
            let mut out = Vec::new();
            while let Some(res) = set.join_next().await {
                if let Ok(Some(pair)) = res {
                    out.push(pair);
                }
            }
            out
        };
        tokio::time::timeout(deadline, drain).await.unwrap_or_default()
    });

    // Apply fetched results + update cache.
    for (idx, enrichment) in &fetched {
        cache.insert(rows[*idx].canonical.clone(), enrichment);
        rows[*idx].enriched = Some(enrichment.clone());
    }
    let _ = cache.save(); // swallow errors silently
    rows
}
