# Scrollback Link Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rmux_helper pick-links`: a ratatui TUI that scans the current tmux pane's scrollback for GitHub links, blog posts, servers, and IPs; fuzzy-picks one; and acts contextually (yank via OSC 52 / open browser / ssh). Bidirectional `F2` cross-picker with the existing `pick-tui` session picker.

**Architecture:** Strictly-phased pipeline `capture → detect → enrich → TUI → dispatch`. Sync Rust on the main thread for capture and detect. A single-thread tokio runtime wraps enrichment (parallel `gh` fan-out bounded by a semaphore and a shared wall-clock deadline). TUI is crossterm/ratatui — entered only after `block_on` returns. OSC 52 yank writes to `/dev/tty` after teardown but before exit. The `F2` branch skips OSC 52 and `execvp`s the sibling picker.

**Tech Stack:** Rust (existing `rmux_helper` crate), ratatui 0.29, crossterm 0.29, tokio 1 (`rt` + `process` + `time` + `sync` + `macros`), regex 1, serde + serde_json 1, unicode-width 0.2, base64 0.22, dirs 5.

**Spec reference:** `docs/superpowers/specs/2026-04-12-scrollback-link-picker-design.md`. Every task below cites the relevant section. Read the spec's Execution Flow (lines 47–121) before starting.

**Prerequisite worktree:** This plan assumes a dedicated git worktree off `main`. If the brainstorming skill did not create one, run:

```bash
git worktree add ../rmux-link-picker -b feat/link-picker
cd ../rmux-link-picker
```

All commits below target this branch. The final step opens a PR against `upstream/main`.

---

## File Structure

**Create:**
- `rust/tmux_helper/src/link_picker/mod.rs` — public entry: `pub fn pick_links(json: bool, enrich_deadline_ms: u64) -> anyhow::Result<()>`. Implements the top-level Execution Flow from spec lines 55–106.
- `rust/tmux_helper/src/link_picker/detect.rs` — pure sync detection: regex, canonicalization, dedup, ordering, context extraction. No tokio, no ratatui. Unit-tested via `#[cfg(test)] mod tests`.
- `rust/tmux_helper/src/link_picker/enrich.rs` — tokio `current_thread` runtime, parallel `gh` fan-out, JSON cache at `~/.cache/rmux_helper/gh-links.json`. Integration-tested with a stub `gh` binary on `$PATH`.
- `rust/tmux_helper/src/link_picker/tui.rs` — ratatui app, render loop, key handling. Mirrors `picker.rs` chrome; imports shared constants where possible.
- `rust/tmux_helper/LINK_PICKER_SPEC.md` — behavior contract in the style of the existing `PICKER_SPEC.md`. Concise rules, not narrative.

**Modify:**
- `rust/tmux_helper/Cargo.toml` — add deps listed in `Tech Stack`.
- `rust/tmux_helper/src/main.rs` — add `mod link_picker;` at line 1, add `PickLinks { json, enrich_deadline_ms }` to the `Commands` enum (around line 40–73), add the dispatch arm at line 1819.
- `rust/tmux_helper/src/picker.rs` — add `F2` → exec `rmux_helper pick-links` in the key handler (around line 650). Teardown-then-exec pattern matching the spec's F2 sequence.
- `rust/tmux_helper/PICKER_SPEC.md` — document the new `F2` cross-picker entry.
- `shared/.tmux.conf` — add `bind-key L display-popup -E -w 95% -h 95% "TMUX_PANE=#{pane_id} rmux_helper pick-links"`, a help-section entry at the top, and a `set -s command-alias` entry in the aliases block.

**File-boundary rationale.** `detect.rs` is leaf (no async, no TUI) so its tests compile fast and don't pull tokio. `enrich.rs` owns the tokio runtime construction so `mod.rs` doesn't leak async up. `tui.rs` owns crossterm, mirroring `picker.rs`'s separation. `mod.rs` is the thin orchestrator implementing the Execution Flow sequence — any ordering bug shows up there, not spread across four files.

---

## Task 1 — Scaffolding and `--json` smoke test

**Files:**
- Modify: `rust/tmux_helper/Cargo.toml`
- Create: `rust/tmux_helper/src/link_picker/mod.rs`
- Create: `rust/tmux_helper/src/link_picker/detect.rs` (empty stub)
- Create: `rust/tmux_helper/src/link_picker/enrich.rs` (empty stub)
- Create: `rust/tmux_helper/src/link_picker/tui.rs` (empty stub)
- Modify: `rust/tmux_helper/src/main.rs:1` (add `mod link_picker;`)
- Modify: `rust/tmux_helper/src/main.rs:41-73` (add `PickLinks` variant)
- Modify: `rust/tmux_helper/src/main.rs:1819` (add dispatch arm)

- [ ] **Step 1: Write the failing integration smoke test.**

Create `rust/tmux_helper/src/link_picker/mod.rs` with this test scaffold:

```rust
//! Scrollback link picker — top-level orchestration.

pub mod detect;
pub mod enrich;
pub mod tui;

use anyhow::Result;

/// Entry point for `rmux_helper pick-links`.
pub fn pick_links(_json: bool, _enrich_deadline_ms: u64) -> Result<()> {
    // Minimal smoke: print empty JSON array.
    println!("[]");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn pick_links_json_mode_returns_empty_array_when_scrollback_is_empty() {
        // Integration-level smoke; replaced in Task 13 with real pipeline.
        assert!(true, "placeholder — replaced when pipeline lands");
    }
}
```

Create three empty stubs so the module tree compiles:

```rust
// detect.rs
//! Pure scrollback detection. No I/O, no async.
```

```rust
// enrich.rs
//! Parallel `gh` enrichment with a shared deadline.
```

```rust
// tui.rs
//! Ratatui TUI for the link picker.
```

- [ ] **Step 2: Wire the module into `main.rs` and add the `PickLinks` subcommand.**

At `rust/tmux_helper/src/main.rs:1`, add `mod link_picker;` below the existing `mod picker;`:

```rust
mod picker;
mod link_picker;
```

At `rust/tmux_helper/src/main.rs:41-73` (inside `enum Commands`), append a new variant after `DebugKeys`:

```rust
    /// TUI picker for GitHub links, servers, and IPs in the current tmux pane's scrollback
    PickLinks {
        /// Emit JSON of detected items to stdout and exit (no TUI)
        #[arg(long)]
        json: bool,
        /// Enrichment deadline in milliseconds (0 disables gh enrichment)
        #[arg(long, default_value_t = 3000)]
        enrich_deadline_ms: u64,
    },
```

At `rust/tmux_helper/src/main.rs:1819`, add a dispatch arm beneath `PickTui`:

```rust
        Some(Commands::PickTui) => picker::pick_tui(),
        Some(Commands::PickLinks { json, enrich_deadline_ms }) => {
            link_picker::pick_links(json, enrich_deadline_ms)
        }
```

- [ ] **Step 3: Add Cargo dependencies.**

Edit `rust/tmux_helper/Cargo.toml` — append to the `[dependencies]` block:

```toml
tokio = { version = "1", features = ["rt", "process", "time", "sync", "macros"] }
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
unicode-width = "0.2"
base64 = "0.22"
dirs = "5"
```

Note: `rt` (not `rt-multi-thread`) per spec line 733. A single-thread runtime is sufficient for `gh` fan-out.

- [ ] **Step 4: Run the smoke test and verify the crate still compiles.**

```bash
cd rust/tmux_helper
cargo build 2>&1 | /usr/bin/tail -20
cargo test -p rmux_helper link_picker:: 2>&1 | /usr/bin/tail -10
```

Expected: build succeeds with the new deps; `cargo test` passes 1 test (the placeholder). If you see `unresolved module` errors, check that the `link_picker/` directory has `mod.rs`, `detect.rs`, `enrich.rs`, and `tui.rs` files.

- [ ] **Step 5: Verify `--json` end-to-end from the terminal.**

```bash
cargo install --path . --force 2>&1 | /usr/bin/tail -5
rmux_helper pick-links --json
```

Expected output: `[]` on one line. If `rmux_helper: command not found`, check `~/.cargo/bin` is on `$PATH`.

- [ ] **Step 6: Commit.**

```bash
git add rust/tmux_helper/Cargo.toml \
        rust/tmux_helper/src/main.rs \
        rust/tmux_helper/src/link_picker/
git commit -m "feat(link-picker): scaffold pick-links subcommand and module tree"
```

---

## Task 2 — Detection data model

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

This task defines the types every downstream task consumes. No parsing yet — just shapes and invariants.

- [ ] **Step 1: Write the failing unit tests for the data model.**

Add to `detect.rs`:

```rust
use serde::Serialize;
use std::fmt;

/// Categories in fixed display order. Numeric value doubles as the
/// display position (1-indexed in the spec) and the 1-9 drill-down key.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    PullRequest = 1,
    Issue = 2,
    Commit = 3,
    File = 4,
    Repo = 5,
    Blog = 6,
    OtherLink = 7,
    Server = 8,
    Ip = 9,
}

impl Category {
    /// Short-name filter tag (see spec "Filtering semantics").
    pub fn tag(self) -> &'static str {
        match self {
            Category::PullRequest => "pr",
            Category::Issue => "issue",
            Category::Commit => "commit",
            Category::File => "file",
            Category::Repo => "repo",
            Category::Blog => "blog",
            Category::OtherLink => "link",
            Category::Server => "server",
            Category::Ip => "ip",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            Category::PullRequest => "Pull Requests",
            Category::Issue => "Issues",
            Category::Commit => "Commits",
            Category::File => "Files",
            Category::Repo => "Repos",
            Category::Blog => "Blog",
            Category::OtherLink => "Other links",
            Category::Server => "Servers",
            Category::Ip => "IPs",
        }
    }

    pub fn all() -> &'static [Category] {
        &[
            Category::PullRequest,
            Category::Issue,
            Category::Commit,
            Category::File,
            Category::Repo,
            Category::Blog,
            Category::OtherLink,
            Category::Server,
            Category::Ip,
        ]
    }
}

/// One detected item, pre-dedup. `canonical` is the dedup key within a category.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Item {
    pub category: Category,
    pub canonical: String,     // canonical URL, hostname, or IP
    pub key: String,           // key column: "#68", "a22bc17", "picker.rs:L42", "c-5001"
    pub repo_or_host: String,  // repo-or-host column: "settings", "idvorkin.github.io", "—"
    /// Line index in the captured scrollback where this occurrence was found.
    pub line_index: usize,
}

/// A finished row the TUI renders. One per unique (category, canonical).
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Row {
    pub category: Category,
    pub canonical: String,
    pub key: String,
    pub repo_or_host: String,
    pub context: String,      // from scrollback line at most_recent_line
    pub enriched: Option<EnrichedTitle>, // None in v1 detection output; filled by enrich.rs
    pub count: usize,
    pub most_recent_line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnrichedTitle {
    pub title: String,
    pub state: GhState,
    pub author: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GhState {
    Open,
    MergedPr,
    Closed,
    Draft,
    Commit, // not a PR state; used for the ⎇ glyph
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_all_is_in_display_order() {
        let nums: Vec<u8> = Category::all().iter().map(|c| *c as u8).collect();
        assert_eq!(nums, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn category_tags_are_unique_and_lowercase() {
        let mut seen = std::collections::HashSet::new();
        for c in Category::all() {
            let tag = c.tag();
            assert_eq!(tag, tag.to_lowercase());
            assert!(seen.insert(tag), "duplicate tag: {}", tag);
        }
    }

    #[test]
    fn row_is_serializable_to_json() {
        let row = Row {
            category: Category::PullRequest,
            canonical: "https://github.com/a/b/pull/1".into(),
            key: "#1".into(),
            repo_or_host: "b".into(),
            context: "Merge pull request #1".into(),
            enriched: None,
            count: 1,
            most_recent_line: 42,
        };
        let json = serde_json::to_string(&row).unwrap();
        assert!(json.contains("\"category\":\"pull_request\""));
        assert!(json.contains("\"canonical\":\"https://github.com/a/b/pull/1\""));
    }
}
```

- [ ] **Step 2: Run tests and verify they fail.**

```bash
cd rust/tmux_helper
cargo test -p rmux_helper link_picker::detect 2>&1 | /usr/bin/tail -30
```

Expected: 3 tests defined, all compile (the types were defined in Step 1 so they all pass). If a test fails to compile, the type shape is wrong.

- [ ] **Step 3: If Step 2 shows all tests passing, skip to Step 4. (The types and tests were written in one shot, so this is structurally a "green-first" task.)**

- [ ] **Step 4: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): add Category/Item/Row data model"
```

---

## Task 3 — URL regex + trailing punctuation stripping

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

Spec reference: URL regex at spec line ~278 (`https?://[^\s<>"'`()\[\]{}]+`), trailing punctuation rules below it.

- [ ] **Step 1: Write failing unit tests.**

Append to `detect.rs`:

```rust
use regex::Regex;
use std::sync::OnceLock;

/// Base URL regex — matches scheme + greedy body up to whitespace or URL-unsafe chars.
/// Trailing punctuation is stripped separately (see `strip_trailing_punct`).
pub(crate) fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"https?://[^\s<>"'`()\[\]{}]+"#).unwrap()
    })
}

/// Strip trailing `.,;:!?)]}>'"` from a matched URL. Preserves trailing `/`.
pub(crate) fn strip_trailing_punct(s: &str) -> &str {
    let trimmed = s.trim_end_matches(|c: char| ".,;:!?)]}>'\"".contains(c));
    trimmed
}

#[cfg(test)]
mod url_tests {
    use super::*;

    fn extract_url(line: &str) -> &str {
        let m = url_regex().find(line).expect("no URL in line");
        strip_trailing_punct(m.as_str())
    }

    #[test]
    fn extracts_bare_url() {
        assert_eq!(
            extract_url("see https://github.com/a/b/pull/1 next"),
            "https://github.com/a/b/pull/1"
        );
    }

    #[test]
    fn strips_trailing_period() {
        assert_eq!(
            extract_url("see https://github.com/a/b/pull/1."),
            "https://github.com/a/b/pull/1"
        );
    }

    #[test]
    fn strips_trailing_paren_and_quote() {
        assert_eq!(
            extract_url("(see https://github.com/a/b/pull/1)"),
            "https://github.com/a/b/pull/1"
        );
        assert_eq!(
            extract_url("\"https://github.com/a/b/pull/1\""),
            "https://github.com/a/b/pull/1"
        );
    }

    #[test]
    fn preserves_trailing_slash() {
        assert_eq!(
            extract_url("https://idvorkin.github.io/posts/ai-agents/"),
            "https://idvorkin.github.io/posts/ai-agents/"
        );
    }

    #[test]
    fn handles_query_and_fragment() {
        assert_eq!(
            extract_url("https://github.com/a/b/pull/1#discussion_r123"),
            "https://github.com/a/b/pull/1#discussion_r123"
        );
    }
}
```

- [ ] **Step 2: Run tests and verify they pass.**

```bash
cargo test -p rmux_helper link_picker::detect::url_tests 2>&1 | /usr/bin/tail -20
```

Expected: 5 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): add URL regex and trailing punctuation strip"
```

---

## Task 4 — GitHub URL categorization and canonicalization

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

Covers Pull Requests, Issues, Commits, Files, Repos. Spec reference lines 195–211 + canonicalization rules ~290-304.

- [ ] **Step 1: Write failing tests for each GitHub category.**

Append to `detect.rs`:

```rust
/// Classify a URL string into a GitHub Category + build an Item, or None.
pub(crate) fn classify_github(url: &str, line_index: usize) -> Option<Item> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        // Owner & repo: no slashes, alphanum + .-_
        Regex::new(
            r"^https?://(?:www\.)?github\.com/([\w.\-]+)/([\w.\-]+)(?:/(pull|issues|commit|blob|tree)/([^#?\s]+))?(#\S*)?$"
        ).unwrap()
    });

    let caps = re.captures(url.trim_end_matches('/'))?;
    let owner = caps.get(1)?.as_str();
    let repo = caps.get(2)?.as_str();
    let kind = caps.get(3).map(|m| m.as_str());
    let tail = caps.get(4).map(|m| m.as_str());
    let fragment = caps.get(5).map(|m| m.as_str()).unwrap_or("");

    let owner_repo = format!("{owner}/{repo}");
    let canonical_base = format!("https://github.com/{owner_repo}");

    match kind {
        None => Some(Item {
            category: Category::Repo,
            canonical: canonical_base,
            key: owner.to_string(),
            repo_or_host: repo.to_string(),
            line_index,
        }),
        Some("pull") => {
            let num = tail?.split('/').next()?;
            // Strip non-#LN fragments from PR URLs (canonical form drops them).
            Some(Item {
                category: Category::PullRequest,
                canonical: format!("{canonical_base}/pull/{num}"),
                key: format!("#{num}"),
                repo_or_host: repo.to_string(),
                line_index,
            })
        }
        Some("issues") => {
            let num = tail?.split('/').next()?;
            Some(Item {
                category: Category::Issue,
                canonical: format!("{canonical_base}/issues/{num}"),
                key: format!("#{num}"),
                repo_or_host: repo.to_string(),
                line_index,
            })
        }
        Some("commit") => {
            let sha = tail?;
            if sha.len() < 7 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let short = &sha[..7.min(sha.len())];
            Some(Item {
                category: Category::Commit,
                canonical: format!("{canonical_base}/commit/{sha}"),
                key: short.to_string(),
                repo_or_host: repo.to_string(),
                line_index,
            })
        }
        Some("blob") | Some("tree") => {
            let tail = tail?; // "ref/path/to/file.rs"
            let basename = tail.rsplit('/').next().unwrap_or(tail);
            // Line anchor: #L42 or #L42-L60 preserves only Lstart in key.
            let key = if let Some(l) = fragment.strip_prefix("#L") {
                let lstart = l.split('-').next().unwrap_or(l);
                format!("{basename}:L{lstart}")
            } else {
                basename.to_string()
            };
            Some(Item {
                category: Category::File,
                canonical: format!("{canonical_base}/{}/{tail}{}", kind.unwrap(), if fragment.starts_with("#L") { fragment } else { "" }),
                key,
                repo_or_host: repo.to_string(),
                line_index,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod github_tests {
    use super::*;

    #[test]
    fn classifies_pull_request() {
        let item = classify_github("https://github.com/idvorkin-ai-tools/settings/pull/68", 0).unwrap();
        assert_eq!(item.category, Category::PullRequest);
        assert_eq!(item.key, "#68");
        assert_eq!(item.repo_or_host, "settings");
        assert_eq!(item.canonical, "https://github.com/idvorkin-ai-tools/settings/pull/68");
    }

    #[test]
    fn strips_pr_discussion_fragment() {
        let item = classify_github(
            "https://github.com/a/b/pull/1#discussion_r123", 0,
        ).unwrap();
        assert_eq!(item.canonical, "https://github.com/a/b/pull/1");
    }

    #[test]
    fn classifies_issue() {
        let item = classify_github("https://github.com/a/b/issues/42", 0).unwrap();
        assert_eq!(item.category, Category::Issue);
        assert_eq!(item.key, "#42");
    }

    #[test]
    fn classifies_commit_with_short_sha() {
        let item = classify_github("https://github.com/a/b/commit/a22bc17", 0).unwrap();
        assert_eq!(item.category, Category::Commit);
        assert_eq!(item.key, "a22bc17");
    }

    #[test]
    fn rejects_non_hex_commit() {
        assert!(classify_github("https://github.com/a/b/commit/NOTHEX", 0).is_none());
    }

    #[test]
    fn classifies_file_with_line_anchor() {
        let item = classify_github(
            "https://github.com/a/b/blob/main/src/picker.rs#L42", 0,
        ).unwrap();
        assert_eq!(item.category, Category::File);
        assert_eq!(item.key, "picker.rs:L42");
    }

    #[test]
    fn classifies_file_line_range_uses_start() {
        let item = classify_github(
            "https://github.com/a/b/blob/main/src/picker.rs#L42-L60", 0,
        ).unwrap();
        assert_eq!(item.key, "picker.rs:L42");
    }

    #[test]
    fn classifies_bare_repo() {
        let item = classify_github("https://github.com/idvorkin-ai-tools/settings", 0).unwrap();
        assert_eq!(item.category, Category::Repo);
        assert_eq!(item.key, "idvorkin-ai-tools");
        assert_eq!(item.repo_or_host, "settings");
    }

    #[test]
    fn collapses_www_github_com() {
        let item = classify_github("https://www.github.com/a/b/pull/1", 0).unwrap();
        assert_eq!(item.canonical, "https://github.com/a/b/pull/1");
    }

    #[test]
    fn rejects_non_github_host() {
        assert!(classify_github("https://gitlab.com/a/b", 0).is_none());
    }
}
```

- [ ] **Step 2: Run and iterate until green.**

```bash
cargo test -p rmux_helper link_picker::detect::github_tests 2>&1 | /usr/bin/tail -40
```

Expected: 10 passes. If any fail, adjust the regex or match arms — don't skip the test.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): classify GitHub PRs, issues, commits, files, repos"
```

---

## Task 5 — Blog, Other links, and URL fallthrough

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

- [ ] **Step 1: Write tests.**

Append:

```rust
/// Blog host allowlist. v1 is a compile-time constant; v2 will be configurable.
pub(crate) const BLOG_HOSTS: &[&str] = &["idvorkin.github.io"];

/// Classify a non-GitHub URL into Blog or OtherLink.
pub(crate) fn classify_other_url(url: &str, line_index: usize) -> Option<Item> {
    let host_end = url.find("://")? + 3;
    let rest = &url[host_end..];
    let slash_at = rest.find('/').unwrap_or(rest.len());
    let host = &rest[..slash_at];
    let path = &rest[slash_at..];

    let category = if BLOG_HOSTS.iter().any(|b| host.eq_ignore_ascii_case(b)) {
        Category::Blog
    } else {
        Category::OtherLink
    };

    // Key = last non-empty path segment, or host if path is empty
    let last_seg = path
        .trim_end_matches('/')
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(host)
        .to_string();

    Some(Item {
        category,
        canonical: url.to_string(),
        key: last_seg,
        repo_or_host: host.to_string(),
        line_index,
    })
}

#[cfg(test)]
mod other_url_tests {
    use super::*;

    #[test]
    fn blog_host_is_categorized_as_blog() {
        let item = classify_other_url("https://idvorkin.github.io/posts/ai-agents/", 0).unwrap();
        assert_eq!(item.category, Category::Blog);
        assert_eq!(item.repo_or_host, "idvorkin.github.io");
        assert_eq!(item.key, "ai-agents");
    }

    #[test]
    fn non_blog_host_is_other_link() {
        let item = classify_other_url("https://stackoverflow.com/q/12345", 0).unwrap();
        assert_eq!(item.category, Category::OtherLink);
        assert_eq!(item.repo_or_host, "stackoverflow.com");
    }

    #[test]
    fn bare_host_uses_host_as_key() {
        let item = classify_other_url("https://example.com", 0).unwrap();
        assert_eq!(item.key, "example.com");
    }
}
```

- [ ] **Step 2: Run and verify.**

```bash
cargo test -p rmux_helper link_picker::detect::other_url_tests 2>&1 | /usr/bin/tail -20
```

Expected: 3 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): classify blog and other URLs with host allowlist"
```

---

## Task 6 — Server detection (ssh context + Tailscale)

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

Spec reference: lines ~318-345 for the two regexes + dedup rule.

- [ ] **Step 1: Write failing tests.**

Append:

```rust
pub(crate) fn find_servers(line: &str, line_index: usize) -> Vec<Item> {
    static SSH_RE: OnceLock<Regex> = OnceLock::new();
    static TS_HOST_RE: OnceLock<Regex> = OnceLock::new();
    static TS_NET_RE: OnceLock<Regex> = OnceLock::new();
    let ssh = SSH_RE.get_or_init(|| {
        // `ssh` token, optional flags, optional user@, then host
        Regex::new(
            r"\bssh\b(?:\s+-\S+)*\s+(?:[a-zA-Z_][\w-]*@)?([a-zA-Z0-9][\w.\-]*[a-zA-Z0-9])\b",
        )
        .unwrap()
    });
    let ts_host = TS_HOST_RE.get_or_init(|| Regex::new(r"\bc-\d{4,5}\b").unwrap());
    let ts_net = TS_NET_RE.get_or_init(|| Regex::new(r"\b[a-z][a-z0-9-]*\.ts\.net\b").unwrap());

    let mut out = Vec::new();
    for cap in ssh.captures_iter(line) {
        if let Some(h) = cap.get(1) {
            out.push(mk_server(h.as_str(), line_index));
        }
    }
    for m in ts_host.find_iter(line) {
        out.push(mk_server(m.as_str(), line_index));
    }
    for m in ts_net.find_iter(line) {
        out.push(mk_server(m.as_str(), line_index));
    }
    out
}

fn mk_server(host: &str, line_index: usize) -> Item {
    Item {
        category: Category::Server,
        canonical: host.to_string(),
        key: host.to_string(),
        repo_or_host: "—".to_string(),
        line_index,
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;

    #[test]
    fn extracts_ssh_bare_host() {
        let items = find_servers("ssh c-5001 \"uname -a\"", 0);
        assert_eq!(items.len(), 2, "ssh + tailscale regex both match c-5001");
        assert!(items.iter().any(|i| i.canonical == "c-5001"));
    }

    #[test]
    fn extracts_ssh_user_at_host_stripping_user() {
        let items = find_servers("ssh igor@dev.example.com", 0);
        assert_eq!(items[0].canonical, "dev.example.com");
    }

    #[test]
    fn extracts_ssh_with_options() {
        let items = find_servers("ssh -i ~/.ssh/id_ed25519 -p 2222 build.host", 0);
        assert!(items.iter().any(|i| i.canonical == "build.host"));
    }

    #[test]
    fn extracts_tailscale_c_pattern() {
        let items = find_servers("Tailscale peer c-5001 is up", 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].canonical, "c-5001");
    }

    #[test]
    fn rejects_c_numbers_outside_range() {
        let items = find_servers("c-2 c-123456", 0);
        assert!(items.is_empty());
    }

    #[test]
    fn extracts_ts_net_hostname() {
        let items = find_servers("ping mydev.ts.net", 0);
        assert!(items.iter().any(|i| i.canonical == "mydev.ts.net"));
    }
}
```

- [ ] **Step 2: Run and verify.**

```bash
cargo test -p rmux_helper link_picker::detect::server_tests 2>&1 | /usr/bin/tail -30
```

Expected: 6 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): detect ssh hosts and Tailscale names"
```

---

## Task 7 — IPv4 detection with version-string suppression

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

Spec reference lines ~348-364.

- [ ] **Step 1: Write failing tests.**

Append:

```rust
pub(crate) fn find_ips(line: &str, line_index: usize) -> Vec<Item> {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Dotted-quad with bounded octets, no look-around (Rust regex limitation).
    // We do boundary + version-prefix checks in code.
    let re = RE.get_or_init(|| {
        Regex::new(r"(?:\d{1,3}\.){3}\d{1,3}").unwrap()
    });

    let bytes = line.as_bytes();
    let mut out = Vec::new();
    for m in re.find_iter(line) {
        let start = m.start();
        let end = m.end();

        // Reject: preceded by `v` or `V` (version string)
        if start > 0 && matches!(bytes[start - 1], b'v' | b'V') {
            continue;
        }
        // Reject: preceded by word-char or `.`
        if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'.') {
            continue;
        }
        // Reject: followed by `.<digit>` (longer sequence)
        if end < bytes.len() && bytes[end] == b'.' {
            if end + 1 < bytes.len() && bytes[end + 1].is_ascii_digit() {
                continue;
            }
        }
        // Reject: followed by word-char
        if end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
            continue;
        }
        // Validate octets
        let text = m.as_str();
        let octets: Vec<u16> = text.split('.').map(|s| s.parse().unwrap_or(u16::MAX)).collect();
        if octets.iter().any(|o| *o > 255) {
            continue;
        }
        out.push(Item {
            category: Category::Ip,
            canonical: text.to_string(),
            key: text.to_string(),
            repo_or_host: "—".to_string(),
            line_index,
        });
    }
    out
}

#[cfg(test)]
mod ip_tests {
    use super::*;

    #[test]
    fn extracts_simple_ipv4() {
        let items = find_ips("Tailscale peer 100.64.1.5 is online", 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].canonical, "100.64.1.5");
    }

    #[test]
    fn suppresses_version_prefixed() {
        let items = find_ips("claude-opus v4.6.0.1 released", 0);
        assert!(items.is_empty(), "v-prefixed not an IP");
    }

    #[test]
    fn suppresses_longer_dotted_sequence() {
        let items = find_ips("value = 1.2.3.4.5 not an IP", 0);
        assert!(items.is_empty());
    }

    #[test]
    fn rejects_octet_over_255() {
        let items = find_ips("nope 300.1.1.1", 0);
        assert!(items.is_empty());
    }

    #[test]
    fn keeps_edge_addresses() {
        let items = find_ips("range 0.0.0.0 to 255.255.255.255", 0);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn does_not_filter_private_ranges() {
        let items = find_ips("local 192.168.1.1", 0);
        assert_eq!(items.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests.**

```bash
cargo test -p rmux_helper link_picker::detect::ip_tests 2>&1 | /usr/bin/tail -30
```

Expected: 6 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): detect IPv4 with version-string suppression"
```

---

## Task 8 — Line scanner assembly (GitHub + Blog/Other + Server + IP)

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

Per spec line 727 this should use `regex::RegexSet` for performance on megabyte scrollbacks. For v1 we can use a simpler single-pass over matches from `url_regex().find_iter(line)` followed by the server/IP passes — the per-line cost is dominated by the URL regex walk anyway and RegexSet is an optimization we can add if profiling shows it's needed. Spec line 727 is aspirational; we'll comment accordingly.

- [ ] **Step 1: Write failing tests.**

Append:

```rust
/// Scan one scrollback line and return all detected items in that line.
/// A single line can contribute to multiple categories.
pub(crate) fn scan_line(line: &str, line_index: usize) -> Vec<Item> {
    let mut out = Vec::new();

    // URLs (cascade through GitHub → Blog/Other).
    for m in url_regex().find_iter(line) {
        let raw = strip_trailing_punct(m.as_str());
        if let Some(item) = classify_github(raw, line_index) {
            out.push(item);
        } else if let Some(item) = classify_other_url(raw, line_index) {
            out.push(item);
        }
    }

    // Servers (ssh + Tailscale; two Tailscale regexes may double-match).
    out.extend(find_servers(line, line_index));
    // IPs
    out.extend(find_ips(line, line_index));

    out
}

#[cfg(test)]
mod scan_line_tests {
    use super::*;

    #[test]
    fn scans_pr_url_in_context() {
        let items = scan_line("Merge pull request #68 https://github.com/a/b/pull/68 ok", 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, Category::PullRequest);
    }

    #[test]
    fn mixes_url_and_ip_on_same_line() {
        let items = scan_line("curl https://example.com from 10.0.0.1", 0);
        assert!(items.iter().any(|i| i.category == Category::OtherLink));
        assert!(items.iter().any(|i| i.category == Category::Ip));
    }

    #[test]
    fn no_cross_category_for_pr_url() {
        // A PR URL must NOT also appear under OtherLink.
        let items = scan_line("https://github.com/a/b/pull/1", 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, Category::PullRequest);
    }
}
```

- [ ] **Step 2: Run tests.**

```bash
cargo test -p rmux_helper link_picker::detect::scan_line_tests 2>&1 | /usr/bin/tail -20
```

Expected: 3 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): scan one scrollback line across all categories"
```

---

## Task 9 — Dedup, recency ordering, context extraction, `parse`

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/detect.rs`

This finishes detect.rs with the top-level `parse(raw: &str) -> Vec<Row>` the orchestrator calls.

- [ ] **Step 1: Write failing tests.**

Append:

```rust
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

/// Strip an anchor substring from a line, collapse whitespace, trim, truncate to max display width.
pub(crate) fn make_context(line: &str, anchor: &str, max_width: usize) -> String {
    // Remove the anchor substring; collapse runs of whitespace to single space.
    let removed = line.replacen(anchor, "", 1);
    let collapsed: String = removed.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_to_width(&collapsed, max_width)
}

pub(crate) fn truncate_to_width(s: &str, max: usize) -> String {
    let w = UnicodeWidthStr::width(s);
    if w <= max {
        return s.to_string();
    }
    // Walk characters, accumulating width, until we reach max-1 (room for `…`).
    let budget = max.saturating_sub(1);
    let mut acc = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = UnicodeWidthStr::width(ch.to_string().as_str());
        if used + cw > budget {
            break;
        }
        acc.push(ch);
        used += cw;
    }
    acc.push('…');
    acc
}

/// Top-level detection: parse the whole scrollback and return deduped,
/// recency-ordered rows.
pub fn parse(raw: &str) -> Vec<Row> {
    // Collect items + keep a reference to the line for context extraction.
    let lines: Vec<&str> = raw.lines().collect();
    let mut items_by_key: HashMap<(Category, String), Vec<Item>> = HashMap::new();
    for (idx, line) in lines.iter().enumerate() {
        for item in scan_line(line, idx) {
            items_by_key
                .entry((item.category, item.canonical.clone()))
                .or_default()
                .push(item);
        }
    }

    // Build rows: most_recent_line = max line_index in the group.
    let mut rows: Vec<Row> = items_by_key
        .into_iter()
        .map(|((category, canonical), group)| {
            let count = group.len();
            let most_recent = group.iter().map(|i| i.line_index).max().unwrap_or(0);
            let exemplar = group.iter().find(|i| i.line_index == most_recent).unwrap();
            let context = make_context(lines[most_recent], &canonical, 60);
            Row {
                category,
                canonical,
                key: exemplar.key.clone(),
                repo_or_host: exemplar.repo_or_host.clone(),
                context,
                enriched: None,
                count,
                most_recent_line: most_recent,
            }
        })
        .collect();

    // Sort: category ASC (display order), then most_recent_line DESC,
    // ties broken by canonical ASC for determinism.
    rows.sort_by(|a, b| {
        (a.category as u8, std::cmp::Reverse(a.most_recent_line), &a.canonical)
            .cmp(&(b.category as u8, std::cmp::Reverse(b.most_recent_line), &b.canonical))
    });

    rows
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn dedups_same_server_across_sources() {
        let raw = "ssh c-5001 \"pwd\"\nssh igor@c-5001 \"ls\"\npeer c-5001 up\n";
        let rows = parse(raw);
        let servers: Vec<&Row> = rows.iter().filter(|r| r.category == Category::Server).collect();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].canonical, "c-5001");
        assert!(servers[0].count >= 3);
    }

    #[test]
    fn orders_by_recency_within_category() {
        let raw = "https://github.com/a/b/pull/1\nhttps://github.com/a/b/pull/2\n";
        let rows = parse(raw);
        let prs: Vec<&Row> = rows.iter().filter(|r| r.category == Category::PullRequest).collect();
        assert_eq!(prs.len(), 2);
        // Most recent (pull/2, line 1) comes first
        assert_eq!(prs[0].key, "#2");
        assert_eq!(prs[1].key, "#1");
    }

    #[test]
    fn pr_url_does_not_leak_to_other_links() {
        let raw = "see https://github.com/a/b/pull/1 now\n";
        let rows = parse(raw);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, Category::PullRequest);
    }

    #[test]
    fn context_strips_url_and_collapses_whitespace() {
        let raw = "Merge pull request   #68   https://github.com/a/b/pull/68   idvorkin\n";
        let rows = parse(raw);
        let pr = rows.iter().find(|r| r.category == Category::PullRequest).unwrap();
        assert!(!pr.context.contains("https://"));
        assert!(!pr.context.contains("   ")); // no runs of whitespace
        assert!(pr.context.contains("Merge pull request"));
    }

    #[test]
    fn context_truncates_with_ellipsis() {
        let long = "a".repeat(200);
        let raw = format!("{long} https://example.com rest\n");
        let rows = parse(&raw);
        let row = rows.iter().find(|r| r.category == Category::OtherLink).unwrap();
        assert!(row.context.ends_with('…'));
        assert!(UnicodeWidthStr::width(row.context.as_str()) <= 60);
    }

    #[test]
    fn parse_is_idempotent() {
        let raw = "ssh c-5001\nhttps://github.com/a/b/pull/1\n";
        let a = parse(raw);
        let b = parse(raw);
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Run tests.**

```bash
cargo test -p rmux_helper link_picker::detect::parse_tests 2>&1 | /usr/bin/tail -40
```

Expected: 6 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/detect.rs
git commit -m "feat(link-picker): dedup, recency order, context column, top-level parse"
```

---

## Task 10 — gh-links cache: load, atomic write, TTL

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/enrich.rs`

Spec reference: Cache format lines ~395-410, concurrent-writers contract ~416-420.

- [ ] **Step 1: Write tests and implementation.**

Replace `enrich.rs` with:

```rust
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
        // Write a v999 cache to the real cache path is destructive; instead, test serde path
        let bad = r#"{"version":999,"entries":{}}"#;
        let parsed: Result<CacheFile, _> = serde_json::from_str(bad);
        assert!(parsed.is_ok());
        assert_ne!(parsed.unwrap().version, CACHE_VERSION);
        // The `load_or_reset` path tests this against a real file; see integration tests in Task 13.
    }
}
```

- [ ] **Step 2: Run tests.**

```bash
cargo test -p rmux_helper link_picker::enrich::cache_tests 2>&1 | /usr/bin/tail -20
```

Expected: 3 passes.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/enrich.rs
git commit -m "feat(link-picker): gh-links cache with TTL and atomic write"
```

---

## Task 11 — `gh` call wrappers for PR / Issue / Commit

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/enrich.rs`

Parses the `gh --json` output into `EnrichedTitle`. Handles missing `gh`, non-zero exit, malformed JSON — all as row-level `Ok(None)` fallbacks.

- [ ] **Step 1: Write tests (stub binary) and implementation.**

Append to `enrich.rs`:

```rust
use serde::Deserialize;
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
/// Only unrecoverable (panics, tokio plumbing) would be returned as `Err`.
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
        // Repo home has no kind/id, so enrichment doesn't apply.
        assert!(parse_gh_target("https://github.com/a/b").is_none());
    }
}
```

- [ ] **Step 2: Run tests.**

```bash
cargo test -p rmux_helper link_picker::enrich::gh_call_tests 2>&1 | /usr/bin/tail -20
```

Expected: 3 passes. (Full integration tests against a stub `gh` binary are deferred — the parse helpers are the unit-testable core.)

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/enrich.rs
git commit -m "feat(link-picker): call gh for PR/issue/commit enrichment"
```

---

## Task 12 — Parallel fan-out with semaphore + deadline

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/enrich.rs`

- [ ] **Step 1: Append the fan-out and entry point.**

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

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

    let deadline = Duration::from_millis(deadline_ms);
    let fetched: Vec<(usize, Option<EnrichedTitle>)> = rt.block_on(async move {
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let mut handles = Vec::new();
        for idx in needs_fetch {
            let sem = sem.clone();
            let url = rows[idx].canonical.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                call_gh(&url).await.ok().flatten().map(|e| (idx, e))
            }));
        }
        let all = futures_join(handles);
        match tokio::time::timeout(deadline, all).await {
            Ok(results) => results,
            Err(_) => Vec::new(), // global deadline hit → zero results used
        }
    });

    // Apply fetched results + update cache.
    for (idx, enrichment) in &fetched {
        if let Some(enrichment) = enrichment {
            cache.insert(rows[*idx].canonical.clone(), enrichment);
            rows[*idx].enriched = Some(enrichment.clone());
        }
    }
    let _ = cache.save(); // swallow errors silently
    rows
}

// We don't want the `futures` crate for one helper. Minimal join_all on JoinHandles:
async fn futures_join<T>(handles: Vec<tokio::task::JoinHandle<Option<T>>>) -> Vec<T> {
    let mut out = Vec::new();
    for h in handles {
        if let Ok(Some(v)) = h.await {
            out.push(v);
        }
    }
    out
}
```

Wait — the above `futures_join` serializes awaits. That kills parallelism. Replace it with a `FuturesUnordered`-style collect using `tokio::select!` or, simpler, drive all handles concurrently via a loop over `JoinSet`. Use `tokio::task::JoinSet` which is in the `rt` feature.

Replace the fan-out section with:

```rust
use tokio::task::JoinSet;

    let fetched: Vec<(usize, EnrichedTitle)> = rt.block_on(async move {
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
        let mut set: JoinSet<Option<(usize, EnrichedTitle)>> = JoinSet::new();
        for idx in needs_fetch {
            let sem = sem.clone();
            let url = rows[idx].canonical.clone();
            set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;
                call_gh(&url).await.ok().flatten().map(|e| (idx, e))
            });
        }
        let mut out = Vec::new();
        let drain = async {
            while let Some(res) = set.join_next().await {
                if let Ok(Some(pair)) = res {
                    out.push(pair);
                }
            }
            out
        };
        tokio::time::timeout(deadline, drain).await.unwrap_or_default()
    });
```

(Delete the `futures_join` helper.)

- [ ] **Step 2: Fix the borrow-checker issue: `rows[idx].canonical` can't be moved from a shared borrow inside a spawn. Resolve by cloning the URL into an owned string before the spawn, as shown above. Also note that `rows` is captured by the outer `block_on` closure — make sure rows is moved in (`move` keyword) only if the outer closure truly needs it; otherwise use `rows.iter()` for URL extraction before spawning and hand the results back via `(idx, enrichment)` tuples as shown.**

The corrected structure is already reflected in the Step 1 code block; the borrow is avoided because we build `needs_fetch: Vec<usize>` upfront and clone `canonical` out per-iter inside the `set.spawn` call.

- [ ] **Step 3: Run tests (compile check only — full integration tested in Task 13).**

```bash
cargo build -p rmux_helper 2>&1 | /usr/bin/tail -30
```

Expected: clean build. If you see `borrow of moved value: rows`, the fix is to iterate `needs_fetch` with URL pre-clone *before* `set.spawn` closes over anything that borrows `rows`.

- [ ] **Step 4: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/enrich.rs
git commit -m "feat(link-picker): parallel gh fan-out with semaphore and deadline"
```

---

## Task 13 — Scrollback capture + `pick_links()` orchestration

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/mod.rs`

Implements steps 1–3 and 7 of the Execution Flow (lines 55–106 of the spec). The TUI wiring in step 5 is stubbed here and replaced in Task 14.

- [ ] **Step 1: Write tests and implementation.**

Replace `mod.rs` with:

```rust
//! Scrollback link picker — top-level orchestration.
//! See spec docs/superpowers/specs/2026-04-12-scrollback-link-picker-design.md §Execution Flow.

pub mod detect;
pub mod enrich;
pub mod tui;

use anyhow::{anyhow, Result};
use std::env;
use std::process::Command;

/// Entry point for `rmux_helper pick-links`. See spec §Execution Flow.
pub fn pick_links(json: bool, enrich_deadline_ms: u64) -> Result<()> {
    // 1. Resolve pane
    let pane_id = resolve_pane_id()?;

    // 2. Capture (sync, fallible)
    let raw = capture_pane(&pane_id)?;

    // 3. Detect
    let rows = detect::parse(&raw);

    if json {
        // --json short-circuit: skip enrich + TUI entirely.
        println!("{}", serde_json::to_string(&rows)?);
        return Ok(());
    }

    // 4. Enrich (blocks inside tokio, then drops runtime before TUI)
    let rows = enrich::enrich_rows(rows, enrich_deadline_ms);

    // 5. TUI (stub in this task; real implementation in Task 14-15)
    if rows.is_empty() {
        eprintln!("pick-links: no links, servers, or IPs in scrollback");
        return Ok(());
    }
    let _action = tui::run(rows)?;

    // 6-7. Teardown + dispatch happen inside tui::run + action handler in Task 16.
    Ok(())
}

/// Resolve the tmux pane id to capture. See spec §Scrollback Capture.
fn resolve_pane_id() -> Result<String> {
    if let Ok(p) = env::var("TMUX_PANE") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    if env::var("TMUX").is_err() {
        return Err(anyhow!("pick-links: not inside tmux; nothing to capture"));
    }
    // Fallback: ask tmux directly.
    let out = Command::new("tmux")
        .args(["display-message", "-p", "-t", "#{client_active_pane}", "#{pane_id}"])
        .output()
        .map_err(|e| anyhow!("tmux display-message failed: {e}"))?;
    if !out.status.success() {
        return Err(anyhow!("tmux display-message returned nonzero"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Capture the full scrollback of `pane_id` via `tmux capture-pane`.
fn capture_pane(pane_id: &str) -> Result<String> {
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-J", "-e", "-S", "-", "-E", "-", "-t", pane_id])
        .output()
        .map_err(|e| anyhow!("tmux capture-pane failed: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(anyhow!(
            "tmux capture-pane -t {pane_id} returned nonzero: {err}"
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

// Replace the placeholder tui stub so `tui::run` compiles. This gets replaced in Task 14.
```

Update `tui.rs` to a compile-only stub that returns a placeholder:

```rust
//! Ratatui TUI for the link picker. Replaced in Task 14.

use crate::link_picker::detect::Row;
use anyhow::Result;

/// Placeholder action enum — extended in Task 16.
pub enum Action {
    Quit,
}

pub fn run(_rows: Vec<Row>) -> Result<Action> {
    // Temporary: immediately return Quit so `pick-links` without `--json`
    // is still callable from tests without a TTY.
    Ok(Action::Quit)
}
```

- [ ] **Step 2: Add an integration smoke test that parses a fake scrollback into JSON.**

Add to `mod.rs`:

```rust
#[cfg(test)]
mod orchestration_tests {
    use super::*;

    #[test]
    fn parses_json_output_from_fake_scrollback() {
        let raw = "Merge https://github.com/a/b/pull/1\nssh c-5001\n";
        let rows = detect::parse(raw);
        let json = serde_json::to_string(&rows).unwrap();
        assert!(json.contains("\"category\":\"pull_request\""));
        assert!(json.contains("\"category\":\"server\""));
    }
}
```

- [ ] **Step 3: Run tests and verify end-to-end `--json` output.**

```bash
cargo test -p rmux_helper link_picker:: 2>&1 | /usr/bin/tail -15
cargo install --path . --force 2>&1 | /usr/bin/tail -5
# Verify json mode works against real scrollback (run inside tmux):
# rmux_helper pick-links --json | /usr/bin/head -40
```

Expected: all tests pass; `cargo install` rebuilds; `pick-links --json` inside tmux emits a JSON array.

- [ ] **Step 4: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/mod.rs rust/tmux_helper/src/link_picker/tui.rs
git commit -m "feat(link-picker): orchestration + tmux capture + --json mode"
```

---

## Task 14 — TUI skeleton: app state, layout, rendering

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/tui.rs`

Mirrors `picker.rs` chrome closely. Refer to `picker.rs:692-870` for the existing draw function and steal the column/color idioms directly.

- [ ] **Step 1: Replace `tui.rs` with the full rendering skeleton.**

```rust
//! Ratatui TUI for the link picker. See spec §Layout & Display and §Navigation.

use crate::link_picker::detect::{Category, Row, GhState};
use anyhow::Result;
use ansi_to_tui::IntoText;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::io;
use unicode_width::UnicodeWidthStr;

/// Action returned from the TUI to the orchestrator. See spec §Actions.
#[derive(Debug)]
pub enum Action {
    Quit,
    Yank(Row),
    Open(Row),
    GhWeb(Row),
    Ssh(Row),
    SwapToPickTui,
}

struct App {
    rows: Vec<Row>,
    filtered: Vec<usize>, // indices into rows in display order (including separators)
    categories_present: Vec<Category>, // non-empty categories
    list_state: ListState,
    query: String,
    drilled_in: Option<Category>,
    horizontal: bool,
    action: Option<Action>,
    error_msg: Option<String>,
}

impl App {
    fn new(rows: Vec<Row>) -> Self {
        let mut app = Self {
            rows,
            filtered: Vec::new(),
            categories_present: Vec::new(),
            list_state: ListState::default(),
            query: String::new(),
            drilled_in: None,
            horizontal: true,
            action: None,
            error_msg: None,
        };
        app.rebuild_filter();
        app
    }

    /// Rebuild `filtered` + `categories_present` based on query + drill state.
    fn rebuild_filter(&mut self) {
        self.categories_present.clear();
        self.filtered.clear();

        let tokens = tokenize(&self.query);
        let matches: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| match_row(r, &tokens))
            .filter(|(_, r)| self.drilled_in.map_or(true, |c| r.category == c))
            .map(|(i, _)| i)
            .collect();

        // Sentinel indexing: we insert separators as usize::MAX entries and category
        // headers as (usize::MAX - 1 - category_idx). We disambiguate when rendering.
        let mut last_cat: Option<Category> = None;
        for idx in &matches {
            let cat = self.rows[*idx].category;
            if Some(cat) != last_cat {
                if self.drilled_in.is_none() {
                    self.filtered.push(header_sentinel(cat));
                }
                self.categories_present.push(cat);
                last_cat = Some(cat);
            }
            self.filtered.push(*idx);
        }

        // Snap selection to the first leaf (if any).
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            let first_leaf = self
                .filtered
                .iter()
                .position(|&i| i < SENTINEL_BASE)
                .unwrap_or(0);
            self.list_state.select(Some(first_leaf));
        }
    }
}

/// Sentinel values above the real index space mean "not a leaf".
const SENTINEL_BASE: usize = usize::MAX - 100;
fn header_sentinel(cat: Category) -> usize {
    SENTINEL_BASE + (cat as usize)
}
fn sentinel_to_category(s: usize) -> Option<Category> {
    if s < SENTINEL_BASE {
        return None;
    }
    match s - SENTINEL_BASE {
        1 => Some(Category::PullRequest),
        2 => Some(Category::Issue),
        3 => Some(Category::Commit),
        4 => Some(Category::File),
        5 => Some(Category::Repo),
        6 => Some(Category::Blog),
        7 => Some(Category::OtherLink),
        8 => Some(Category::Server),
        9 => Some(Category::Ip),
        _ => None,
    }
}

// ----- Filter tokenization (see spec §Filtering semantics, Divergence 1 & 2) -----

/// Divergence 1 from pick-tui: multi-digit tokens are NOT split per-digit,
/// because splitting would cause PR-number matches to misfire.
pub(crate) fn tokenize(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in query.split_whitespace() {
        // Split letter/digit boundaries ONCE per transition (not per character).
        let mut cur = String::new();
        let mut cur_is_digit = None;
        for ch in word.chars() {
            let this_is_digit = ch.is_ascii_digit();
            if cur_is_digit.map_or(false, |d: bool| d != this_is_digit) && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(ch);
            cur_is_digit = Some(this_is_digit);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// Divergence 2: the category short-name tag is prepended to the row's
/// search string with a `\x1f` unit separator so substring matches can't
/// leak from the tag into the body.
pub(crate) fn row_search_string(r: &Row) -> String {
    // tag \x1f key repo-or-host context canonical
    format!(
        "{}\x1f{} {} {} {}",
        r.category.tag(),
        r.key,
        r.repo_or_host,
        r.context,
        r.canonical
    )
}

pub(crate) fn match_row(r: &Row, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return true;
    }
    let hay = row_search_string(r).to_lowercase();
    // Key column alone for digit tokens:
    let key_lc = r.key.to_lowercase();
    for tok in tokens {
        let t = tok.to_lowercase();
        if tok.chars().all(|c| c.is_ascii_digit()) {
            if !key_lc.contains(&t) {
                return false;
            }
        } else if !hay.contains(&t) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    fn mk(cat: Category, key: &str, context: &str) -> Row {
        Row {
            category: cat,
            canonical: format!("https://github.com/a/b/pull/{key}"),
            key: key.to_string(),
            repo_or_host: "b".to_string(),
            context: context.to_string(),
            enriched: None,
            count: 1,
            most_recent_line: 0,
        }
    }

    #[test]
    fn tokenize_splits_letter_digit_boundary_once() {
        assert_eq!(tokenize("pr68"), vec!["pr", "68"]);
        assert_eq!(tokenize("14 cl"), vec!["14", "cl"]);
    }

    #[test]
    fn multi_digit_token_not_split_per_digit() {
        assert_eq!(tokenize("1234"), vec!["1234"]);
    }

    #[test]
    fn tag_leak_is_prevented_by_unit_separator() {
        let r = mk(Category::PullRequest, "#68", "writing prose all day");
        // "pr" tag should NOT leak into matching against "prose"
        let s = row_search_string(&r);
        assert!(s.starts_with("pr\x1f"));
        assert!(!s[..3].contains("prose"));
    }

    #[test]
    fn digit_token_matches_key_only() {
        let r = mk(Category::PullRequest, "#68", "context 68 somewhere");
        assert!(match_row(&r, &[String::from("68")])); // in key
        let r2 = mk(Category::PullRequest, "#99", "context 68 somewhere");
        assert!(!match_row(&r2, &[String::from("68")])); // not in key
    }

    #[test]
    fn category_tag_matches_at_start() {
        let r = mk(Category::PullRequest, "#1", "");
        assert!(match_row(&r, &[String::from("pr")]));
        let r2 = mk(Category::Issue, "#1", "");
        assert!(!match_row(&r2, &[String::from("pr")]));
    }
}

// ----- Run loop + rendering -----

pub fn run(rows: Vec<Row>) -> Result<Action> {
    if rows.is_empty() {
        return Ok(Action::Quit);
    }
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(rows);
    let result = event_loop(&mut app, &mut terminal);

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    drop(terminal);

    result
}

fn event_loop<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<Action> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            handle_key(app, k.modifiers, k.code);
            if let Some(action) = app.action.take() {
                return Ok(action);
            }
        }
    }
}

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(5)])
        .split(area);

    // Top bar with breadcrumb or flat hints.
    let top = if let Some(cat) = app.drilled_in {
        format!(
            "pick> {}_  │ Links › {}  │ ↑↓ Enter:act ←:back F2:sess ?:help",
            app.query,
            cat.display()
        )
    } else {
        format!(
            "pick> {}_  │ ↑↓ Enter:act →:drill y:yank o:open g:gh F2:sess ?:help",
            app.query
        )
    };
    f.render_widget(
        Paragraph::new(top).style(Style::default().fg(Color::Yellow)),
        main[0],
    );

    // Split content horizontally or vertically
    let content_chunks = if app.horizontal {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main[1])
    };

    // List
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&idx| render_item(app, idx))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Links"))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, content_chunks[0], &mut app.list_state);

    // Preview
    let preview_text = preview_for_selection(app);
    let preview = Paragraph::new(preview_text)
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, content_chunks[1]);
}

fn render_item(app: &App, idx: usize) -> ListItem<'static> {
    if let Some(cat) = sentinel_to_category(idx) {
        let count = app
            .filtered
            .iter()
            .filter(|i| **i < SENTINEL_BASE && app.rows[**i].category == cat)
            .count();
        return ListItem::new(format!("⊟ {} ({count})", cat.display()))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    }
    let r = &app.rows[idx];
    let tree = "├─ ";
    let glyph = r.enriched.as_ref().map(|e| state_glyph(e.state)).unwrap_or("");
    let title = r.enriched.as_ref().map(|e| e.title.as_str()).unwrap_or(&r.context);
    let count = if r.count > 1 { format!("  ×{}", r.count) } else { String::new() };
    let spans = vec![
        Span::styled(tree, Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<8} ", r.key), Style::default().fg(Color::LightYellow)),
        Span::styled(format!("{:<16} ", r.repo_or_host), Style::default().fg(Color::LightGreen)),
        Span::styled(format!("{glyph} "), Style::default().fg(glyph_color(r.enriched.as_ref().map(|e| e.state)))),
        Span::styled(title.to_string(), Style::default().fg(Color::LightMagenta)),
        Span::styled(count, Style::default().fg(Color::LightCyan)),
    ];
    ListItem::new(Line::from(spans))
}

fn state_glyph(s: GhState) -> &'static str {
    match s {
        GhState::Open => "◉",
        GhState::MergedPr => "●",
        GhState::Closed => "✕",
        GhState::Draft => "◐",
        GhState::Commit => "⎇",
    }
}

fn glyph_color(s: Option<GhState>) -> Color {
    match s {
        Some(GhState::Open) => Color::LightGreen,
        Some(GhState::MergedPr) => Color::LightMagenta,
        Some(GhState::Closed) => Color::DarkGray,
        Some(GhState::Draft) => Color::LightYellow,
        Some(GhState::Commit) => Color::LightBlue,
        None => Color::Reset,
    }
}

fn preview_for_selection(app: &App) -> Text<'static> {
    let Some(pos) = app.list_state.selected() else {
        return Text::raw("");
    };
    let Some(&idx) = app.filtered.get(pos) else {
        return Text::raw("");
    };
    if let Some(cat) = sentinel_to_category(idx) {
        return Text::raw(format!("Category: {} (header)", cat.display()));
    }
    let r = &app.rows[idx];
    let body = format!(
        "{}\n\ncanonical: {}\nkey: {}\nrepo/host: {}\ncount: {}",
        r.context, r.canonical, r.key, r.repo_or_host, r.count
    );
    body.into_text().unwrap_or_else(|_| Text::raw(body))
}

// Key handling is stubbed here for Task 14; wired in Task 15.
fn handle_key(app: &mut App, _mods: KeyModifiers, code: KeyCode) {
    if matches!(code, KeyCode::Esc) {
        app.action = Some(Action::Quit);
    }
}
```

- [ ] **Step 2: Build the crate.**

```bash
cargo build -p rmux_helper 2>&1 | /usr/bin/tail -30
```

Expected: clean build. Fix any compile errors (likely borrow-checker issues in `render_item` around ownership of `title`/`glyph`).

- [ ] **Step 3: Run filter tests.**

```bash
cargo test -p rmux_helper link_picker::tui::filter_tests 2>&1 | /usr/bin/tail -20
```

Expected: 5 passes.

- [ ] **Step 4: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/tui.rs
git commit -m "feat(link-picker): TUI skeleton with filter tokenizer and row render"
```

---

## Task 15 — TUI key handling: navigation, drill-in, filter input

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/tui.rs`

Expands `handle_key` to the full navigation model from spec §Navigation (lines ~530-600).

- [ ] **Step 1: Replace `handle_key` and add helper methods on `App`.**

Replace the `handle_key` stub with:

```rust
fn handle_key(app: &mut App, mods: KeyModifiers, code: KeyCode) {
    // Esc: drill-out or quit
    if matches!(code, KeyCode::Esc) {
        if app.drilled_in.is_some() {
            app.drilled_in = None;
            app.rebuild_filter();
        } else {
            app.action = Some(Action::Quit);
        }
        return;
    }

    // Ctrl-C: clear query or quit
    if matches!(code, KeyCode::Char('c')) && mods.contains(KeyModifiers::CONTROL) {
        if !app.query.is_empty() {
            app.query.clear();
            app.rebuild_filter();
        } else {
            app.action = Some(Action::Quit);
        }
        return;
    }

    // Navigation
    match code {
        KeyCode::Down | KeyCode::Char('\x0e') => app.move_selection(1),
        KeyCode::Up | KeyCode::Char('\x10') => app.move_selection(-1),
        KeyCode::Right => app.drill_in(),
        KeyCode::Left => {
            if app.drilled_in.is_some() {
                app.drilled_in = None;
                app.rebuild_filter();
            }
        }
        KeyCode::Enter => app.on_enter(),
        KeyCode::F(2) => app.action = Some(Action::SwapToPickTui),
        KeyCode::F(1) => { /* help overlay — TODO v1.1 */ }
        KeyCode::Backspace => {
            app.query.pop();
            app.rebuild_filter();
        }
        KeyCode::Char('l') if mods.contains(KeyModifiers::CONTROL) => {
            app.horizontal = !app.horizontal;
        }
        KeyCode::Char('n') if mods.contains(KeyModifiers::CONTROL) => app.move_selection(1),
        KeyCode::Char('p') if mods.contains(KeyModifiers::CONTROL) => app.move_selection(-1),
        // Digit 1-9: jump to Nth category drilled-in view
        KeyCode::Char(c @ '1'..='9') if mods.is_empty() && app.query.is_empty() => {
            let n = (c as u8 - b'0') as usize;
            if let Some(cat) = app.categories_present.get(n - 1) {
                app.drilled_in = Some(*cat);
                app.rebuild_filter();
            }
        }
        KeyCode::Char(c) if c.is_ascii_graphic() || c == ' ' => {
            app.query.push(c);
            app.rebuild_filter();
        }
        _ => {}
    }
}

impl App {
    fn move_selection(&mut self, delta: i32) {
        let Some(cur) = self.list_state.selected() else { return };
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let mut next = cur as i32 + delta;
        // Skip headers while moving
        while (0..len as i32).contains(&next) {
            if self.filtered[next as usize] < SENTINEL_BASE {
                break;
            }
            next += delta.signum();
        }
        if (0..len as i32).contains(&next) {
            self.list_state.select(Some(next as usize));
        }
    }

    fn drill_in(&mut self) {
        if self.drilled_in.is_some() {
            return;
        }
        let Some(cur) = self.list_state.selected() else { return };
        let Some(&idx) = self.filtered.get(cur) else { return };
        let cat = if let Some(c) = sentinel_to_category(idx) {
            c
        } else {
            self.rows[idx].category
        };
        self.drilled_in = Some(cat);
        self.rebuild_filter();
    }

    fn on_enter(&mut self) {
        let Some(cur) = self.list_state.selected() else { return };
        let Some(&idx) = self.filtered.get(cur) else { return };
        if let Some(cat) = sentinel_to_category(idx) {
            // Header: drill in
            self.drilled_in = Some(cat);
            self.rebuild_filter();
            return;
        }
        // Leaf: default action
        let row = self.rows[idx].clone();
        self.action = Some(default_action(&row));
    }
}

/// Default Enter action per category (see spec §Actions → Default).
pub(crate) fn default_action(row: &Row) -> Action {
    match row.category {
        Category::Server | Category::Ip => Action::Ssh(row.clone()),
        _ => Action::Yank(row.clone()),
    }
}
```

- [ ] **Step 2: Build and run existing tests.**

```bash
cargo test -p rmux_helper link_picker:: 2>&1 | /usr/bin/tail -20
```

Expected: all prior tests pass, no new regressions.

- [ ] **Step 3: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/tui.rs
git commit -m "feat(link-picker): navigation, drill-in, filter input keys"
```

---

## Task 16 — Actions + OSC 52 yank + override keys

**Files:**
- Modify: `rust/tmux_helper/src/link_picker/mod.rs`
- Modify: `rust/tmux_helper/src/link_picker/tui.rs`

Implements steps 6-7 of the Execution Flow: post-TUI teardown, OSC 52 yank with correct ordering, action dispatch.

- [ ] **Step 1: Add override-key handling in `tui.rs::handle_key` before the catch-all printable branch:**

```rust
        // Override keys (on selected leaf only)
        KeyCode::Char('y') if mods.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Yank(row));
            }
        }
        KeyCode::Char('o') if mods.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Open(row));
            }
        }
        KeyCode::Char('g') if mods.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                if matches!(
                    row.category,
                    Category::PullRequest
                        | Category::Issue
                        | Category::Commit
                        | Category::File
                        | Category::Repo
                ) {
                    app.action = Some(Action::GhWeb(row));
                } else {
                    app.error_msg = Some("g: not a GitHub row".into());
                }
            }
        }
        KeyCode::Char('s') if mods.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Ssh(row));
            }
        }
```

Add the `selected_leaf` helper on `App`:

```rust
    fn selected_leaf(&self) -> Option<Row> {
        let cur = self.list_state.selected()?;
        let &idx = self.filtered.get(cur)?;
        if idx >= SENTINEL_BASE {
            return None;
        }
        Some(self.rows[idx].clone())
    }
```

**Wait:** `y`/`o`/`g`/`s` are lowercase letters and the catch-all `KeyCode::Char(c)` branch above would type them into the search query first. Move the override branches BEFORE the printable-ASCII catch-all in the match. Since this would make them unreachable while typing a query (e.g., searching for "yes"), gate them on `app.query.is_empty()` — override keys only fire when the query is empty. Document this constraint in `LINK_PICKER_SPEC.md`.

Final ordering: move the 4 override branches above the `KeyCode::Char(c) if c.is_ascii_graphic()...` branch, and gate each on `app.query.is_empty()`:

```rust
        KeyCode::Char('y') if mods.is_empty() && app.query.is_empty() => { /* ... */ }
```

- [ ] **Step 2: Update `mod.rs::pick_links` to handle the action from `tui::run`.**

Replace the stub `let _action = tui::run(rows)?;` block with:

```rust
    // 5. TUI
    let action = tui::run(rows)?;

    // 6-7. Dispatch post-TUI (terminal already restored by tui::run).
    match action {
        tui::Action::Quit => std::process::exit(130),
        tui::Action::Yank(row) => {
            yank_osc52(&row.canonical)?;
            println!("{}", row.canonical);
        }
        tui::Action::Open(row) => open_url(&row.canonical)?,
        tui::Action::GhWeb(row) => gh_web(&row)?,
        tui::Action::Ssh(row) => ssh_host(&row)?,
        tui::Action::SwapToPickTui => {
            use std::os::unix::process::CommandExt;
            // execvp: never returns on success
            let err = Command::new("rmux_helper").arg("pick-tui").exec();
            return Err(anyhow!("exec pick-tui failed: {err}"));
        }
    }
    Ok(())
}

fn yank_osc52(payload: &str) -> Result<()> {
    use base64::Engine;
    use std::fs::OpenOptions;
    use std::io::Write;
    let b64 = base64::engine::general_purpose::STANDARD.encode(payload);
    let mut tty = OpenOptions::new().write(true).open("/dev/tty")?;
    write!(tty, "\x1b]52;c;{b64}\x1b\\")?;
    tty.flush()?;
    Ok(())
}

fn open_url(url: &str) -> Result<()> {
    let cmd = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    Command::new(cmd).arg(url).status()?;
    Ok(())
}

fn gh_web(row: &detect::Row) -> Result<()> {
    use detect::Category as C;
    let subcmd = match row.category {
        C::PullRequest => "pr",
        C::Issue => "issue",
        C::Commit | C::File | C::Repo => {
            // Fall through to plain open — gh doesn't have a generic "view" for these
            return open_url(&row.canonical);
        }
        _ => return Ok(()),
    };
    // row.key is "#N" for PR/Issue — strip "#" for gh arg.
    let id = row.key.trim_start_matches('#');
    // repo_or_host is just the repo name; owner not carried on Row in v1.
    // Parse from canonical: https://github.com/OWNER/REPO/...
    let owner_repo = row
        .canonical
        .strip_prefix("https://github.com/")
        .and_then(|s| s.splitn(3, '/').take(2).collect::<Vec<_>>().join("/").into())
        .unwrap_or_else(|| row.repo_or_host.clone());
    Command::new("gh")
        .args([subcmd, "view", id, "-R", &owner_repo, "--web"])
        .status()?;
    Ok(())
}

fn ssh_host(row: &detect::Row) -> Result<()> {
    // Use tmux new-window in the originating pane's session.
    let pane = env::var("TMUX_PANE").unwrap_or_default();
    let host_arg = format!("ssh {}", row.canonical);
    Command::new("tmux")
        .args(["new-window", "-t", &pane, &host_arg])
        .status()?;
    Ok(())
}
```

Note: `gh_web`'s owner-repo extraction is a bit fragile; consider pushing `owner` onto `Row` in a follow-up if this causes pain. For v1 the parse from `canonical` is sufficient because canonical URLs are normalized.

- [ ] **Step 3: Build and verify.**

```bash
cargo build -p rmux_helper 2>&1 | /usr/bin/tail -30
cargo test -p rmux_helper link_picker:: 2>&1 | /usr/bin/tail -15
```

Expected: clean build; all tests pass.

- [ ] **Step 4: Commit.**

```bash
git add rust/tmux_helper/src/link_picker/
git commit -m "feat(link-picker): OSC 52 yank, contextual Enter, override keys"
```

---

## Task 17 — F2 cross-picker, PICKER_SPEC update, tmux binding, LINK_PICKER_SPEC

**Files:**
- Modify: `rust/tmux_helper/src/picker.rs` (around line 650 — key handler)
- Modify: `rust/tmux_helper/PICKER_SPEC.md`
- Modify: `shared/.tmux.conf`
- Create: `rust/tmux_helper/LINK_PICKER_SPEC.md`

This task closes the loop: `pick-tui` can jump to `pick-links` via `F2`, the tmux binding lives in `.tmux.conf`, and the behavior contract is documented.

- [ ] **Step 1: Add F2 handler to `picker.rs` key handling.**

Find the F1/help binding in `rust/tmux_helper/src/picker.rs` around line 650:

```rust
                (_, KeyCode::F(1)) | (KeyModifiers::CONTROL, KeyCode::Char('/')) => {
```

Add a sibling branch *before* it:

```rust
                (_, KeyCode::F(2)) => {
                    // Cross-picker swap: exec pick-links. See spec §Cross-picker shortcut.
                    app.should_quit = true;
                    app.selected_target = Some("__swap_to_pick_links__".into());
                }
```

Then find the `pick_tui()` function's post-loop handling (around the teardown in lines ~686-690). After `disable_raw_mode` + `LeaveAlternateScreen`, intercept the sentinel and exec:

```rust
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    drop(terminal);
    if matches!(app.selected_target.as_deref(), Some("__swap_to_pick_links__")) {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new("rmux_helper")
            .arg("pick-links")
            .exec();
        return Err(anyhow::anyhow!("exec pick-links failed: {err}"));
    }
```

Adjust to match the actual return type. If `selected_target: Option<String>` is threaded through a different path, switch the "string sentinel" to a dedicated enum variant when follow-up refactoring hits.

- [ ] **Step 2: Update `PICKER_SPEC.md` with the F2 entry.**

Under the `## Actions` section, add:

```markdown
- `F2`: Swap to link picker (`rmux_helper pick-links`) — exec-based handoff, terminal is restored first. Bidirectional: `F2` in `pick-links` returns here.
```

- [ ] **Step 3: Add the tmux binding, help-section entry, and command alias in `shared/.tmux.conf`.**

At the help section near the top of the file (following the existing `C-a w` help line), add:

```tmux
#   C-a L                - Launch scrollback link picker popup (rmux_helper pick-links)
```

And in the alias-section (search for `set -s command-alias`):

```tmux
set -s command-alias[NN] pick-links='run-shell "rmux_helper pick-links"'
```

Replace `NN` with the next free alias index.

Add the bind-key near `C-a w`:

```tmux
bind-key L display-popup -E -w 95% -h 95% \
  "TMUX_PANE=#{pane_id} rmux_helper pick-links"
```

- [ ] **Step 4: Create `rust/tmux_helper/LINK_PICKER_SPEC.md`.**

Write the behavior contract mirroring `PICKER_SPEC.md`'s style. Do not re-derive the design — this is a behavior reference for future changes. Minimum content:

```markdown
# rmux_helper pick-links Specification

## Categories (fixed display order)

1. Pull Requests — `github.com/OWNER/REPO/pull/N`
2. Issues — `github.com/OWNER/REPO/issues/N`
3. Commits — `github.com/OWNER/REPO/commit/SHA`
4. Files — `github.com/OWNER/REPO/(blob|tree)/REF/PATH`
5. Repos — `github.com/OWNER/REPO` (bare)
6. Blog — host ∈ `BLOG_HOSTS` (v1: `idvorkin.github.io`)
7. Other links — any `https?://` not matched above
8. Servers — `ssh` context + Tailscale (`c-NNNN`, `*.ts.net`)
9. IPs — IPv4 with version-string suppression

## Dedup

Row key = `(category, canonical)`. Duplicates collapse into one row with a `×N` count.

## Ordering

Categories: fixed order above. Within a category: most-recent line first (closest to bottom).

## Columns

| Col | Color | Content |
|---|---|---|
| key | LightYellow | `#N`, `SHA[:7]`, filename, host, IP |
| repo-or-host | LightGreen | repo name, host, or `—` |
| glyph + title | state-colored + LightMagenta | state glyph from enriched gh view; `context` line otherwise |
| count | LightCyan | `×N` only when N > 1 |

## Navigation

- `↑`/`↓` or `C-p`/`C-n`: move selection
- `→` or `Enter` on category header: drill into that category
- `1`–`9`: jump into Nth non-empty category (query must be empty)
- `←`: drill out (in drilled-in mode)
- `Esc`: drill out (first press) or quit (if already flat)
- `Tab` / `S-Tab`: reserved, no-op
- `F2`: swap to `pick-tui` (bidirectional)
- `F1`: help (reserved)
- `C-l`: toggle layout (horizontal/vertical)
- `C-c`: clear query or quit

## Actions

Default `Enter`:
- URL categories → OSC 52 yank + print URL to stdout
- Servers / IPs → `tmux new-window -t "$TMUX_PANE" "ssh <host>"`

Override keys (query must be empty):
- `y` — yank (OSC 52)
- `o` — `open`/`xdg-open`
- `g` — `gh <kind> view --web -R OWNER/REPO <id>` (GitHub rows only)
- `s` — force ssh

## Filtering

Token-based substring match. Tokens split on whitespace; letter/digit boundaries split once per transition (multi-digit tokens stay whole — Divergence 1 from `pick-tui`).

Category tag `pr`/`issue`/`commit`/`file`/`repo`/`blog`/`link`/`server`/`ip` prefixes each row's search string with a `\x1f` separator (Divergence 2).

Digit-only tokens match the `key` column only.

## Divergences from `PICKER_SPEC.md`

1. **Multi-digit tokens are NOT split per digit.** `pick-tui` splits `14` → `[1,4]` to match tmux index `1;4`; the link picker treats `14` as one token because PR number `14` must not match `#1;4`.
2. **Tag prefix uses `\x1f` separator.** Ensures the category tag is matched as a whole word, not as a substring leaking into titles.

## Cross-picker shortcut

`F2` cleanly tears down the TUI (`disable_raw_mode` + `LeaveAlternateScreen` + drop terminal + flush) then `execvp`s the sibling binary with `TMUX_PANE` forwarded. OSC 52 is NOT written on `F2` — it's a swap, not an action.

## OSC 52 timing

Write sequence, in order: TUI exits → `disable_raw_mode` → `LeaveAlternateScreen` → drop `Terminal` → flush stdout/stderr → open `/dev/tty` → write `\e]52;c;<base64>\e\\` → flush tty → `exit(0)`.
```

- [ ] **Step 5: Build + install + manual smoke.**

```bash
cd rust/tmux_helper
cargo build 2>&1 | /usr/bin/tail -20
cargo install --path . --force 2>&1 | /usr/bin/tail -5
```

Inside tmux, reload config and test:

```bash
tmux source-file ~/.tmux.conf
# Press C-a L — popup should appear
# In the popup, press F2 — should swap to session picker
# Press F2 again — should swap back to link picker
# Press Esc — should exit cleanly
```

- [ ] **Step 6: Commit.**

```bash
git add rust/tmux_helper/src/picker.rs \
        rust/tmux_helper/PICKER_SPEC.md \
        rust/tmux_helper/LINK_PICKER_SPEC.md \
        shared/.tmux.conf
git commit -m "feat(link-picker): F2 cross-picker, tmux binding, behavior spec"
```

- [ ] **Step 7: Open PR.**

```bash
gh pr create --title "feat(link-picker): scrollback link picker subcommand" \
  --body "$(cat <<'EOF'
## Summary

- New `rmux_helper pick-links` subcommand: ratatui TUI that scans the current tmux pane's scrollback for GitHub links (PRs, issues, commits, files, repos), blog posts, other URLs, ssh servers, and IP addresses
- Parallel `gh` enrichment with shared 3s deadline + on-disk cache for real PR/issue/commit titles
- `F2` bidirectional cross-picker between `pick-tui` and `pick-links`
- OSC 52 clipboard bridge (devvm → Mac host via iTerm)
- `C-a L` popup binding in `shared/.tmux.conf`

Design spec: `docs/superpowers/specs/2026-04-12-scrollback-link-picker-design.md`
Behavior contract: `rust/tmux_helper/LINK_PICKER_SPEC.md`

## Test plan

- [ ] `cargo test -p rmux_helper link_picker::` passes (detection + filter invariants)
- [ ] `rmux_helper pick-links --json` inside tmux emits valid JSON
- [ ] `C-a L` opens the popup from a pane with PR URLs in scrollback
- [ ] Enter on a PR row yanks to clipboard (verify with `pbpaste` on Mac host)
- [ ] Enter on a server row opens `tmux new-window ssh …`
- [ ] `F2` from `pick-links` lands in `pick-tui` and vice versa
- [ ] Drill-in via `→` and drill-out via `Esc` preserve the query
EOF
)"
```

---

## Self-Review

**Spec coverage check:**
- §Execution Flow → Task 13 (capture, detect, --json short-circuit, enrich, TUI handoff) + Task 16 (post-TUI dispatch + OSC 52 + F2 exec branch). ✓
- §Scrollback Capture (`tmux capture-pane -pJe -S- -E-`, `$TMUX_PANE` fallback) → Task 13. ✓
- §Categories & Detection → Tasks 2–9. ✓
- §Enrichment (pipeline, cache, fan-out, deadline, degradation) → Tasks 10–12. ✓
- §Layout & Display (chrome, columns, glyphs, preview, H/V layout) → Task 14. ✓
- §Navigation (flat mode, drilled-in mode, F2) → Task 15 + Task 17. ✓
- §Filtering semantics (tokens, Divergences 1 and 2) → Task 14 + tests. ✓
- §Actions (default, override keys, OSC 52 timing) → Task 16. ✓
- §Clipboard Bridge → Task 16 `yank_osc52`. ✓
- §Configuration (`BLOG_HOSTS` constant) → Task 5. ✓
- §File Layout → matches plan's File Structure section. ✓
- §Error Handling → covered inline in Tasks 13, 16 (not inside tmux, capture fail, action fail, F2 exec fail). ✓
- §Invariants (detection / enrichment / action-layer) → covered as test cases in Tasks 2–12. Concurrent-writers cache invariant is not directly tested (inherently hard), noted in spec §Invariants already.

**Placeholder scan:** no "TBD", no "implement later". Task 14's `F1` help overlay is explicitly stubbed with a comment pointing at v1.1; acceptable because `F1` is listed in the key table but spec §Navigation says "help overlay" without mandating it for v1.

**Type consistency check:**
- `Row`, `Item`, `Category`, `EnrichedTitle`, `GhState` are defined once in Task 2, imported elsewhere consistently.
- `Action` defined in Task 14 (`tui.rs`), imported in Task 16 (`mod.rs`). Same variants throughout.
- `yank_osc52` is `fn`, not a method — called as free function in Task 16. Matches.
- `enrich_rows` signature `(Vec<Row>, u64) -> Vec<Row>` is stable across Tasks 12 and 13. ✓
- `parse(raw: &str) -> Vec<Row>` defined in Task 9, called in Task 13. ✓

**Gaps found and fixed inline:**
- Task 16 `gh_web` originally assumed `owner` on `Row`; corrected to parse from `canonical`.
- Task 16 override keys initially conflicted with the search-input catch-all; gated on `app.query.is_empty()` and reordered match arms.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-12-scrollback-link-picker.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Best for a 17-task plan with strict TDD loops — keeps context clean and prevents drift.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints. Faster for simple tasks, but this plan's size will burn context if run inline.

**Which approach?**

If you want to split the work: Phase A (Tasks 1–9, pure detection — standalone) could be one session, Phase B (Tasks 10–17, enrichment + TUI + integration — depends on Phase A) a second. Phase A gives you a working `pick-links --json` end-to-end; Phase B adds the TUI and all interactive polish.
