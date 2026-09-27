//! Heuristic resolution of GitHub short refs (`#14`, `chop#14`,
//! `owner/repo#14`) for herdr, whose `pane read` drops OSC 8 link targets.
//! Under tmux the real target comes from the hyperlink (see `flatten_ansi`).
//!
//! ponytail: this guesses the repo. Delete it once herdr's `pane read`
//! returns OSC 8 targets (herdrdev/herdr discussion #4235).

use super::detect::{
    classify_github, is_github_noise_url, strip_trailing_punct, url_regex, Category, Item,
};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Where a short ref's repo can come from besides the scrollback itself.
pub(crate) struct RefContext {
    /// `owner/repo` of the pane's working directory, if it is a GitHub repo.
    pub cwd_repo: Option<String>,
    /// Directory of local checkouts (`~/gits`), matched by name for `name#N`.
    pub gits_root: Option<PathBuf>,
}

/// Words people put before `#N` that are not repo names (`PR#14`).
const NOT_REPOS: &[&str] = &[
    "pr", "prs", "pull", "issue", "issues", "gh", "bug", "fix", "see",
];

/// Short-ref scanner for one capture: repo hints plus the repos and PR
/// numbers the capture's full URLs name.
pub(crate) struct ShortRefs {
    ctx: RefContext,
    seen: ScrollbackRepos,
}

fn short_ref_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:([A-Za-z0-9][\w.\-]*(?:/[A-Za-z0-9][\w.\-]*)?))?#(\d{1,6})\b").unwrap()
    })
}

impl ShortRefs {
    pub(crate) fn new(raw: &str, ctx: RefContext) -> Self {
        Self {
            ctx,
            seen: ScrollbackRepos::scan(raw),
        }
    }

    /// Items for the short refs in `line` whose repo resolves. Links to
    /// `/issues/N` unless the capture shows `/pull/N` for that repo — GitHub
    /// redirects an issues URL to the PR when N is a PR.
    pub(crate) fn scan(&self, line: &str, line_index: usize) -> Vec<Item> {
        let url_spans: Vec<(usize, usize)> = url_regex()
            .find_iter(line)
            .map(|m| (m.start(), m.end()))
            .collect();
        let mut out = Vec::new();
        for caps in short_ref_regex().captures_iter(line) {
            let m = caps.get(0).unwrap();
            if url_spans.iter().any(|&(s, e)| m.start() < e && s < m.end()) {
                continue;
            }
            // `&#123;`, `a/#1`, `x##1` and mid-word matches are not refs.
            if line[..m.start()]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || "/#&_.-".contains(c))
            {
                continue;
            }
            let name = caps
                .get(1)
                .map(|n| n.as_str())
                .filter(|n| !NOT_REPOS.contains(&n.to_ascii_lowercase().as_str()));
            let num = &caps[2];
            let Some(repo) = resolve_repo(name, &self.ctx, &self.seen) else {
                continue;
            };
            let kind = if self
                .seen
                .prs
                .contains(&(repo.to_ascii_lowercase(), num.to_string()))
            {
                "pull"
            } else {
                "issues"
            };
            let url = format!("https://github.com/{repo}/{kind}/{num}");
            out.extend(classify_github(&url, line_index));
        }
        out
    }
}

/// GitHub repos and PR numbers named by full URLs in the scrollback.
#[derive(Default)]
struct ScrollbackRepos {
    /// `owner/repo`, first-seen order, deduplicated case-insensitively.
    repos: Vec<String>,
    /// Lowercased `owner/repo` and PR number.
    prs: HashSet<(String, String)>,
}

impl ScrollbackRepos {
    fn scan(raw: &str) -> Self {
        let mut me = Self::default();
        for m in url_regex().find_iter(raw) {
            let url = strip_trailing_punct(m.as_str());
            if is_github_noise_url(url) {
                continue;
            }
            let Some(item) = classify_github(url, 0) else {
                continue;
            };
            let Some(repo) = item
                .canonical
                .strip_prefix("https://github.com/")
                .map(|s| s.splitn(3, '/').take(2).collect::<Vec<_>>().join("/"))
            else {
                continue;
            };
            if item.category == Category::PullRequest {
                let num = item.key.trim_start_matches('#').to_string();
                me.prs.insert((repo.to_ascii_lowercase(), num));
            }
            if !me.repos.iter().any(|r| r.eq_ignore_ascii_case(&repo)) {
                me.repos.push(repo);
            }
        }
        me
    }
}

fn resolve_repo(name: Option<&str>, ctx: &RefContext, seen: &ScrollbackRepos) -> Option<String> {
    let Some(name) = name else {
        // Bare `#N`: the pane's repo, else the only repo the scrollback names.
        return ctx
            .cwd_repo
            .clone()
            .or_else(|| match seen.repos.as_slice() {
                [only] => Some(only.clone()),
                _ => None,
            });
    };
    if name.contains('/') {
        return Some(name.to_string());
    }
    let lower = name.to_ascii_lowercase();
    let repo_name = |r: &String| r.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    let mut candidates: Vec<&String> = Vec::new();
    for r in ctx.cwd_repo.iter().chain(seen.repos.iter()) {
        if !candidates.iter().any(|c| c.eq_ignore_ascii_case(r)) {
            candidates.push(r);
        }
    }
    if let Some(r) = unique(candidates.iter().filter(|r| repo_name(r) == lower)) {
        return Some((*r).clone());
    }
    if let Some(r) = unique(
        candidates
            .iter()
            .filter(|r| repo_name(r).starts_with(&lower)),
    ) {
        return Some((*r).clone());
    }
    resolve_from_gits(&lower, ctx.gits_root.as_deref()?)
}

/// `name` → the GitHub repo of `~/gits/<dir>`, exact dir name first, then a
/// unique prefix. Ambiguous prefixes (`chop` → chop-conventions, chop-mcp)
/// resolve to nothing rather than a guess.
fn resolve_from_gits(lower: &str, root: &Path) -> Option<String> {
    let dirs: Vec<String> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join(".git").is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let dir = dirs
        .iter()
        .find(|d| d.to_ascii_lowercase() == lower)
        .or_else(|| {
            unique(
                dirs.iter()
                    .filter(|d| d.to_ascii_lowercase().starts_with(lower)),
            )
        })?;
    let config = std::fs::read_to_string(root.join(dir).join(".git/config")).ok()?;
    github_repo_from_remote(origin_url(&config)?)
}

fn unique<T>(mut it: impl Iterator<Item = T>) -> Option<T> {
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

/// The `url` of `[remote "origin"]` in a git config file.
fn origin_url(config: &str) -> Option<&str> {
    let mut in_origin = false;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            in_origin = line == r#"[remote "origin"]"#;
        } else if in_origin {
            if let Some(url) = line.strip_prefix("url").map(str::trim_start) {
                return url.strip_prefix('=').map(str::trim);
            }
        }
    }
    None
}

/// `owner/repo` from a GitHub remote URL (https or ssh form).
pub(crate) fn github_repo_from_remote(url: &str) -> Option<String> {
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))?;
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let (owner, repo) = path.split_once('/')?;
    (!owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
        .then(|| format!("{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(cwd: Option<&str>) -> RefContext {
        RefContext {
            cwd_repo: cwd.map(String::from),
            gits_root: None,
        }
    }

    /// Canonical URLs of the short refs in the last line of `raw`.
    fn refs(raw: &str, ctx: RefContext) -> Vec<String> {
        let r = ShortRefs::new(raw, ctx);
        r.scan(raw.lines().last().unwrap(), 0)
            .into_iter()
            .map(|i| i.canonical)
            .collect()
    }

    #[test]
    fn bare_ref_uses_pane_repo() {
        assert_eq!(
            refs("merged #14 today", ctx(Some("idvorkin/settings"))),
            ["https://github.com/idvorkin/settings/issues/14"]
        );
    }

    #[test]
    fn bare_ref_falls_back_to_only_scrollback_repo() {
        let raw = "https://github.com/o/r/pull/3\nfixed in #14";
        assert_eq!(refs(raw, ctx(None)), ["https://github.com/o/r/issues/14"]);
    }

    #[test]
    fn bare_ref_with_no_repo_is_skipped() {
        assert!(refs("item #14", ctx(None)).is_empty());
    }

    #[test]
    fn named_ref_matches_scrollback_repo_by_prefix() {
        let raw = "see https://github.com/idvorkin/chop-conventions\nchop#14";
        assert_eq!(
            refs(raw, ctx(None)),
            ["https://github.com/idvorkin/chop-conventions/issues/14"]
        );
    }

    #[test]
    fn named_ref_resolves_when_cwd_repo_also_in_scrollback() {
        let raw =
            "https://github.com/idvorkin/chop-conventions/pull/5\nchop#14 and chop-conventions#15";
        let want = [
            "https://github.com/idvorkin/chop-conventions/issues/14",
            "https://github.com/idvorkin/chop-conventions/issues/15",
        ];
        assert_eq!(refs(raw, ctx(Some("idvorkin/chop-conventions"))), want);
        let raw = "https://github.com/idvorkin/settings/pull/5\nsett#14";
        assert_eq!(
            refs(raw, ctx(Some("idvorkin/Settings"))),
            ["https://github.com/idvorkin/Settings/issues/14"]
        );
    }

    #[test]
    fn scrollback_urls_drop_trailing_punct_and_noise() {
        let raw = "see https://github.com/o/r. and https://github.com/o/r/pull/14.\nr#14 then #9";
        assert_eq!(
            refs(raw, ctx(None)),
            [
                "https://github.com/o/r/pull/14",
                "https://github.com/o/r/issues/9"
            ]
        );
        let raw = "open https://github.com/x/y/pull/new\nhttps://github.com/o/r/pull/1\n#9";
        assert_eq!(refs(raw, ctx(None)), ["https://github.com/o/r/issues/9"]);
    }

    #[test]
    fn known_pr_number_links_to_pull() {
        let raw = "https://github.com/o/r/pull/14 landed\nr#14 again";
        assert_eq!(refs(raw, ctx(None)), ["https://github.com/o/r/pull/14"]);
    }

    #[test]
    fn owner_repo_ref_is_used_as_is() {
        assert_eq!(
            refs("herdrdev/herdr#4235", ctx(None)),
            ["https://github.com/herdrdev/herdr/issues/4235"]
        );
    }

    #[test]
    fn pr_word_prefix_means_bare_ref() {
        assert_eq!(
            refs("PR#7", ctx(Some("o/r"))),
            ["https://github.com/o/r/issues/7"]
        );
    }

    #[test]
    fn ignores_url_fragments_and_html_entities() {
        let line = "https://github.com/o/r/pull/1#issuecomment-99 &#123; a/#5";
        assert!(refs(line, ctx(Some("o/r"))).is_empty());
    }

    #[test]
    fn short_ref_row_keeps_original_line_as_context() {
        let raw = "merged #14 and chop#3";
        let r = ShortRefs::new(raw, ctx(Some("o/r")));
        let rows = super::super::detect::parse_with(raw, &|l, i| r.scan(l, i));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].canonical, "https://github.com/o/r/issues/14");
        assert_eq!(rows[0].context, "merged #14 and chop#3");
    }

    #[test]
    fn gits_dir_exact_then_unique_prefix() {
        let root = std::env::temp_dir().join(format!("shortref-{}", std::process::id()));
        for (dir, url) in [
            (
                "chop-conventions",
                "git@github.com:idvorkin/chop-conventions.git",
            ),
            ("chop-mcp", "https://github.com/idvorkin/chop-mcp"),
            ("settings", "https://github.com/idvorkin/Settings.git"),
        ] {
            let git = root.join(dir).join(".git");
            std::fs::create_dir_all(&git).unwrap();
            std::fs::write(
                git.join("config"),
                format!("[core]\n\tbare = false\n[remote \"origin\"]\n\turl = {url}\n"),
            )
            .unwrap();
        }
        let c = RefContext {
            cwd_repo: None,
            gits_root: Some(root.clone()),
        };
        let seen = ScrollbackRepos::default();
        assert_eq!(
            resolve_repo(Some("settings"), &c, &seen).as_deref(),
            Some("idvorkin/Settings")
        );
        assert_eq!(
            resolve_repo(Some("chop-mcp"), &c, &seen).as_deref(),
            Some("idvorkin/chop-mcp")
        );
        assert_eq!(
            resolve_repo(Some("chop"), &c, &seen),
            None,
            "ambiguous prefix must not guess"
        );
        assert_eq!(
            resolve_repo(Some("sett"), &c, &seen).as_deref(),
            Some("idvorkin/Settings")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn parses_remote_url_forms() {
        for url in [
            "git@github.com:o/r.git",
            "https://github.com/o/r",
            "https://github.com/o/r.git",
            "ssh://git@github.com/o/r.git",
        ] {
            assert_eq!(
                github_repo_from_remote(url).as_deref(),
                Some("o/r"),
                "{url}"
            );
        }
        assert_eq!(github_repo_from_remote("https://gitlab.com/o/r"), None);
    }
}
