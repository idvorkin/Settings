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

    #[cfg(test)]
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
    pub canonical: String,    // canonical URL, hostname, or IP
    pub key: String,          // key column: "#68", "a22bc17", "picker.rs:L42", "c-5001"
    pub repo_or_host: String, // repo-or-host column: "settings", "idvorkin.github.io", "—"
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
    pub context: String, // from scrollback line at most_recent_line
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
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

/// Base URL regex — matches scheme + RFC-3986 unreserved/reserved ASCII chars.
/// Deliberately positive (allowlist), not exclusion-based, so:
///   - `\` (backslash) is excluded → escape sequences in scrollback source
///     code like `"pull/1\nssh"` don't leak `\n` into canonicals.
///   - Non-ASCII chars (e.g. `…` U+2026 from terminal truncation) are excluded.
///   - `(`, `)`, `'`, `"`, `<`, `>`, brackets, braces are excluded so URLs
///     embedded in prose (`(see https://.../)`) terminate cleanly.
/// Trailing punctuation is stripped separately (see `strip_trailing_punct`).
pub(crate) fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"https?://[A-Za-z0-9\-._~:/?#@!$&*+,;=%]+").unwrap())
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

    #[test]
    fn excludes_backslash_from_url() {
        // Rust test source often embeds `\n` as two literal characters
        // (backslash + n) in string literals that end up in scrollback.
        // The URL regex must not treat `\` as a URL character, or canonicals
        // leak escaped-newline fragments like `pull/1\nssh`.
        let line = r#"let raw = "https://github.com/a/b/pull/1\nssh c-5001\n";"#;
        let m = url_regex().find(line).expect("URL still present");
        let extracted = strip_trailing_punct(m.as_str());
        assert_eq!(extracted, "https://github.com/a/b/pull/1");
        assert!(!extracted.contains('\\'));
    }

    #[test]
    fn excludes_non_ascii_ellipsis_from_url() {
        // Terminals sometimes truncate URLs with `…` (U+2026) when wrapping.
        // The regex must not include non-ASCII characters, or the truncated
        // `…` leaks into the canonical and duplicates rows.
        let line = "see https://idvorkin.github.io/posts/pro… for more";
        let m = url_regex().find(line).expect("URL still present");
        let extracted = strip_trailing_punct(m.as_str());
        assert_eq!(extracted, "https://idvorkin.github.io/posts/pro");
        assert!(!extracted.contains('…'));
    }
}

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
                              // Require a file path after the ref. `blob/main` alone is a
                              // branch-tree view, not a file; line-wrapped scrollback produces
                              // truncated `blob/m` cases which must also be rejected.
            if !tail.contains('/') {
                return None;
            }
            let basename = tail.rsplit('/').next().unwrap_or(tail);
            // Line anchor: #L42 or #L42-L60 preserves only Lstart in key.
            let key = if let Some(l) = fragment.strip_prefix("#L") {
                let lstart = l.split('-').next().unwrap_or(l);
                format!("{basename}:L{lstart}")
            } else {
                basename.to_string()
            };
            let kind_str = kind.unwrap();
            let frag_out = if fragment.starts_with("#L") {
                fragment
            } else {
                ""
            };
            Some(Item {
                category: Category::File,
                canonical: format!("{canonical_base}/{kind_str}/{tail}{frag_out}"),
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
        let item =
            classify_github("https://github.com/idvorkin-ai-tools/settings/pull/68", 0).unwrap();
        assert_eq!(item.category, Category::PullRequest);
        assert_eq!(item.key, "#68");
        assert_eq!(item.repo_or_host, "settings");
        assert_eq!(
            item.canonical,
            "https://github.com/idvorkin-ai-tools/settings/pull/68"
        );
    }

    #[test]
    fn strips_pr_discussion_fragment() {
        let item = classify_github("https://github.com/a/b/pull/1#discussion_r123", 0).unwrap();
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
        let item =
            classify_github("https://github.com/a/b/blob/main/src/picker.rs#L42", 0).unwrap();
        assert_eq!(item.category, Category::File);
        assert_eq!(item.key, "picker.rs:L42");
    }

    #[test]
    fn classifies_file_line_range_uses_start() {
        let item =
            classify_github("https://github.com/a/b/blob/main/src/picker.rs#L42-L60", 0).unwrap();
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

    #[test]
    fn rejects_blob_without_path() {
        // `blob/main` (or any truncated `blob/X` with no file path after the
        // ref) is not a File URL — it's a branch/tree view. Line-wrapped
        // scrollback produces cases like `blob/m` where the URL was cut off.
        // classify_github must return None so these don't pollute the Files
        // category; they fall through to OtherLink.
        assert!(classify_github("https://github.com/a/b/blob/main", 0).is_none());
        assert!(classify_github("https://github.com/a/b/blob/m", 0).is_none());
        assert!(classify_github("https://github.com/a/b/tree/main", 0).is_none());
    }
}

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

pub(crate) fn find_servers(line: &str, line_index: usize) -> Vec<Item> {
    static SSH_RE: OnceLock<Regex> = OnceLock::new();
    static TS_HOST_RE: OnceLock<Regex> = OnceLock::new();
    static TS_NET_RE: OnceLock<Regex> = OnceLock::new();
    let ssh = SSH_RE.get_or_init(|| {
        // `ssh` token, optional flags (with optional value args for short opts), optional user@, then host
        Regex::new(
            r"\bssh\b(?:\s+(?:-[a-zA-Z]\s+\S+|-\S+))*\s+(?:[a-zA-Z_][\w-]*@)?([a-zA-Z0-9][\w.\-]*[a-zA-Z0-9])\b",
        )
        .unwrap()
    });
    let ts_host = TS_HOST_RE.get_or_init(|| Regex::new(r"\bc-\d{4,5}\b").unwrap());
    let ts_net = TS_NET_RE.get_or_init(|| Regex::new(r"\b[a-z][a-z0-9-]*\.ts\.net\b").unwrap());

    let mut out = Vec::new();
    for cap in ssh.captures_iter(line) {
        if let Some(h) = cap.get(1) {
            let host = h.as_str();
            // Suppress bare English words after the `ssh` verb ("URLs and
            // ssh servers and IPs" → not a real host). Require the host to
            // either contain a `.` (FQDN / dotted form) or match the
            // Tailscale short form `c-NNNN`. Config-alias hosts without a
            // dot are accepted only when they also appear in the separate
            // Tailscale regexes below.
            if host.contains('.') || ts_host.is_match(host) {
                out.push(mk_server(host, line_index));
            }
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

    #[test]
    fn rejects_bare_english_word_after_ssh_verb() {
        // Prose like "URLs, ssh servers, IPs" or "scrapes ssh in the docs"
        // must NOT produce servers named `servers`, `for`, `context`, etc.
        // Require the host to look like a real hostname: contain `.` or
        // match the Tailscale short form.
        assert!(find_servers("URLs and ssh servers and IPs", 0).is_empty());
        assert!(find_servers("- scrapes scrollback for ssh for context", 0).is_empty());
        assert!(find_servers("when ssh lands in the pane", 0).is_empty());
        assert!(find_servers("force ssh action on any row", 0).is_empty());
    }

    #[test]
    fn still_extracts_dotted_host_after_ssh() {
        // Regression guard for the fix above: dotted hostnames must still
        // be picked up.
        let items = find_servers("ssh dev.example.com", 0);
        assert!(items.iter().any(|i| i.canonical == "dev.example.com"));
    }
}

pub(crate) fn find_ips(line: &str, line_index: usize) -> Vec<Item> {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Dotted-quad with bounded octets, no look-around (Rust regex limitation).
    // We do boundary + version-prefix checks in code.
    let re = RE.get_or_init(|| Regex::new(r"(?:\d{1,3}\.){3}\d{1,3}").unwrap());

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
        let octets: Vec<u16> = text
            .split('.')
            .map(|s| s.parse().unwrap_or(u16::MAX))
            .collect();
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
        let items = scan_line(
            "Merge pull request #68 https://github.com/a/b/pull/68 ok",
            0,
        );
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
        (
            a.category as u8,
            std::cmp::Reverse(a.most_recent_line),
            &a.canonical,
        )
            .cmp(&(
                b.category as u8,
                std::cmp::Reverse(b.most_recent_line),
                &b.canonical,
            ))
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
        let servers: Vec<&Row> = rows
            .iter()
            .filter(|r| r.category == Category::Server)
            .collect();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].canonical, "c-5001");
        assert!(servers[0].count >= 3);
    }

    #[test]
    fn orders_by_recency_within_category() {
        let raw = "https://github.com/a/b/pull/1\nhttps://github.com/a/b/pull/2\n";
        let rows = parse(raw);
        let prs: Vec<&Row> = rows
            .iter()
            .filter(|r| r.category == Category::PullRequest)
            .collect();
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
        let pr = rows
            .iter()
            .find(|r| r.category == Category::PullRequest)
            .unwrap();
        assert!(!pr.context.contains("https://"));
        assert!(!pr.context.contains("   ")); // no runs of whitespace
        assert!(pr.context.contains("Merge pull request"));
    }

    #[test]
    fn context_truncates_with_ellipsis() {
        let long = "a".repeat(200);
        let raw = format!("{long} https://example.com rest\n");
        let rows = parse(&raw);
        let row = rows
            .iter()
            .find(|r| r.category == Category::OtherLink)
            .unwrap();
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
