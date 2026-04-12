//! Pure scrollback detection. No I/O, no async.

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
    s.trim_end_matches(|c: char| ".,;:!?)]}>'\"".contains(c))
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
