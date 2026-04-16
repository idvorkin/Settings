mod agent_continue;
mod link_picker;
mod picker;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use clap_complete::engine::{ArgValueCompleter, CompletionCandidate};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use sysinfo::{Pid, ProcessRefreshKind, System};

pub const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("GIT_HASH"), ")");

#[derive(Parser)]
#[command(name = "rmux_helper")]
#[command(version = VERSION)]
#[command(about = "A fast Tmux helper utility for session/window/pane management")]
#[command(long_about = "rmux_helper - A fast Tmux helper written in Rust

Features:
  - Fuzzy session/window/pane picker with tree view (pick-tui)
  - Auto-rename windows based on running processes (rename-all)
  - Layout rotation and 1/3-2/3 split management (rotate, third)
  - Resolve caller's owning tmux pane via parent-PID walk (parent-pid-tree)

Keybindings (configured in .tmux.conf):
  C-a w     Launch picker popup
  C-a C-w   Built-in tmux tree (fallback)

Source: https://github.com/idvorkin/settings")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Rename all tmux windows based on their current state
    RenameAll,
    /// Get current pane info as JSON
    Info,
    /// Toggle between even-horizontal and even-vertical layouts
    Rotate,
    /// Toggle between even layout and 1/3-2/3 layout (works with 2 panes)
    Third {
        /// Optional command to run in the first pane
        #[arg(default_value = "")]
        command: String,
    },
    /// Native TUI picker for session/window/pane (ratatui)
    PickTui,
    /// Open a file in a side nvim pane, reusing the pane across calls.
    /// Supports file:line syntax (e.g. foo.py:42). No args = print status only.
    SideEdit {
        /// File path to open (supports file:line syntax)
        #[arg(value_hint = ValueHint::FilePath)]
        file: Option<String>,
    },
    /// Run a shell command in the side pane (reuses same pane as side-edit).
    /// No args = print status only.
    SideRun {
        /// Command to run in the side pane
        command: Option<String>,
        /// Force kill nvim if it's running in the side pane
        #[arg(long)]
        force: bool,
    },
    /// Debug: show raw key events (press q to quit)
    DebugKeys,
    /// TUI picker for GitHub links, servers, and IPs in the current tmux pane's scrollback
    PickLinks {
        /// Emit JSON of detected items to stdout and exit (no TUI)
        #[arg(long)]
        json: bool,
        /// Enrichment deadline in milliseconds (0 disables gh enrichment)
        #[arg(long, default_value_t = 3000)]
        enrich_deadline_ms: u64,
    },
    /// Resolve the calling process's owning tmux pane by walking the parent-PID chain.
    /// Use this instead of `tmux display-message -p '#{pane_id}'`, which returns the
    /// tmux-active pane (focused pane) rather than the caller's pane.
    ParentPidTree {
        /// Emit structured JSON with pane_id, pane_pid, walked_from_pid, and ancestors_walked
        #[arg(long)]
        json: bool,
        /// Start the walk from this pid instead of the caller's pid.
        /// When omitted, starts from the parent of rmux_helper (i.e. the caller).
        #[arg(long, add = ArgValueCompleter::new(pid_completer))]
        pid: Option<u32>,
        /// Log the walk (ancestor chain, pane match) to stderr for debugging
        #[arg(long)]
        verbose: bool,
        /// Print the full ancestor chain as a visual tree with cmdline / exe
        /// metadata per PID. Combine with --json for structured output.
        #[arg(long)]
        tree: bool,
    },
    /// Install shell completions for rmux_helper.
    ///
    /// Writes a shell-specific completion script to the conventional location for
    /// the target shell and prints a one-line confirmation. Completions are
    /// dynamic — e.g. `parent-pid-tree --pid <TAB>` resolves live pids from
    /// `/proc` at tab-time.
    InstallCompletions {
        /// Target shell. Defaults to auto-detection from `$SHELL`.
        #[arg(long, value_enum)]
        shell: Option<CompletionShell>,
        /// Print the completion script to stdout instead of writing a file.
        #[arg(long, conflicts_with = "dry_run")]
        print_only: bool,
        /// Report the target path and skip the write. Useful for scripting.
        #[arg(long)]
        dry_run: bool,
    },
    /// Resume the most recent agent session found in the caller's pane scrollback.
    ///
    /// Scans the last N lines of the owning tmux pane for `claude --resume <UUID>`
    /// (extensible to other agents via the registry in `agent_continue.rs`).
    /// Exactly one match → exec `<launcher> --resume <id>` through `$SHELL -ic`.
    /// Zero matches → exit 1. Multiple distinct matches → exit 2 (refuses to guess).
    AgentContinue {
        /// How many lines of scrollback to scan (default 50).
        #[arg(long, default_value_t = 50)]
        window: usize,
        /// Print the command that would run and exit 0 instead of exec'ing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Same as `agent-continue`, but launches through the permissive wrapper
    /// (`yolo-claude` for claude). Requires a container — the wrapper enforces
    /// this via `_require_container`.
    AgentYoloContinue {
        /// How many lines of scrollback to scan (default 50).
        #[arg(long, default_value_t = 50)]
        window: usize,
        /// Print the command that would run and exit 0 instead of exec'ing.
        #[arg(long)]
        dry_run: bool,
    },
}

// Layout state constants
const LAYOUT_STATE_OPTION: &str = "@layout_state";
const THIRD_STATE_OPTION: &str = "@third_state";
const STATE_HORIZONTAL: &str = "horizontal";
const STATE_VERTICAL: &str = "vertical";
const STATE_THIRD_HORIZONTAL: &str = "third_horizontal";
const STATE_THIRD_VERTICAL: &str = "third_vertical";
const STATE_NORMAL: &str = "normal";
const SIDE_EDIT_PANE_OPTION: &str = "@side_edit_pane_id";

// Pane title cache file - stores original pane titles for future computation
fn get_pane_title_cache_path() -> PathBuf {
    PathBuf::from("/tmp/tmux_pane_titles.cache")
}

/// Context for title generation with width and pane information
struct TitleContext<'a> {
    short_path: &'a str,
    pane_title: Option<&'a str>,
    window_width: u32,
}

/// Load the pane title cache from disk
fn load_pane_title_cache() -> HashMap<String, String> {
    let cache_path = get_pane_title_cache_path();
    let mut cache = HashMap::new();

    if let Ok(file) = fs::File::open(&cache_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if let Some((pane_id, title)) = line.split_once('\t') {
                cache.insert(pane_id.to_string(), title.to_string());
            }
        }
    }
    cache
}

/// Save the pane title cache to disk
fn save_pane_title_cache(cache: &HashMap<String, String>) {
    let cache_path = get_pane_title_cache_path();
    if let Ok(mut file) = fs::File::create(&cache_path) {
        for (pane_id, title) in cache {
            let _ = writeln!(file, "{}\t{}", pane_id, title);
        }
    }
}

/// Get the pane title, updating cache if source changed
fn get_original_pane_title(
    pane_id: &str,
    current_title: &str,
    cache: &mut HashMap<String, String>,
) -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    // Normalize: treat hostname and empty as "no title"
    let normalized_current =
        if current_title.eq_ignore_ascii_case(&hostname) || current_title.is_empty() {
            String::new()
        } else {
            current_title.to_string()
        };

    // Check if we have a cached value
    if let Some(cached) = cache.get(pane_id) {
        // If current title changed from cached, update cache
        if cached != &normalized_current {
            cache.insert(pane_id.to_string(), normalized_current.clone());
        }
        // Return current (possibly updated) value
        return cache.get(pane_id).cloned().unwrap_or_default();
    }

    // First time seeing this pane - cache it
    cache.insert(pane_id.to_string(), normalized_current.clone());
    normalized_current
}

/// Shorten a pane title to fit within available width
fn shorten_pane_title(title: &str, max_width: usize) -> String {
    if title.is_empty() || max_width == 0 {
        return String::new();
    }

    let chars: Vec<char> = title.chars().collect();
    if chars.len() <= max_width {
        return title.to_string();
    }

    // Truncate and add ellipsis
    if max_width <= 1 {
        return "…".to_string();
    }

    let truncated: String = chars[..max_width - 1].iter().collect();
    format!("{}…", truncated)
}

/// Shorten a path by collapsing the middle to "…", preserving root/repo and leaf.
fn shorten_path_middle(path: &str, max_width: usize) -> String {
    if path.is_empty() || max_width == 0 {
        return String::new();
    }

    let path_len = path.chars().count();
    if path_len <= max_width {
        return path.to_string();
    }

    let (prefix, rest) = if let Some(stripped) = path.strip_prefix("~/") {
        ("~", stripped)
    } else if let Some(stripped) = path.strip_prefix('/') {
        ("/", stripped)
    } else {
        ("", path)
    };

    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return shorten_pane_title(path, max_width);
    }
    if parts.len() == 1 {
        return shorten_pane_title(path, max_width);
    }

    let last = parts.last().unwrap();
    let base = if prefix.is_empty() {
        parts.first().unwrap().to_string()
    } else {
        prefix.to_string()
    };

    let mut candidate = if prefix.is_empty() {
        format!("{}/…/{}", base, last)
    } else if prefix == "/" {
        format!("/…/{}", last)
    } else {
        format!("{}/…/{}", base, last)
    };

    if candidate.chars().count() <= max_width {
        return candidate;
    }

    let prefix_len = if prefix.is_empty() {
        format!("{}/…/", base).chars().count()
    } else if prefix == "/" {
        "/…/".chars().count()
    } else {
        format!("{}/…/", base).chars().count()
    };

    if max_width <= prefix_len {
        return shorten_pane_title(&base, max_width);
    }

    let space_for_leaf = max_width - prefix_len;
    let shortened_leaf = shorten_pane_title(last, space_for_leaf);
    if shortened_leaf.is_empty() {
        return shorten_pane_title(&base, max_width);
    }

    candidate = if prefix.is_empty() {
        format!("{}/…/{}", base, shortened_leaf)
    } else if prefix == "/" {
        format!("/…/{}", shortened_leaf)
    } else {
        format!("{}/…/{}", base, shortened_leaf)
    };
    candidate
}

fn read_proc_cmdline(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        let path = format!("/proc/{}/cmdline", pid);
        let Ok(bytes) = fs::read(path) else {
            return None;
        };
        if bytes.is_empty() {
            return None;
        }
        let cmdline = bytes
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s))
            .collect::<Vec<_>>()
            .join(" ");
        if cmdline.is_empty() {
            None
        } else {
            Some(cmdline)
        }
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "args="])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let cmdline = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if cmdline.is_empty() {
            None
        } else {
            Some(cmdline)
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

fn format_ai_title_no_pane(short_path: &str, label: &str, window_width: u32) -> String {
    let available = window_width as usize;
    let label_len = label.chars().count();
    if available <= label_len {
        return label.to_string();
    }

    let path_width = available.saturating_sub(label_len + 1);
    let rich = shorten_path_middle(short_path, path_width);
    if rich.is_empty() {
        return label.to_string();
    }

    format!("{} {}", label, rich)
}

#[derive(Debug)]
struct PaneInfo {
    pane_id: String,
    window_id: String,
    window_name: String,
    pane_pid: u32,
    pane_current_command: String,
    pane_current_path: String,
    pane_title: String,
    window_width: u32,
}

#[derive(Debug)]
struct ProcessInfo {
    pid: u32,
    name: String,
    cmdline: String,
    exe: Option<String>,
    cwd: String,
    children: Vec<ProcessInfo>,
}

// TODO: migrate callers of `run_tmux_command` (and the scattered
// `Command::new("tmux")` fire-and-forget sites in `side_edit`, `side_run`,
// `rename_all`, `rotate`, `third`, `info`, `resolve_side_pane`,
// `ensure_two_panes`, `create_side_pane_shell`, `open_file_in_pane`, etc.)
// to the `TmuxProvider` trait introduced for `parent-pid-tree`. None of
// those functions have characterization tests today, so migrating them
// blind would lose the "tests pass" safety net and could silently break
// live tmux behavior. The pure helpers (`pick_side_pane`, `format_pane_status`,
// `parse_file_line`, `resolve_pane_by_parent_chain`) are already DI-friendly
// and tested; the shelling helpers need tests first, then trait migration.
pub fn run_tmux_command(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("Failed to run tmux command")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_all_pane_info() -> Result<Vec<PaneInfo>> {
    let output = run_tmux_command(&[
        "list-panes",
        "-a",
        "-F",
        "#{pane_id}\t#{window_id}\t#{window_name}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}\t#{pane_title}\t#{window_width}",
    ])?;

    let mut panes = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            panes.push(PaneInfo {
                pane_id: parts[0].to_string(),
                window_id: parts[1].to_string(),
                window_name: parts[2].to_string(),
                pane_pid: parts[3].parse().unwrap_or(0),
                pane_current_command: parts.get(4).unwrap_or(&"").to_string(),
                pane_current_path: parts.get(5).unwrap_or(&"").to_string(),
                pane_title: parts.get(6).unwrap_or(&"").to_string(),
                window_width: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(80),
            });
        }
    }
    Ok(panes)
}

fn get_process_info(system: &System, pid: u32) -> Option<ProcessInfo> {
    let pid = Pid::from_u32(pid);
    let process = system.process(pid)?;

    let children: Vec<ProcessInfo> = system
        .processes()
        .iter()
        .filter(|(_, p)| p.parent() == Some(pid))
        .filter_map(|(child_pid, _)| get_process_info(system, child_pid.as_u32()))
        .collect();

    Some(ProcessInfo {
        pid: pid.as_u32(),
        name: process.name().to_string_lossy().to_string(),
        cmdline: process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        exe: process.exe().map(|p| p.to_string_lossy().to_string()),
        cwd: process
            .cwd()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        children,
    })
}

fn process_tree_has_pattern(info: &ProcessInfo, patterns: &[&str]) -> bool {
    let cmdline_lower = info.cmdline.to_lowercase();
    let name_lower = info.name.to_lowercase();
    if patterns
        .iter()
        .any(|p| cmdline_lower.contains(p) || name_lower.contains(p))
    {
        return true;
    }
    if let Some(exe) = info.exe.as_ref() {
        let exe_lower = exe.to_lowercase();
        if patterns.iter().any(|p| exe_lower.contains(p)) {
            return true;
        }
    }
    if let Some(proc_cmdline) = read_proc_cmdline(info.pid) {
        let proc_lower = proc_cmdline.to_lowercase();
        if patterns.iter().any(|p| proc_lower.contains(p)) {
            return true;
        }
    }
    info.children
        .iter()
        .any(|child| process_tree_has_pattern(child, patterns))
}

pub fn get_git_repo_name(cwd: &str, cache: &mut HashMap<String, Option<String>>) -> Option<String> {
    if cwd.is_empty() {
        return None;
    }

    if let Some(cached) = cache.get(cwd) {
        return cached.clone();
    }

    let result = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            } else {
                None
            }
        });

    cache.insert(cwd.to_string(), result.clone());
    result
}

pub fn get_short_path(cwd: &str, git_repo: Option<&str>) -> String {
    let path_mappings: HashMap<&str, &str> = [("idvorkin.github.io", "blog"), ("idvorkin", "me")]
        .into_iter()
        .collect();

    if let Some(repo) = git_repo {
        let base_name = path_mappings.get(repo).copied().unwrap_or(repo);
        // Try to get relative path within repo
        if let Ok(output) = Command::new("git")
            .args(["rev-parse", "--show-prefix"])
            .current_dir(cwd)
            .output()
        {
            let rel_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if rel_path.is_empty() {
                return base_name.to_string();
            }
            return format!("{}/{}", base_name, rel_path);
        }
        return base_name.to_string();
    }

    // Not in git repo - shorten home path
    if let Some(home) = std::env::var_os("HOME") {
        let home = home.to_string_lossy();
        if cwd.starts_with(home.as_ref()) {
            return format!("~{}", &cwd[home.len()..]);
        }
    }
    cwd.to_string()
}

/// Generate a window title from process info with optional pane context.
///
/// When running Claude/Codex and there's room, includes shortened pane title:
/// - "path:pane_title"
/// - Falls back to "cl|cx <path>" (richer path) if no pane title
///
/// Title format rules (in priority order):
/// 1. Known dev tools with path: "ai <path>", "cl|cx [pane] <path>", "vi <path>", "docker <path>"
/// 2. Plain shell (no children): "z <path>"
/// 3. Shell with known child commands: handled by get_child_command_title()
///    - just: "j <subcommand> <path>" (e.g., "j dev blog")
///    - jekyll: "jekyll <path>"
/// 4. Shell with unknown children: returns None (caller uses tmux fallback)
/// 5. Other processes: just the process name
///
/// Returns None when we can't determine a good title (caller should use tmux fallback)
fn generate_title_with_context(info: &ProcessInfo, ctx: &TitleContext) -> Option<String> {
    let short_path = ctx.short_path;

    // Priority 1: Known dev tools - check entire process tree for patterns
    if process_tree_has_pattern(info, &["aider"]) {
        return Some(format!("ai {}", short_path));
    }
    if process_tree_has_pattern(info, &["@anthropic-ai/claude-code", "claude"]) {
        return Some(generate_ai_title(ctx, "cl"));
    }
    if process_tree_has_pattern(info, &["@openai/codex", "codex"]) {
        return Some(generate_ai_title(ctx, "cx"));
    }
    if process_tree_has_pattern(info, &["vim", "nvim"]) {
        return Some(format!("vi {}", short_path));
    }
    if process_tree_has_pattern(info, &["docker"]) {
        return Some(format!("docker {}", short_path));
    }

    // Priority 2-4: Shell handling
    if is_shell(&info.name) {
        if info.children.is_empty() {
            // Plain shell with no children: "z <path>"
            return Some(format!("z {}", short_path));
        }
        // Shell with children - try to extract command info
        if let Some(title) = get_child_command_title(info, short_path) {
            return Some(title);
        }
        // Unknown children - let caller use tmux fallback
        return None;
    }

    // Priority 5: Other processes - just use the name
    Some(info.name.clone())
}

/// Compact a path using 2..2 format (first 2 chars + ".." + last 2 chars)
/// Only applies if result is shorter than original
fn compact_path(path: &str) -> String {
    let chars: Vec<char> = path.chars().collect();
    // 2 + 2 + 2 = 6 chars minimum for "xx..yy" format
    // Only compact if path is longer than 6 chars
    if chars.len() <= 6 {
        return path.to_string();
    }

    let first: String = chars[..2].iter().collect();
    let last: String = chars[chars.len() - 2..].iter().collect();
    format!("{}..{}", first, last)
}

/// Strip common prefixes from pane titles (like ✳ from Claude)
fn clean_pane_title(title: &str) -> &str {
    title
        .trim_start_matches("✳ ")
        .trim_start_matches("✳")
        .trim()
}

/// Generate a Claude/Codex title with dynamic width and optional pane name.
///
/// Format with pane title: "path:pane_name"
/// - Path is compacted to 2..2 format if longer than 6 chars
/// - Pane title has ✳ prefix stripped
///
/// Format without pane title: "cl|cx <path>" (richer path, middle-ellipsized if needed)
fn generate_ai_title(ctx: &TitleContext, label: &str) -> String {
    let compact = compact_path(ctx.short_path);

    // If no pane title, just return path
    let pane_title = match ctx.pane_title {
        Some(t) if !t.is_empty() => clean_pane_title(t),
        _ => return format_ai_title_no_pane(ctx.short_path, label, ctx.window_width),
    };

    if pane_title.is_empty() {
        return compact;
    }

    // Format: path:pane_name
    let base = format!("{}:", compact);
    let base_len = base.chars().count();
    let available_width = ctx.window_width as usize;

    // Calculate space for pane title
    let space_for_pane = available_width.saturating_sub(base_len);

    if space_for_pane == 0 {
        return compact;
    }

    // Shorten pane title if needed
    let shortened_pane = shorten_pane_title(pane_title, space_for_pane);
    if shortened_pane.is_empty() {
        return compact;
    }

    format!("{}{}", base, shortened_pane)
}

/// Legacy generate_title without context (for compatibility)
fn generate_title(info: &ProcessInfo, short_path: &str) -> Option<String> {
    let ctx = TitleContext {
        short_path,
        pane_title: None,
        window_width: 80,
    };
    generate_title_with_context(info, &ctx)
}

/// Extract a nice title from shell's child processes.
///
/// This handles commands that need special formatting beyond just the command name.
/// The short_path is the shortened current working directory (e.g., "blog" for idvorkin.github.io).
///
/// Supported commands:
/// - just: "j <subcommand> <path>" - shows the just recipe being run
///   Examples: "j dev blog", "j build settings", "j serve ~"
/// - jekyll: "jekyll <path>" - for jekyll serve/build commands
///   Examples: "jekyll blog", "jekyll ~"
///
/// Returns None if no known command is found (caller will use tmux fallback).
fn get_child_command_title(info: &ProcessInfo, short_path: &str) -> Option<String> {
    for child in &info.children {
        let name_lower = child.name.to_lowercase();
        let cmdline_lower = child.cmdline.to_lowercase();

        // just command: extract the recipe name and append path
        // "just dev" in blog dir -> "j dev blog"
        // "just jekyll-serve" -> "j jekyll blog" (shorten jekyll-* recipes)
        // "just" alone in blog dir -> "j blog"
        if name_lower == "just" {
            let args: Vec<&str> = child.cmdline.split_whitespace().collect();
            if args.len() > 1 {
                let subcommand = args[1..].join(" ");
                // Shorten jekyll-related recipes to just "jekyll"
                if subcommand.to_lowercase().contains("jekyll") {
                    return Some(format!("j jekyll {}", short_path));
                }
                // Has subcommand: "j <subcommand> <path>"
                return Some(format!("j {} {}", subcommand, short_path));
            }
            // No subcommand: "j <path>"
            return Some(format!("j {}", short_path));
        }

        // jekyll command: just show "jekyll <path>"
        // Matches both "jekyll" binary and ruby scripts running jekyll
        if name_lower == "jekyll" || cmdline_lower.contains("jekyll") {
            return Some(format!("jekyll {}", short_path));
        }

        // Recursively check grandchildren (handles nested process trees)
        if let Some(title) = get_child_command_title(child, short_path) {
            return Some(title);
        }
    }
    None
}

fn is_shell(name: &str) -> bool {
    matches!(name, "zsh" | "bash" | "fish" | "sh")
}

/// Generate title from tmux pane info (fallback when process info unavailable)
/// Note: This fallback doesn't have access to pane title context, so uses minimal format
fn generate_title_from_tmux(command: &str, short_path: &str) -> String {
    let cmd_lower = command.to_lowercase();
    if cmd_lower.contains("aider") {
        format!("ai {}", short_path)
    } else if cmd_lower.contains("claude") {
        // Just use richer path (no pane title in fallback)
        format_ai_title_no_pane(short_path, "cl", 80)
    } else if cmd_lower.contains("codex") {
        // Just use richer path (no pane title in fallback)
        format_ai_title_no_pane(short_path, "cx", 80)
    } else if cmd_lower == "vim" || cmd_lower == "nvim" {
        format!("vi {}", short_path)
    } else if cmd_lower.contains("docker") {
        format!("docker {}", short_path)
    } else if cmd_lower == "just" {
        format!("j {}", short_path)
    } else if cmd_lower == "zsh" || cmd_lower == "bash" || cmd_lower == "fish" {
        format!("z {}", short_path)
    } else {
        command.to_string()
    }
}

fn set_tmux_title(title: &str, pane_id: &str, window_id: &str, current_name: &str) {
    if title.is_empty() || title == current_name {
        return;
    }

    // Disable automatic renaming
    let _ = Command::new("tmux")
        .args(["set", "-t", pane_id, "automatic-rename", "off"])
        .output();

    // Rename window
    let _ = Command::new("tmux")
        .args(["rename-window", "-t", window_id, title])
        .output();
}

fn rename_all() -> Result<()> {
    let panes = get_all_pane_info()?;

    // Refresh process info once with full details
    let mut system = System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let mut git_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut renamed_windows: HashSet<String> = HashSet::new();
    let mut pane_title_cache = load_pane_title_cache();

    for pane in &panes {
        // Skip if already renamed this window
        if renamed_windows.contains(&pane.window_id) {
            continue;
        }
        renamed_windows.insert(pane.window_id.clone());

        // Get the original pane title (cached to preserve across renames)
        let original_pane_title =
            get_original_pane_title(&pane.pane_id, &pane.pane_title, &mut pane_title_cache);

        // Try to get process info from system, with tmux fallback
        let title = if let Some(process_info) = get_process_info(&system, pane.pane_pid) {
            let cwd = &process_info.cwd;
            let git_repo = get_git_repo_name(cwd, &mut git_cache);
            let short_path = get_short_path(cwd, git_repo.as_deref());

            let ctx = TitleContext {
                short_path: &short_path,
                pane_title: if original_pane_title.is_empty() {
                    None
                } else {
                    Some(&original_pane_title)
                },
                window_width: pane.window_width,
            };

            generate_title_with_context(&process_info, &ctx).unwrap_or_else(|| {
                generate_title_from_tmux(&pane.pane_current_command, &short_path)
            })
        } else {
            // Fallback: use tmux's pane info (works for remote/container processes)
            let cwd = &pane.pane_current_path;
            let git_repo = get_git_repo_name(cwd, &mut git_cache);
            let short_path = get_short_path(cwd, git_repo.as_deref());
            generate_title_from_tmux(&pane.pane_current_command, &short_path)
        };

        set_tmux_title(&title, &pane.pane_id, &pane.window_id, &pane.window_name);
    }

    // Save the pane title cache
    save_pane_title_cache(&pane_title_cache);

    Ok(())
}

fn info() -> Result<()> {
    // Get current pane info from tmux
    let pane_info = run_tmux_command(&[
        "display-message",
        "-p",
        "#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}",
    ])?;
    let parts: Vec<&str> = pane_info.split('\t').collect();
    let pane_pid: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let pane_cmd = parts.get(1).unwrap_or(&"");
    let pane_path = parts.get(2).unwrap_or(&"");

    let mut system = System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let mut git_cache = HashMap::new();

    let (cwd, title) = if let Some(process_info) = get_process_info(&system, pane_pid) {
        let cwd = process_info.cwd.clone();
        let git_repo = get_git_repo_name(&cwd, &mut git_cache);
        let short_path = get_short_path(&cwd, git_repo.as_deref());
        let title = generate_title(&process_info, &short_path)
            .unwrap_or_else(|| generate_title_from_tmux(pane_cmd, &short_path));
        (cwd, title)
    } else {
        let cwd = pane_path.to_string();
        let git_repo = get_git_repo_name(&cwd, &mut git_cache);
        let short_path = get_short_path(&cwd, git_repo.as_deref());
        let title = generate_title_from_tmux(pane_cmd, &short_path);
        (cwd, title)
    };

    let git_repo = get_git_repo_name(&cwd, &mut git_cache);
    let short_path = get_short_path(&cwd, git_repo.as_deref());

    println!(
        r#"{{"cwd":"{}","short_path":"{}","app":"{}","title":"{}","git_repo":{}}}"#,
        cwd,
        short_path,
        pane_cmd,
        title,
        git_repo
            .as_ref()
            .map(|r| format!("\"{}\"", r))
            .unwrap_or_else(|| "null".to_string())
    );

    // Set the title
    let _ = Command::new("tmux")
        .args(["set", "automatic-rename", "off"])
        .output();
    let _ = Command::new("tmux")
        .args(["rename-window", &title])
        .output();

    Ok(())
}

fn get_tmux_option(option: &str) -> String {
    // Use -wqv for window-local options (not global) so each window/session has its own state
    run_tmux_command(&["show-option", "-wqv", option]).unwrap_or_default()
}

fn set_tmux_option(option: &str, value: &str) {
    // Use -w for window-local options (not global) so each window/session has its own state
    let _ = Command::new("tmux")
        .args(["set-option", "-w", option, value])
        .output();
}

fn ensure_two_panes(command: Option<&str>, caller_pane_id: Option<&str>) -> (Vec<String>, bool) {
    let panes = if let Some(target) = caller_pane_id {
        run_tmux_command(&["list-panes", "-t", target, "-F", "#{pane_id}"])
    } else {
        run_tmux_command(&["list-panes", "-F", "#{pane_id}"])
    }
    .map(|s| {
        s.lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();

    if panes.len() == 1 {
        let cwd = caller_pane_id
            .and_then(|t| {
                run_tmux_command(&["display-message", "-t", t, "-p", "#{pane_current_path}"]).ok()
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "#{pane_current_path}".to_string());

        let mut args = vec!["split-window", "-h", "-c", &cwd];
        if let Some(t) = caller_pane_id {
            args.extend(["-t", t]);
        }
        if let Some(cmd) = command.filter(|c| !c.is_empty()) {
            args.push(cmd);
        }
        let _ = Command::new("tmux").args(&args).output();

        if let Some(t) = caller_pane_id {
            let _ = Command::new("tmux")
                .args(["select-layout", "-t", t, "even-horizontal"])
                .output();
        } else {
            let _ = Command::new("tmux")
                .args(["select-layout", "even-horizontal"])
                .output();
        }

        let new_panes = if let Some(t) = caller_pane_id {
            run_tmux_command(&["list-panes", "-t", t, "-F", "#{pane_id}"])
        } else {
            run_tmux_command(&["list-panes", "-F", "#{pane_id}"])
        }
        .map(|s| {
            s.lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

        return (new_panes, true);
    }
    (panes, false)
}

fn get_layout_orientation_for(caller_pane_id: Option<&str>) -> Option<String> {
    let pane_info = if let Some(t) = caller_pane_id {
        run_tmux_command(&["list-panes", "-t", t, "-F", "#{pane_left},#{pane_top}"])
    } else {
        run_tmux_command(&["list-panes", "-F", "#{pane_left},#{pane_top}"])
    }
    .ok()?;
    let lines: Vec<&str> = pane_info.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    let parts1: Vec<&str> = lines[0].split(',').collect();
    let parts2: Vec<&str> = lines[1].split(',').collect();
    if parts1.len() < 2 || parts2.len() < 2 {
        return None;
    }

    let pane1_left: i32 = parts1[0].parse().ok()?;
    let pane2_left: i32 = parts2[0].parse().ok()?;

    Some(if pane1_left != pane2_left {
        STATE_HORIZONTAL.to_string()
    } else {
        STATE_VERTICAL.to_string()
    })
}

fn rotate() -> Result<()> {
    let caller_pane_id = std::env::var("TMUX_PANE").ok();
    let (panes, created_new) = ensure_two_panes(None, caller_pane_id.as_deref());
    if panes.is_empty() {
        return Ok(());
    }

    if created_new {
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL);
        return Ok(());
    }

    let current_state = get_tmux_option(LAYOUT_STATE_OPTION);

    let select_layout = |layout: &str| {
        let mut args = vec!["select-layout"];
        if let Some(ref t) = caller_pane_id {
            args.extend(["-t", t]);
        }
        args.push(layout);
        let _ = Command::new("tmux").args(&args).output();
    };

    if current_state == STATE_HORIZONTAL {
        select_layout("even-vertical");
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_VERTICAL);
    } else {
        select_layout("even-horizontal");
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL);
    }

    Ok(())
}

fn third(command: &str) -> Result<()> {
    let caller_pane_id = std::env::var("TMUX_PANE").ok();
    let caller = caller_pane_id.as_deref();
    let cmd_opt = if command.is_empty() {
        None
    } else {
        Some(command)
    };
    let (panes, created_new) = ensure_two_panes(cmd_opt, caller);
    if panes.len() != 2 {
        return Ok(());
    }

    let orientation = match get_layout_orientation_for(caller) {
        Some(o) => o,
        None => return Ok(()),
    };

    let is_horizontal = orientation == STATE_HORIZONTAL;
    let mut current_state = get_tmux_option(THIRD_STATE_OPTION);

    // If command provided, always apply layout (don't toggle)
    if !command.is_empty() {
        set_tmux_option(THIRD_STATE_OPTION, STATE_NORMAL);
        current_state = STATE_NORMAL.to_string();
    }

    if current_state == STATE_THIRD_HORIZONTAL || current_state == STATE_THIRD_VERTICAL {
        // Restore to even layout
        if is_horizontal {
            let _ = Command::new("tmux")
                .args(["select-layout", "even-horizontal"])
                .output();
        } else {
            let _ = Command::new("tmux")
                .args(["select-layout", "even-vertical"])
                .output();
        }
        set_tmux_option(THIRD_STATE_OPTION, STATE_NORMAL);
    } else {
        // Get window dimensions — use caller pane as target for background safety
        let dim_args: Vec<&str> = if let Some(t) = caller {
            vec![
                "display-message",
                "-t",
                t,
                "-p",
                "#{window_width},#{window_height}",
            ]
        } else {
            vec!["display-message", "-p", "#{window_width},#{window_height}"]
        };
        let window_info = run_tmux_command(&dim_args)?;
        let parts: Vec<&str> = window_info.split(',').collect();
        if parts.len() != 2 {
            return Ok(());
        }
        let window_width: i32 = parts[0].parse().unwrap_or(0);
        let window_height: i32 = parts[1].parse().unwrap_or(0);

        if is_horizontal {
            let target_width = (window_width as f32 * 0.33) as i32;
            let _ = Command::new("tmux")
                .args([
                    "resize-pane",
                    "-t",
                    &panes[0],
                    "-x",
                    &target_width.to_string(),
                ])
                .output();
            set_tmux_option(THIRD_STATE_OPTION, STATE_THIRD_HORIZONTAL);
        } else {
            let target_height = (window_height as f32 * 0.33) as i32;
            let _ = Command::new("tmux")
                .args([
                    "resize-pane",
                    "-t",
                    &panes[0],
                    "-y",
                    &target_height.to_string(),
                ])
                .output();
            set_tmux_option(THIRD_STATE_OPTION, STATE_THIRD_VERTICAL);
        }
    }

    // If command provided and pane already existed, send command
    if !command.is_empty() && !created_new {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &panes[0], command, "Enter"])
            .output();
    }

    // If command provided, focus the working pane
    if !command.is_empty() {
        let _ = Command::new("tmux")
            .args(["select-pane", "-t", &panes[1]])
            .output();
    }

    Ok(())
}

/// Read $TMUX_PANE — the pane that invoked this binary.
/// Reliable even when the window is backgrounded.
fn get_caller_pane_id() -> Option<String> {
    std::env::var("TMUX_PANE").ok().filter(|s| !s.is_empty())
}

/// Return all pane IDs in the window that contains `window_target`.
fn get_panes_in_window(window_target: &str) -> Vec<String> {
    run_tmux_command(&["list-panes", "-t", window_target, "-F", "#{pane_id}"])
        .map(|s| {
            s.lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Return the current working directory of `pane_id`.
fn get_pane_cwd(pane_id: &str) -> String {
    run_tmux_command(&[
        "display-message",
        "-t",
        pane_id,
        "-p",
        "#{pane_current_path}",
    ])
    .unwrap_or_default()
}

/// Inspect a pane for vim/nvim in its process tree.
///
/// Returns:
/// - `Some(true)`  — pane was inspected and has vim/nvim somewhere in the tree
/// - `Some(false)` — pane was inspected and has no vim/nvim
/// - `None`        — inspection failed (pid query failed, process gone from snapshot, etc.)
///
/// Uses the same broad matching as `process_tree_has_pattern` (name + cmdline + exe substring),
/// so it catches wrappers like `nvim.appimage`, `nvim-qt`, embedded under shells, etc.
fn inspect_pane_for_vim(pane_id: &str, system: &System) -> Option<bool> {
    let pid_str =
        run_tmux_command(&["display-message", "-t", pane_id, "-p", "#{pane_pid}"]).ok()?;
    let pid: u32 = pid_str.trim().parse().ok().filter(|p| *p > 0)?;
    let info = get_process_info(system, pid)?;
    Some(process_tree_has_pattern(&info, &["vim", "nvim"]))
}

/// Return true if nvim/vim is anywhere in the process tree of `pane_id`.
/// Treats inspection failure as "no" — callers that need to distinguish
/// uninspectable from absent should call `inspect_pane_for_vim` directly.
fn is_vim_in_pane(pane_id: &str, system: &System) -> bool {
    inspect_pane_for_vim(pane_id, system).unwrap_or(false)
}

/// Return true if pane is running vim/nvim or an idle shell (safe for side-edit).
/// Returns false if running another foreground process to avoid injecting keystrokes.
fn is_pane_safe_to_adopt(pane_id: &str, system: &System) -> bool {
    let pid_str = run_tmux_command(&["display-message", "-t", pane_id, "-p", "#{pane_pid}"])
        .unwrap_or_default();
    let pid: u32 = match pid_str.trim().parse() {
        Ok(p) if p > 0 => p,
        _ => return false,
    };
    let info = match get_process_info(system, pid) {
        Some(i) => i,
        None => return false,
    };
    // Vim/nvim running — safe
    if process_tree_has_pattern(&info, &["vim", "nvim"]) {
        return true;
    }
    // Idle shell (no children) — safe
    let name = info.name.to_lowercase();
    if (name == "zsh" || name == "bash" || name == "sh") && info.children.is_empty() {
        return true;
    }
    false
}

/// Parse a file argument for trailing `:line` syntax.
/// Returns (file_path, Option<line_number>).
fn parse_file_line(input: &str) -> (String, Option<usize>) {
    if let Some(colon_pos) = input.rfind(':') {
        let (path, rest) = input.split_at(colon_pos);
        let num_str = &rest[1..]; // skip the ':'
        if !path.is_empty() {
            if let Ok(line) = num_str.parse::<usize>() {
                if line > 0 {
                    return (path.to_string(), Some(line));
                }
            }
        }
    }
    (input.to_string(), None)
}

/// Find the nvim PID in a pane's process tree, if any.
fn find_nvim_pid_in_pane(pane_id: &str, system: &System) -> Option<u32> {
    let pid_str = run_tmux_command(&["display-message", "-t", pane_id, "-p", "#{pane_pid}"])
        .unwrap_or_default();
    let pid: u32 = pid_str.trim().parse().ok().filter(|p| *p > 0)?;
    let info = get_process_info(system, pid)?;
    find_nvim_in_tree(&info)
}

/// Recursively find nvim/vim PID in a process tree.
///
/// Matches the binary basename (from `name` or `exe`) so wrappers like
/// `nvim.appimage` and `nvim-qt` are recognized. Avoids cmdline-substring
/// matching here because we need a *specific* pid for `/proc/<pid>/cmdline`,
/// and a parent shell whose cmdline accidentally contains "vim" would
/// otherwise win over the real nvim child.
fn find_nvim_in_tree(info: &ProcessInfo) -> Option<u32> {
    if node_is_vim_binary(info) {
        return Some(info.pid);
    }
    for child in &info.children {
        if let Some(pid) = find_nvim_in_tree(child) {
            return Some(pid);
        }
    }
    None
}

/// True if this process node looks like a vim/nvim binary by basename.
fn node_is_vim_binary(info: &ProcessInfo) -> bool {
    if basename_is_vim(&info.name) {
        return true;
    }
    if let Some(exe) = info.exe.as_ref() {
        // exe is a path; pull off the trailing component without bringing
        // std::path::Path into scope here.
        let base = exe.rsplit('/').next().unwrap_or(exe);
        if basename_is_vim(base) {
            return true;
        }
    }
    false
}

/// True if a binary basename is vim/nvim or a recognizable packaged variant.
///
/// Uses an explicit allow-list for hyphenated wrapper names so unrelated tools
/// like `vim-addon-manager` or `nvim-lsp-installer` (whose names start with
/// `vim-`/`nvim-` but are not themselves editors) cannot win the pid race in
/// `find_nvim_in_tree`. Dotted variants (`nvim.appimage`, `vim.basic`) stay as
/// a prefix rule because the dot is a strong signal it's a packaged binary.
fn basename_is_vim(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.starts_with("nvim.") || lower.starts_with("vim.") {
        return true;
    }
    matches!(
        lower.as_str(),
        "vim"
            | "nvim"
            | "nvim-qt"
            | "nvim-qt.exe"
            | "vim-tiny"
            | "vim-basic"
            | "vim-nox"
            | "vim-gtk"
            | "vim-gtk3"
            | "vim-athena"
    )
}

/// Get the file argument from an nvim process via /proc/cmdline.
fn get_nvim_current_file(nvim_pid: u32) -> Option<String> {
    let cmdline = read_proc_cmdline(nvim_pid)?;
    // cmdline is space-separated; find last arg that isn't a flag
    cmdline
        .split_whitespace()
        .skip(1) // skip "nvim" itself
        .filter(|arg| !arg.starts_with('-') && !arg.starts_with('+'))
        .last()
        .map(|s| s.to_string())
}

/// Pane status info for stdout output.
struct SidePaneStatus {
    /// Resolved side pane id, or `"none"` if no candidate exists,
    /// or `"ambiguous"` if we cannot confidently identify a single side pane.
    /// These sentinel values are part of the wire format for shell consumers
    /// of `side-edit`/`side-run` status output — do not silently rename.
    pane_id: String,
    /// `Some(true|false)` after a real inspection; `None` if we couldn't inspect
    /// (e.g., pid query failed). Callers should treat `None` as "unknown",
    /// not "false".
    nvim_running: Option<bool>,
    file: Option<String>,
}

/// Resolved side pane choice from `resolve_side_pane`.
#[derive(Debug, PartialEq, Eq)]
enum ResolvedSidePane {
    /// A specific pane id (we are confident this is "the" side pane).
    Pane(String),
    /// No candidate pane exists (window has only the caller pane).
    None,
    /// Multiple plausible candidates — caller cannot route to a single pane.
    Ambiguous,
}

/// Result of resolving which pane is "the" side pane and whether vim is on it.
#[derive(Debug, PartialEq, Eq)]
struct ResolvedStatus {
    pane: ResolvedSidePane,
    nvim_running: Option<bool>,
}

/// Pure resolver: pick the side pane and inspect it for vim/nvim.
///
/// (Named `pick_side_pane` rather than `resolve_side_pane` because the latter
/// is already taken by the imperative resolve-or-adopt-or-create helper used
/// by `side_edit`/`side_run`.)
///
/// Resolution order (first match wins):
/// 1. `stored` is non-empty AND in `window_panes` AND not equal to `caller_pane_id`
///    → use `stored` and call `inspect(stored)`.
/// 2. Exactly one "other" pane in the window → use it and call `inspect`.
/// 3. Zero "other" panes → `None`/`Some(false)` (definitively no side pane).
/// 4. Multiple "other" panes → walk each one with `inspect`. If exactly one
///    pane reports `Some(true)` AND no other pane reported `None`, use it.
///    Otherwise:
///    - 0 vim panes, no inspection failures → `None`/`Some(false)`
///    - 0 vim panes, some inspection failures → `None`/`None` (unknown)
///    - 1 vim pane, but other panes were uninspectable → `Ambiguous`/`Some(true)`
///      (one of the uninspectable panes might also have vim — refuse to pick)
///    - >1 vim panes → `Ambiguous`/`Some(true)`
///
/// Pure: takes an injectable `inspect` closure so all branches can be unit-tested
/// with a fake inspector instead of needing tmux + sysinfo.
fn pick_side_pane<F>(
    caller_pane_id: &str,
    window_panes: &[String],
    stored: &str,
    mut inspect: F,
) -> ResolvedStatus
where
    F: FnMut(&str) -> Option<bool>,
{
    // Step 1: stored option, if still valid for this window.
    let stored_valid =
        !stored.is_empty() && window_panes.iter().any(|p| p == stored) && stored != caller_pane_id;
    if stored_valid {
        let nvim_running = inspect(stored);
        return ResolvedStatus {
            pane: ResolvedSidePane::Pane(stored.to_string()),
            nvim_running,
        };
    }

    let others: Vec<&String> = window_panes
        .iter()
        .filter(|p| p.as_str() != caller_pane_id)
        .collect();

    // Step 2: exactly one other pane is the obvious side pane.
    if others.len() == 1 {
        let pane = others[0].clone();
        let nvim_running = inspect(&pane);
        return ResolvedStatus {
            pane: ResolvedSidePane::Pane(pane),
            nvim_running,
        };
    }

    // Step 3: no other panes — definitively nothing to inspect.
    if others.is_empty() {
        return ResolvedStatus {
            pane: ResolvedSidePane::None,
            nvim_running: Some(false),
        };
    }

    // Step 4: multiple other panes — walk each one.
    let mut vim_panes: Vec<String> = Vec::new();
    let mut any_uninspectable = false;
    for pane in &others {
        match inspect(pane) {
            Some(true) => vim_panes.push((*pane).clone()),
            Some(false) => {}
            None => any_uninspectable = true,
        }
    }

    match vim_panes.len() {
        1 if !any_uninspectable => ResolvedStatus {
            pane: ResolvedSidePane::Pane(vim_panes.remove(0)),
            nvim_running: Some(true),
        },
        1 => {
            // One confirmed vim pane, but at least one other pane could not be
            // inspected and could *also* be running vim. Refuse to silently
            // route to a possibly-wrong pane.
            ResolvedStatus {
                pane: ResolvedSidePane::Ambiguous,
                nvim_running: Some(true),
            }
        }
        0 => ResolvedStatus {
            pane: ResolvedSidePane::None,
            // We inspected every pane we could; call it "unknown" only if at
            // least one inspection actually failed.
            nvim_running: if any_uninspectable { None } else { Some(false) },
        },
        _ => ResolvedStatus {
            pane: ResolvedSidePane::Ambiguous,
            nvim_running: Some(true),
        },
    }
}

/// Get the current side pane status (queries tmux + sysinfo, then delegates
/// to the pure `resolve_side_pane` for branch logic).
fn get_side_pane_status(caller_pane_id: &str) -> SidePaneStatus {
    // Query the window-local option scoped to the caller's window (not tmux's "current" window)
    let stored = run_tmux_command(&[
        "show-option",
        "-wqv",
        "-t",
        caller_pane_id,
        SIDE_EDIT_PANE_OPTION,
    ])
    .unwrap_or_default();
    let window_panes = get_panes_in_window(caller_pane_id);

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let resolved = pick_side_pane(caller_pane_id, &window_panes, &stored, |p| {
        inspect_pane_for_vim(p, &sys)
    });

    let (pane_id, file) = match resolved.pane {
        ResolvedSidePane::Pane(p) => {
            let file = if resolved.nvim_running == Some(true) {
                find_nvim_pid_in_pane(&p, &sys).and_then(get_nvim_current_file)
            } else {
                None
            };
            (p, file)
        }
        ResolvedSidePane::None => ("none".to_string(), None),
        ResolvedSidePane::Ambiguous => ("ambiguous".to_string(), None),
    };

    SidePaneStatus {
        pane_id,
        nvim_running: resolved.nvim_running,
        file,
    }
}

/// Format pane status for stdout. Extracted from `print_pane_status` so the
/// wire format can be unit-tested.
fn format_pane_status(status: &SidePaneStatus) -> String {
    let nvim_str = match status.nvim_running {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    };
    format!(
        "pane_id: {}\nnvim: {}\nfile: {}",
        status.pane_id,
        nvim_str,
        status.file.as_deref().unwrap_or(""),
    )
}

/// Print pane status to stdout.
fn print_pane_status(status: &SidePaneStatus) {
    println!("{}", format_pane_status(status));
}

/// Escape a path for use in a nvim Ex command (`:e`).
fn escape_for_vim_ex(path: &str) -> String {
    path.replace('\\', "\\\\")
        .replace(' ', "\\ ")
        .replace('#', "\\#")
        .replace('%', "\\%")
}

/// Shell-quote a path (single-quote wrapping, like shlex.quote).
fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

/// Open a file in an existing pane, reusing nvim if running.
fn open_file_in_pane(
    pane_id: &str,
    shell_file: &str,
    vim_file: &str,
    line: Option<usize>,
    system: &System,
) {
    if is_vim_in_pane(pane_id, system) {
        // Double Escape handles insert/visual/command-line modes
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", pane_id, "Escape"])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", pane_id, "Escape"])
            .output();
        // C-\ C-n exits terminal mode (no-op in normal mode)
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", pane_id, r"C-\"])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", pane_id, "C-n"])
            .output();
        // Open file with vim-escaped path, optionally at line
        let ex_cmd = match line {
            Some(n) => format!(":e +{} {}", n, vim_file),
            None => format!(":e {}", vim_file),
        };
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", pane_id, &ex_cmd, "Enter"])
            .output();
    } else {
        let nvim_cmd = match line {
            Some(n) => format!("nvim +{} {}", n, shell_file),
            None => format!("nvim {}", shell_file),
        };
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", pane_id, &nvim_cmd, "Enter"])
            .output();
    }
}

/// Resolve the side pane: find existing or create a new shell pane. Returns the pane ID.
fn resolve_side_pane(caller_pane_id: &str) -> Result<String> {
    let window_panes = get_panes_in_window(caller_pane_id);
    if window_panes.is_empty() {
        anyhow::bail!("Could not list panes in current window.");
    }

    // Query window-local option scoped to caller's window
    let stored_pane_id = run_tmux_command(&[
        "show-option",
        "-wqv",
        "-t",
        caller_pane_id,
        SIDE_EDIT_PANE_OPTION,
    ])
    .unwrap_or_default();

    if !stored_pane_id.is_empty()
        && window_panes.contains(&stored_pane_id)
        && stored_pane_id != caller_pane_id
    {
        return Ok(stored_pane_id);
    }

    let other_panes: Vec<&String> = window_panes
        .iter()
        .filter(|p| p.as_str() != caller_pane_id)
        .collect();

    match other_panes.len() {
        0 => {
            // Only caller pane — create a shell split
            let new_id =
                create_side_pane_shell(caller_pane_id).context("Failed to create side pane.")?;
            let _ = Command::new("tmux")
                .args([
                    "set-option",
                    "-w",
                    "-t",
                    caller_pane_id,
                    SIDE_EDIT_PANE_OPTION,
                    &new_id,
                ])
                .output();
            Ok(new_id)
        }
        1 => {
            let candidate = other_panes[0];
            let mut sys = System::new();
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::everything(),
            );
            if !is_pane_safe_to_adopt(candidate, &sys) {
                anyhow::bail!(
                    "The other pane is running a foreground process. \
                     Close it or use a 1-pane window so side-edit can create its own."
                );
            }
            let adopted = candidate.clone();
            let _ = Command::new("tmux")
                .args([
                    "set-option",
                    "-w",
                    "-t",
                    caller_pane_id,
                    SIDE_EDIT_PANE_OPTION,
                    &adopted,
                ])
                .output();
            Ok(adopted)
        }
        n => {
            anyhow::bail!(
                "Window has {} other panes and no registered side-edit pane. \
                 Close extra panes or run from a 1-pane window.",
                n
            );
        }
    }
}

/// Create a side pane with just a shell (no nvim). Returns new pane ID.
fn create_side_pane_shell(caller_pane_id: &str) -> Option<String> {
    let caller_cwd = {
        let cwd = get_pane_cwd(caller_pane_id);
        if cwd.is_empty() {
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
        } else {
            cwd
        }
    };

    let third_state = get_tmux_option(THIRD_STATE_OPTION);
    let is_third_active =
        third_state == STATE_THIRD_HORIZONTAL || third_state == STATE_THIRD_VERTICAL;

    let new_pane_id = run_tmux_command(&[
        "split-window",
        "-h",
        "-c",
        &caller_cwd,
        "-t",
        caller_pane_id,
        "-P",
        "-F",
        "#{pane_id}",
    ])
    .ok()
    .filter(|s| !s.is_empty())?;

    if is_third_active {
        if let Ok(width_str) = run_tmux_command(&[
            "display-message",
            "-t",
            caller_pane_id,
            "-p",
            "#{window_width}",
        ]) {
            if let Ok(width) = width_str.trim().parse::<i32>() {
                let target = (width as f32 * 0.33) as i32;
                let _ = Command::new("tmux")
                    .args([
                        "resize-pane",
                        "-t",
                        caller_pane_id,
                        "-x",
                        &target.to_string(),
                    ])
                    .output();
            }
        }
    }

    // Poll up to 500ms for the new pane's shell to be ready
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        let pid_str =
            run_tmux_command(&["display-message", "-t", &new_pane_id, "-p", "#{pane_pid}"])
                .unwrap_or_default();
        if pid_str
            .trim()
            .parse::<u32>()
            .map(|p| p > 0)
            .unwrap_or(false)
        {
            break;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Restore focus to caller
    let _ = Command::new("tmux")
        .args(["select-pane", "-t", caller_pane_id])
        .output();

    Some(new_pane_id)
}

fn side_edit(file: Option<&str>) -> Result<()> {
    let caller_pane_id = get_caller_pane_id()
        .context("$TMUX_PANE is not set. side-edit must be run inside a tmux pane.")?;

    // No file — status only
    let file = match file {
        Some(f) => f,
        None => {
            let status = get_side_pane_status(&caller_pane_id);
            print_pane_status(&status);
            return Ok(());
        }
    };

    // Parse file:line
    let (raw_path, line) = parse_file_line(file);

    // Resolve file path
    let expanded = if raw_path.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        raw_path.replacen('~', &home, 1)
    } else {
        raw_path
    };
    let file_path = if std::path::Path::new(&expanded).is_absolute() {
        expanded
    } else {
        let cwd = get_pane_cwd(&caller_pane_id);
        let base = if cwd.is_empty() {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string())
        } else {
            cwd
        };
        format!("{}/{}", base, expanded)
    };

    let shell_file = shell_quote(&file_path);
    let vim_file = escape_for_vim_ex(&file_path);

    // Resolve or create the side pane
    let side_pane_id = resolve_side_pane(&caller_pane_id)?;

    let mut system = System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    // Open file in the side pane (handles both nvim-reuse and fresh launch)
    open_file_in_pane(&side_pane_id, &shell_file, &vim_file, line, &system);

    // Restore focus to caller
    let _ = Command::new("tmux")
        .args(["select-pane", "-t", &caller_pane_id])
        .output();

    // Print status
    let status = get_side_pane_status(&caller_pane_id);
    print_pane_status(&status);

    Ok(())
}

fn side_run(command: Option<&str>, force: bool) -> Result<()> {
    let caller_pane_id = get_caller_pane_id()
        .context("$TMUX_PANE is not set. side-run must be run inside a tmux pane.")?;

    // No command — status only
    let cmd = match command {
        Some(c) => c,
        None => {
            let status = get_side_pane_status(&caller_pane_id);
            print_pane_status(&status);
            return Ok(());
        }
    };

    // Resolve or create the side pane
    let side_pane_id = resolve_side_pane(&caller_pane_id)?;

    // Check if nvim is running
    let mut system = System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    // Fail-safe: if inspection fails (None), assume vim *might* be there.
    // Sending Enter-terminated keystrokes into a live nvim with unsaved buffers
    // would silently destroy work, so any state other than `Some(false)` blocks
    // the run unless the user explicitly says `--force`.
    let vim_state = inspect_pane_for_vim(&side_pane_id, &system);
    if vim_state != Some(false) {
        if !force {
            let reason = match vim_state {
                Some(true) => "nvim is running",
                None => "side pane could not be inspected",
                Some(false) => unreachable!(),
            };
            eprintln!(
                "{} in side pane ({}). You may lose unsaved work.\n\
                 Use --force to override and run your command.",
                reason, side_pane_id
            );
            // Still print status so caller gets pane info
            let status = get_side_pane_status(&caller_pane_id);
            print_pane_status(&status);
            anyhow::bail!("side pane unsafe ({}); use --force to override", reason);
        }
        // Force: send :qa! to nvim. Harmless no-op if it wasn't actually nvim,
        // since send-keys to a shell prompt just types `:qa!` and Enter.
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &side_pane_id, "Escape"])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &side_pane_id, "Escape"])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &side_pane_id, r"C-\"])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &side_pane_id, "C-n"])
            .output();
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &side_pane_id, ":qa!", "Enter"])
            .output();
        // Brief wait for nvim to exit
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // Send the command
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &side_pane_id, cmd, "Enter"])
        .output();

    // Restore focus to caller
    let _ = Command::new("tmux")
        .args(["select-pane", "-t", &caller_pane_id])
        .output();

    // Print status
    let status = get_side_pane_status(&caller_pane_id);
    print_pane_status(&status);

    Ok(())
}

/// Debug command to show raw key events
fn debug_keys() -> Result<()> {
    use crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use std::io::{self, Write};

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    println!("Press keys to see events (q to quit)\r");
    println!("=================================\r");

    loop {
        if let Event::Key(key) = event::read()? {
            println!(
                "kind={:?} code={:?} modifiers={:?}\r",
                key.kind, key.code, key.modifiers
            );
            stdout.flush()?;

            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

// ============================================================================
// parent-pid-tree: resolve the caller's owning tmux pane by walking ppid chain
// ============================================================================
//
// Why this exists: `tmux display-message -p '#{pane_id}'` returns the tmux-active
// pane (the one focused in the attached client), not the pane the caller is
// running inside. With multiple Claude sessions in different panes, that primitive
// silently targets the wrong session. The correct primitive is to walk from the
// caller's pid up through ppid (via /proc/<pid>/stat field 4) until we hit a pid
// that matches a `pane_pid` reported by `tmux list-panes`. The first ancestor
// match is deterministically the caller's pane, regardless of focus state.
//
// Architecture: Humble Object pattern. The "shell" that shells out to `tmux`
// or reads `/proc` lives behind the `TmuxProvider` + `ProcReader` traits. All
// walk logic, flag handling, exit-code selection, and output formatting lives
// in `run_parent_pid_tree`, which takes the traits as dependencies. The
// command wrapper (and in turn `main()`) is the only place that constructs
// the `Real*` impls and writes to real stdout/stderr. Tests drive
// `run_parent_pid_tree` with in-memory mocks so every exit code and flag
// combination is reachable without touching tmux or `/proc`.
//
// Scope note: `TmuxProvider` only exposes the primitives `parent-pid-tree`
// needs today. Other tmux call sites in this binary (`side_edit`, `side_run`,
// `rename_all`, `rotate`, `third`) still shell out directly via
// `run_tmux_command` and scattered `Command::new("tmux")` calls. Migrating
// those requires characterization tests that don't exist yet. New tmux code
// should use this trait; old sites can migrate incrementally.

/// Errors surfaced by the `TmuxProvider` humble shell.
///
/// The walker + command layer translate these to concrete exit codes; the
/// shell itself doesn't know about the 0/1/2/3 scheme.
#[derive(Debug)]
pub(crate) enum TmuxError {
    /// tmux is not installed, no server is running, or `list-panes` returned
    /// empty output (e.g. no sessions). Maps to exit code 2.
    NotRunning,
    /// Spawning `tmux` failed or the child exited non-zero with an io-level
    /// error. Preserves the underlying `io::Error` for context. Also exit 2,
    /// with a more specific stderr message.
    ListFailed(std::io::Error),
    /// `tmux` output could not be parsed as expected. Present for
    /// forward-compat — the current line-based parser skips malformed lines
    /// rather than erroring. Also exit 2.
    #[allow(dead_code)]
    ParseFailed(String),
}

impl std::fmt::Display for TmuxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TmuxError::NotRunning => write!(f, "tmux not running or no panes"),
            TmuxError::ListFailed(e) => write!(f, "tmux list-panes failed: {}", e),
            TmuxError::ParseFailed(msg) => write!(f, "tmux output parse failed: {}", msg),
        }
    }
}

/// Humble shell over `tmux` for `parent-pid-tree`. Only primitives this PR
/// needs are defined; add more methods here (and to `RealTmuxProvider`) when
/// migrating other call sites.
pub(crate) trait TmuxProvider {
    /// List every tmux pane's `(pane_id, pane_pid)` across all sessions
    /// (`tmux list-panes -a`). Returns `NotRunning` if tmux is unreachable or
    /// the server has no panes. Order is not guaranteed.
    fn list_pane_pids(&self) -> Result<Vec<(String, u32)>, TmuxError>;

    /// Return the currently tmux-active pane id (focused in the attached
    /// client). This is intentionally NOT what `parent-pid-tree` uses to
    /// answer "which pane am I in" — it exists for explicit active-pane
    /// lookups and future migrations.
    #[allow(dead_code)]
    fn active_pane(&self) -> Result<Option<String>, TmuxError>;

    /// Capture the recent scrollback of `pane_id` via
    /// `tmux capture-pane -p -J -S -<window> -E -`. Returns the captured text.
    /// Errors propagate as `TmuxError::ListFailed` for spawn/read problems,
    /// `TmuxError::NotRunning` if tmux returns non-zero.
    fn capture_pane(&self, pane_id: &str, window: usize) -> Result<String, TmuxError>;
}

/// Humble shell over `/proc/<pid>/stat`. Tests inject a mock that returns a
/// pre-built chain without touching the filesystem.
pub(crate) trait ProcReader {
    /// Return the parent pid of `pid` (field 4 of `/proc/<pid>/stat`). Returns
    /// `None` for pid 0, a vanished process, or an unreadable/unparseable stat
    /// file. `None` is non-fatal to the walker — it just means "stop here".
    fn read_ppid(&self, pid: u32) -> Option<u32>;

    /// Return the full command line (argv joined by spaces) from
    /// `/proc/<pid>/cmdline`. Null bytes in the file are argv separators and
    /// are replaced with spaces; a trailing null is stripped. Returns `None`
    /// when the file is missing, unreadable, or empty (kernel threads).
    fn read_cmdline(&self, pid: u32) -> Option<String>;

    /// Return the short process name from `/proc/<pid>/comm`, trimmed of the
    /// trailing newline. Useful fallback for kernel threads whose
    /// `/proc/<pid>/cmdline` is empty. Returns `None` on read/parse failure.
    fn read_comm(&self, pid: u32) -> Option<String>;

    /// Return the executable path via `readlink /proc/<pid>/exe`. Returns
    /// `None` when the symlink can't be read (process gone, permission denied,
    /// kernel thread).
    fn read_exe(&self, pid: u32) -> Option<PathBuf>;
}

/// Build the argv for `tmux capture-pane -p -J -S -<window> -E - -t <pane_id>`.
/// Pulled out so unit tests can assert the shape without spawning tmux.
pub(crate) fn capture_pane_args(pane_id: &str, window: usize) -> Vec<String> {
    vec![
        "capture-pane".to_string(),
        "-p".to_string(),
        "-J".to_string(),
        "-S".to_string(),
        format!("-{}", window),
        "-E".to_string(),
        "-".to_string(),
        "-t".to_string(),
        pane_id.to_string(),
    ]
}

/// Production implementation of `TmuxProvider` — shells out to the `tmux`
/// binary via `std::process::Command`.
pub(crate) struct RealTmuxProvider;

impl TmuxProvider for RealTmuxProvider {
    fn list_pane_pids(&self) -> Result<Vec<(String, u32)>, TmuxError> {
        let output = Command::new("tmux")
            .args(["list-panes", "-a", "-F", "#{pane_id} #{pane_pid}"])
            .output()
            .map_err(TmuxError::ListFailed)?;
        if !output.status.success() {
            return Err(TmuxError::NotRunning);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let pairs = parse_pane_pid_pairs(&stdout);
        if pairs.is_empty() {
            return Err(TmuxError::NotRunning);
        }
        Ok(pairs)
    }

    fn active_pane(&self) -> Result<Option<String>, TmuxError> {
        let output = Command::new("tmux")
            .args(["display-message", "-p", "#{pane_id}"])
            .output()
            .map_err(TmuxError::ListFailed)?;
        if !output.status.success() {
            return Err(TmuxError::NotRunning);
        }
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    }

    fn capture_pane(&self, pane_id: &str, window: usize) -> Result<String, TmuxError> {
        let args = capture_pane_args(pane_id, window);
        let output = Command::new("tmux")
            .args(args.iter().map(String::as_str))
            .output()
            .map_err(TmuxError::ListFailed)?;
        if !output.status.success() {
            return Err(TmuxError::NotRunning);
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Production implementation of `ProcReader`. Delegates to
/// `read_ppid_from_proc`, whose `rfind(')')`-based parser is load-bearing for
/// `comm` fields containing parens or spaces — do NOT reimplement it inline.
pub(crate) struct RealProcReader;

impl ProcReader for RealProcReader {
    fn read_ppid(&self, pid: u32) -> Option<u32> {
        read_ppid_from_proc(pid)
    }

    fn read_cmdline(&self, pid: u32) -> Option<String> {
        read_cmdline_from_proc(pid)
    }

    fn read_comm(&self, pid: u32) -> Option<String> {
        read_comm_from_proc(pid)
    }

    fn read_exe(&self, pid: u32) -> Option<PathBuf> {
        read_exe_from_proc(pid)
    }
}

/// Read `/proc/<pid>/cmdline` — argv joined by null bytes, with a possible
/// trailing null. Convert null separators to spaces. Returns `None` when the
/// file is absent, unreadable, or entirely empty (kernel threads).
fn read_cmdline_from_proc(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let path = format!("/proc/{}/cmdline", pid);
    let bytes = fs::read(&path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    // Strip trailing NUL if present.
    let trimmed = if bytes.last() == Some(&0) {
        &bytes[..bytes.len() - 1]
    } else {
        &bytes[..]
    };
    if trimmed.is_empty() {
        return None;
    }
    // Replace remaining NULs with spaces, lossy UTF-8.
    let with_spaces: Vec<u8> = trimmed
        .iter()
        .map(|b| if *b == 0 { b' ' } else { *b })
        .collect();
    Some(String::from_utf8_lossy(&with_spaces).into_owned())
}

/// Read `/proc/<pid>/comm` — short process name, trailing newline trimmed.
fn read_comm_from_proc(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let path = format!("/proc/{}/comm", pid);
    let content = fs::read_to_string(&path).ok()?;
    let trimmed = content.trim_end_matches('\n').trim_end_matches('\r');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read the executable path via `readlink /proc/<pid>/exe`. Returns `None` for
/// kernel threads, vanished processes, or permission-denied symlinks.
fn read_exe_from_proc(pid: u32) -> Option<PathBuf> {
    if pid == 0 {
        return None;
    }
    let path = format!("/proc/{}/exe", pid);
    fs::read_link(&path).ok()
}

/// Result of a successful parent-pid walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneMatch {
    pub(crate) pane_id: String,
    pub(crate) pane_pid: u32,
    pub(crate) ancestors_walked: Vec<u32>,
}

/// Read field 4 (ppid) of /proc/<pid>/stat.
///
/// The stat file is space-separated, but field 2 (`comm`) is wrapped in parens
/// and CAN contain spaces or closing-parens inside. Find the LAST `)` to locate
/// the end of comm, then split the rest — field 4 becomes index 1 after the
/// state char.
fn read_ppid_from_proc(pid: u32) -> Option<u32> {
    if pid == 0 {
        return None;
    }
    let path = format!("/proc/{}/stat", pid);
    let bytes = fs::read(&path).ok()?;
    let content = String::from_utf8_lossy(&bytes);
    let last_paren = content.rfind(')')?;
    let after = content.get(last_paren + 1..)?.trim_start();
    // after = "<state> <ppid> <pgrp> ..."
    let mut fields = after.split_ascii_whitespace();
    let _state = fields.next()?;
    let ppid_str = fields.next()?;
    ppid_str.parse().ok()
}

/// Parse `tmux list-panes -a -F '#{pane_id} #{pane_pid}'` output into a list
/// of `(pane_id, pane_pid)` tuples. Malformed lines are silently skipped so
/// a single garbage line from tmux doesn't blow up the whole walk.
fn parse_pane_pid_pairs(output: &str) -> Vec<(String, u32)> {
    let mut pairs = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_ascii_whitespace();
        let Some(pane_id) = parts.next() else {
            continue;
        };
        let Some(pid_str) = parts.next() else {
            continue;
        };
        if let Ok(pid) = pid_str.parse::<u32>() {
            pairs.push((pane_id.to_string(), pid));
        }
    }
    pairs
}

/// Parse `tmux list-panes -a -F '#{pane_id} #{pane_pid}'` output into a map
/// from pane_pid -> pane_id. Kept for test-level coverage of the parser;
/// production code goes through `parse_pane_pid_pairs` inside
/// `RealTmuxProvider::list_pane_pids`.
#[cfg(test)]
fn parse_pane_pids(output: &str) -> HashMap<u32, String> {
    parse_pane_pid_pairs(output)
        .into_iter()
        .map(|(pane_id, pid)| (pid, pane_id))
        .collect()
}

/// Safety cap on walk depth — normal process trees on Linux are well under this.
const PPID_WALK_MAX_DEPTH: usize = 64;

/// Walk from `start_pid` up through parent PIDs until we find one whose pid
/// appears in `pane_pids`. Returns the first match, or None if we reach pid 1/0,
/// hit max depth, fail to read a ppid before matching, or detect a cycle.
///
/// `read_ppid` is injected so tests can provide a fake ancestor chain without
/// touching /proc. The first entry in `ancestors_walked` is always `start_pid`.
///
/// Behavior notes:
/// - If `start_pid` itself is in `pane_pids`, it matches immediately.
/// - If `read_ppid` returns None for a specific pid (vanished process, unreadable
///   stat file), we stop walking — this is graceful, not a panic.
pub(crate) fn resolve_pane_by_parent_chain<F>(
    start_pid: u32,
    pane_pids: &HashMap<u32, String>,
    mut read_ppid: F,
) -> Option<PaneMatch>
where
    F: FnMut(u32) -> Option<u32>,
{
    let mut current = start_pid;
    let mut ancestors: Vec<u32> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();

    for _ in 0..PPID_WALK_MAX_DEPTH {
        if current == 0 || current == 1 {
            // Reached init / sentinel — no pane match.
            if !ancestors.contains(&current) {
                ancestors.push(current);
            }
            return None;
        }
        if !seen.insert(current) {
            // Cycle detection — shouldn't happen with real ppid but be safe.
            return None;
        }
        ancestors.push(current);
        if let Some(pane_id) = pane_pids.get(&current) {
            return Some(PaneMatch {
                pane_id: pane_id.clone(),
                pane_pid: current,
                ancestors_walked: ancestors,
            });
        }
        // Read the next ancestor. If the read fails we stop walking — the
        // process probably exited mid-walk. We don't treat that as a hard error;
        // first-match-wins semantics have already been applied above.
        match read_ppid(current) {
            Some(next) => current = next,
            None => return None,
        }
    }
    None
}

/// CLI-level flags for `parent-pid-tree`. Extracted into a struct so the
/// testable core can be driven with plain values without re-parsing clap.
#[derive(Debug, Clone, Copy)]
struct ParentPidTreeArgs {
    json: bool,
    pid: Option<u32>,
    verbose: bool,
    tree: bool,
}

impl ParentPidTreeArgs {
    /// True when the caller wants the rich per-pid chain details collected.
    /// Both `--tree` (human text or structured JSON) and `--verbose` (JSON
    /// payload inspection) trigger this; either flag makes `--json` emit the
    /// full `chain[]` array. Without either, `--json` stays on the minimal
    /// payload path for scriptability.
    fn wants_rich_chain(&self) -> bool {
        self.tree || self.verbose
    }
}

/// One row of the `--tree` output: a PID in the ancestor chain plus any
/// cheap `/proc/<pid>/` metadata we could harvest. All metadata fields are
/// `Option` because any `/proc` read can fail for kernel threads, vanished
/// processes, or permission-gated targets.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeEntry {
    pid: u32,
    comm: Option<String>,
    cmdline: Option<String>,
    exe: Option<PathBuf>,
}

/// Structured result of running `parent-pid-tree`.
///
/// `main()` is the only place that actually writes to real stdout/stderr —
/// this type makes the command function a pure data transform that tests can
/// assert against. Any non-empty `stdout` is printed as-is with a trailing
/// newline, and every `stderr_lines` entry is printed on its own line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParentPidTreeOutcome {
    /// Line to print to stdout (matched pane id or JSON blob). Empty on the
    /// failure exit codes 1/2/3.
    stdout: String,
    /// Lines to print to stderr. `--verbose` adds walk-chain entries; error
    /// paths add a human-readable message.
    stderr_lines: Vec<String>,
    /// Concrete exit code. See `run_parent_pid_tree` for the contract.
    exit_code: i32,
    /// Populated only when `--tree` is set. Ordered from the start pid at
    /// index 0 down to the pane-matching pid (or the last pid walked if no
    /// match). `None` when `--tree` was not requested.
    tree: Option<Vec<TreeEntry>>,
    /// When `--tree` is set and the walk matched a pane, this is the matched
    /// pane id. Lets the formatter annotate the leaf without re-walking.
    /// `None` when tree was not requested or no pane was matched.
    tree_pane_id: Option<String>,
    /// When `--tree` is set and the walk matched a pane, this is the matched
    /// pane_pid. `None` when tree was not requested or no match.
    tree_pane_pid: Option<u32>,
    /// The start pid of the walk, captured so the JSON tree payload can
    /// report it. `None` when tree was not requested or the walk never
    /// started (e.g. exit 2/3 paths).
    tree_start_pid: Option<u32>,
}

/// Testable core of `parent-pid-tree`. Takes humble-shell dependencies so
/// every branch is reachable without touching tmux or `/proc`.
///
/// Exit code contract (mirrored in CLAUDE.md — keep in sync):
/// - `0` — pane found, `stdout` holds the pane id or JSON payload
/// - `1` — no match (walker exhausted chain without finding a pane_pid)
/// - `2` — tmux not running / list-panes failed / empty pane set
/// - `3` — could not read self's ppid (default-start-pid path only)
///
/// `self_pid` is passed explicitly instead of calling `std::process::id()`
/// internally so tests can control the "read my own ppid" branch. Production
/// callers pass `std::process::id()`.
fn run_parent_pid_tree(
    args: ParentPidTreeArgs,
    self_pid: u32,
    tmux: &dyn TmuxProvider,
    proc: &dyn ProcReader,
) -> ParentPidTreeOutcome {
    let mut stderr_lines: Vec<String> = Vec::new();

    // 1. Fetch pane_pid map from tmux.
    let pane_pids: HashMap<u32, String> = match tmux.list_pane_pids() {
        Ok(pairs) => pairs
            .into_iter()
            .map(|(pane_id, pid)| (pid, pane_id))
            .collect(),
        Err(TmuxError::NotRunning) => {
            stderr_lines.push("tmux not running or no panes".to_string());
            return ParentPidTreeOutcome {
                stdout: String::new(),
                stderr_lines,
                exit_code: 2,
                tree: None,
                tree_pane_id: None,
                tree_pane_pid: None,
                tree_start_pid: None,
            };
        }
        Err(e) => {
            stderr_lines.push(format!("tmux not running or no panes: {}", e));
            return ParentPidTreeOutcome {
                stdout: String::new(),
                stderr_lines,
                exit_code: 2,
                tree: None,
                tree_pane_id: None,
                tree_pane_pid: None,
                tree_start_pid: None,
            };
        }
    };

    // 2. Determine start pid. When the user passed --pid we trust it verbatim.
    // Otherwise we walk from the PARENT of rmux_helper itself: rmux_helper is
    // a child of whoever invoked it, and that caller is what we want to resolve.
    let start_pid = match args.pid {
        Some(p) => p,
        None => match proc.read_ppid(self_pid) {
            Some(p) => p,
            None => {
                stderr_lines.push(format!(
                    "could not read /proc/{}/stat to find caller pid",
                    self_pid
                ));
                return ParentPidTreeOutcome {
                    stdout: String::new(),
                    stderr_lines,
                    exit_code: 3,
                    tree: None,
                    tree_pane_id: None,
                    tree_pane_pid: None,
                    tree_start_pid: None,
                };
            }
        },
    };

    if args.verbose {
        stderr_lines.push(format!(
            "parent-pid-tree: starting walk at pid {}",
            start_pid
        ));
    }

    // 3. Walk the chain. The walker takes its own read_ppid closure, which we
    //    adapt from the injected `ProcReader`. When --tree or --verbose is set
    //    we also capture the full walked chain even on no-match, so the tree
    //    view (and rich JSON) can help the user debug why resolution failed.
    let wants_rich = args.wants_rich_chain();
    let mut walked_chain: Vec<u32> = Vec::new();
    let result = resolve_pane_by_parent_chain(start_pid, &pane_pids, |p| {
        let next = proc.read_ppid(p);
        if wants_rich {
            if let Some(n) = next {
                walked_chain.push(n);
            }
        }
        next
    });

    match result {
        Some(m) => {
            if args.verbose {
                let chain: Vec<String> = m.ancestors_walked.iter().map(|p| p.to_string()).collect();
                stderr_lines.push(format!(
                    "parent-pid-tree: walked {} (pane_pid) -> pane {}",
                    chain.join(" -> "),
                    m.pane_id
                ));
            }
            // Collect rich chain metadata whenever the caller asked for it —
            // either via --tree (text or JSON) or --verbose (JSON payload).
            let tree_data = if wants_rich {
                Some(collect_tree_entries(&m.ancestors_walked, proc))
            } else {
                None
            };
            let stdout = if args.tree && !args.json {
                // Human-readable tree text only when --tree is explicit.
                let entries = tree_data.as_deref().unwrap_or(&[]);
                format_tree_text(entries, Some(&m.pane_id))
            } else if args.json && wants_rich {
                // Rich JSON: --tree --json OR --verbose --json produces the
                // same payload shape. --verbose also leaves walk lines on
                // stderr (above) for human inspection.
                let entries = tree_data.as_deref().unwrap_or(&[]);
                format_tree_json(start_pid, Some(&m.pane_id), Some(m.pane_pid), entries)
            } else if args.json {
                // Minimal JSON — scriptable contract preserved when neither
                // --tree nor --verbose is set.
                let payload = serde_json::json!({
                    "pane_id": m.pane_id,
                    "pane_pid": m.pane_pid,
                    "walked_from_pid": start_pid,
                    "ancestors_walked": m.ancestors_walked,
                });
                payload.to_string()
            } else {
                // Plain text: just the pane id. --verbose alone does not
                // change stdout; its effect lives entirely on stderr.
                m.pane_id.clone()
            };
            ParentPidTreeOutcome {
                stdout,
                stderr_lines,
                exit_code: 0,
                tree: tree_data,
                tree_pane_id: if args.tree {
                    Some(m.pane_id.clone())
                } else {
                    None
                },
                tree_pane_pid: if args.tree { Some(m.pane_pid) } else { None },
                tree_start_pid: if args.tree { Some(start_pid) } else { None },
            }
        }
        None => {
            if args.verbose {
                stderr_lines.push(format!(
                    "parent-pid-tree: no pane match for pid {} (walked until init/unreadable/max-depth)",
                    start_pid
                ));
            }
            stderr_lines.push(format!("no tmux pane found for pid {}", start_pid));
            // On no-match, rebuild the chain walked: start_pid plus whatever
            // ancestors the walker consumed before stopping. We didn't have a
            // `PaneMatch` to harvest from, so synthesize from `walked_chain`.
            let tree_data = if wants_rich {
                let mut chain: Vec<u32> = Vec::with_capacity(walked_chain.len() + 1);
                chain.push(start_pid);
                for p in &walked_chain {
                    if !chain.contains(p) {
                        chain.push(*p);
                    }
                }
                Some(collect_tree_entries(&chain, proc))
            } else {
                None
            };
            let stdout = if args.tree && !args.json {
                let entries = tree_data.as_deref().unwrap_or(&[]);
                format_tree_text(entries, None)
            } else if args.json && wants_rich {
                let entries = tree_data.as_deref().unwrap_or(&[]);
                format_tree_json(start_pid, None, None, entries)
            } else {
                // Minimal --json on no-match keeps the historical empty-stdout
                // contract (exit 1 signals failure). Plain text is also empty.
                String::new()
            };
            ParentPidTreeOutcome {
                stdout,
                stderr_lines,
                exit_code: 1,
                tree: tree_data,
                tree_pane_id: None,
                tree_pane_pid: None,
                tree_start_pid: if args.tree { Some(start_pid) } else { None },
            }
        }
    }
}

/// Harvest `/proc/<pid>/` metadata for each pid in the chain. Pure data
/// transform — all external reads go through the injected `ProcReader`.
fn collect_tree_entries(chain: &[u32], proc: &dyn ProcReader) -> Vec<TreeEntry> {
    chain
        .iter()
        .map(|&pid| TreeEntry {
            pid,
            comm: proc.read_comm(pid),
            cmdline: proc.read_cmdline(pid),
            exe: proc.read_exe(pid),
        })
        .collect()
}

/// Max width (chars) we truncate cmdline output to in the text tree view.
/// Users who need the full cmdline should use `--tree --json`.
const TREE_CMDLINE_MAX_WIDTH: usize = 120;

/// Render an ASCII tree for the chain. `pane_id` is `Some` when the walk
/// matched a tmux pane — the leaf entry is annotated with `(pane shell)` and
/// a `tmux pane:` line. When `None`, no annotations are added.
fn format_tree_text(entries: &[TreeEntry], pane_id: Option<&str>) -> String {
    let mut out = String::from("parent-pid-tree\n");
    let n = entries.len();
    if n == 0 {
        return out;
    }
    // Compute the longest comm for column alignment (bounded to a reasonable
    // width so absurd comms don't blow out the layout).
    let comm_col = entries
        .iter()
        .map(|e| e.comm.as_deref().unwrap_or("?").len())
        .max()
        .unwrap_or(1)
        .min(16);
    for (i, entry) in entries.iter().enumerate() {
        let is_leaf = i == n - 1;
        let is_start = i == 0;
        let branch = if is_leaf { "└─" } else { "├─" };
        let cont = if is_leaf { "  " } else { "│ " };
        let comm = entry.comm.as_deref().unwrap_or("?");
        let cmd_display = match entry.cmdline.as_deref() {
            Some(c) if !c.is_empty() => truncate_display(c, TREE_CMDLINE_MAX_WIDTH),
            // No cmdline — convention for kernel threads is [comm].
            _ => format!("[{}]", comm),
        };
        // Annotate the start/leaf positions so users can see at a glance
        // where the walk started and where it stopped. Single-entry chains
        // collapse into a combined annotation. On no-match (`pane_id` is
        // None) the leaf is labeled `(no pane found)` instead of
        // `(pane shell)`.
        let annotation = match (is_start, is_leaf, pane_id.is_some()) {
            (true, true, true) => "  (start, pane shell)",
            (true, true, false) => "  (start, no pane found)",
            (true, false, _) => "  (start)",
            (false, true, true) => "  (pane shell)",
            (false, true, false) => "  (no pane found)",
            _ => "",
        };
        out.push_str(&format!(
            "{} [pid {:<7}] {:<width$}  {}{}\n",
            branch,
            entry.pid,
            comm,
            cmd_display,
            annotation,
            width = comm_col,
        ));
        match entry.exe.as_ref() {
            Some(path) => {
                out.push_str(&format!("{}    exe: {}\n", cont, path.display()));
            }
            None => {
                out.push_str(&format!("{}    exe: (unreadable)\n", cont));
            }
        }
        if is_leaf {
            if let Some(pid) = pane_id {
                out.push_str(&format!("{}    tmux pane: {}\n", cont, pid));
            }
        }
    }
    out
}

/// Render the tree as JSON. `pane_id` / `pane_pid` are None when the walk
/// didn't match a pane (useful for debugging why resolution failed).
///
/// Each chain entry carries a `role` string derived from its position and
/// whether its pid matches `pane_pid`:
///   - `"start"` — first entry of a multi-entry chain
///   - `"ancestor"` — interior entries
///   - `"pane_shell"` — leaf whose pid matches `pane_pid`
///   - `"start_and_pane_shell"` — single-entry chain where start IS the pane
///   - `"walked_past_root"` — leaf of a no-match chain (`pane_pid` is None)
fn format_tree_json(
    start_pid: u32,
    pane_id: Option<&str>,
    pane_pid: Option<u32>,
    entries: &[TreeEntry],
) -> String {
    let n = entries.len();
    let chain: Vec<serde_json::Value> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let role = chain_entry_role(i, n, e.pid, pane_pid);
            serde_json::json!({
                "pid": e.pid,
                "comm": e.comm,
                "cmdline": e.cmdline,
                "exe": e.exe.as_ref().map(|p| p.display().to_string()),
                "role": role,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "start_pid": start_pid,
        "pane_id": pane_id,
        "pane_pid": pane_pid,
        "chain": chain,
    });
    payload.to_string()
}

/// Pure helper: derive the `role` string for chain entry at `index` given
/// the chain length, the entry's pid, and the resolved `pane_pid` (if any).
/// Extracted so tests can assert the role-assignment policy directly.
fn chain_entry_role(index: usize, len: usize, pid: u32, pane_pid: Option<u32>) -> &'static str {
    let is_first = index == 0;
    let is_last = index + 1 == len;
    let matches_pane = pane_pid.is_some_and(|pp| pp == pid);
    match (is_first, is_last, matches_pane, pane_pid.is_some()) {
        // Single-entry chain where start IS the pane shell.
        (true, true, true, _) => "start_and_pane_shell",
        // Any leaf without a pane match — walker stopped here empty-handed.
        (_, true, false, false) => "walked_past_root",
        // Multi-entry leaf that matches pane_pid is the pane shell.
        (false, true, true, _) => "pane_shell",
        // First entry of a multi-entry chain.
        (true, false, _, _) => "start",
        // Everything else — interior entries.
        _ => "ancestor",
    }
}

/// Truncate `s` to at most `max` display chars, appending `…` if cut. Uses
/// char count, not byte count — this is `/proc/<pid>/cmdline` so worst-case
/// we have multi-byte UTF-8 (rare in practice for binary paths/argv).
fn truncate_display(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    // Reserve one char for the ellipsis marker.
    let keep = max.saturating_sub(1);
    let mut out: String = s.chars().take(keep).collect();
    out.push('…');
    out
}

/// Thin wrapper that wires the real humble-shell impls into the testable core
/// and performs the actual stdout/stderr writes. `main()` calls this; tests
/// call `run_parent_pid_tree` directly with mocks.
fn parent_pid_tree_cmd(json: bool, pid: Option<u32>, verbose: bool, tree: bool) -> i32 {
    let args = ParentPidTreeArgs {
        json,
        pid,
        verbose,
        tree,
    };
    let tmux = RealTmuxProvider;
    let proc = RealProcReader;
    let outcome = run_parent_pid_tree(args, std::process::id(), &tmux, &proc);
    for line in &outcome.stderr_lines {
        eprintln!("{}", line);
    }
    if !outcome.stdout.is_empty() {
        // Tree text output already contains trailing newlines per row; avoid
        // adding an extra blank line for the multi-line tree view. For the
        // single-line (non-tree) output path we keep the historical newline
        // behavior.
        if outcome.stdout.ends_with('\n') {
            print!("{}", outcome.stdout);
        } else {
            println!("{}", outcome.stdout);
        }
    }
    outcome.exit_code
}

// ---------------------------------------------------------------------------
// Shell completions
// ---------------------------------------------------------------------------

/// Shells for which `install-completions` knows a conventional install path
/// and writes a completion script. `Powershell` and `Elvish` are accepted for
/// `--print-only` use but do not have a default install path (noted in docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CompletionShell {
    Zsh,
    Bash,
    Fish,
    Powershell,
    Elvish,
}

impl CompletionShell {
    fn as_str(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Bash => "bash",
            Self::Fish => "fish",
            Self::Powershell => "powershell",
            Self::Elvish => "elvish",
        }
    }
}

/// Pure function: map a `$SHELL`-style string (e.g. `/bin/zsh`, `fish`) to
/// a `CompletionShell`. Returns `None` on unknown/empty input so the caller
/// can emit a helpful `--shell` suggestion.
fn detect_shell_from_env(shell: Option<&str>) -> Option<CompletionShell> {
    let s = shell?;
    let basename = s.rsplit('/').next().unwrap_or(s).trim();
    if basename.is_empty() {
        return None;
    }
    match basename {
        "zsh" => Some(CompletionShell::Zsh),
        "bash" => Some(CompletionShell::Bash),
        "fish" => Some(CompletionShell::Fish),
        "pwsh" | "powershell" => Some(CompletionShell::Powershell),
        "elvish" => Some(CompletionShell::Elvish),
        _ => None,
    }
}

/// Environment snapshot used by `completion_install_path`. Extracted to a
/// struct so tests can inject values without mutating the real process env.
#[derive(Debug, Default, Clone)]
struct EnvSnapshot {
    pub home: Option<String>,
    pub zdotdir: Option<String>,
    pub xdg_data_home: Option<String>,
    pub xdg_config_home: Option<String>,
}

impl EnvSnapshot {
    fn from_env() -> Self {
        Self {
            home: std::env::var("HOME").ok(),
            zdotdir: std::env::var("ZDOTDIR").ok(),
            xdg_data_home: std::env::var("XDG_DATA_HOME").ok(),
            xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
        }
    }
}

/// Resolve the conventional install path for a shell's `rmux_helper`
/// completion file. Returns `None` for shells without a default location
/// (powershell / elvish) or when `$HOME` is unset and no XDG override is
/// available.
fn completion_install_path(shell: CompletionShell, env: &EnvSnapshot) -> Option<PathBuf> {
    match shell {
        CompletionShell::Zsh => {
            let base = env
                .zdotdir
                .as_deref()
                .or(env.home.as_deref())
                .map(PathBuf::from)?;
            Some(base.join(".zfunc").join("_rmux_helper"))
        }
        CompletionShell::Bash => {
            let base = if let Some(x) = env.xdg_data_home.as_deref() {
                PathBuf::from(x)
            } else {
                let home = env.home.as_deref()?;
                PathBuf::from(home).join(".local").join("share")
            };
            Some(
                base.join("bash-completion")
                    .join("completions")
                    .join("rmux_helper"),
            )
        }
        CompletionShell::Fish => {
            let base = if let Some(x) = env.xdg_config_home.as_deref() {
                PathBuf::from(x)
            } else {
                let home = env.home.as_deref()?;
                PathBuf::from(home).join(".config")
            };
            Some(
                base.join("fish")
                    .join("completions")
                    .join("rmux_helper.fish"),
            )
        }
        CompletionShell::Powershell | CompletionShell::Elvish => None,
    }
}

/// Friendly post-install note per shell — printed on successful write so the
/// user knows what (if anything) they need to do to activate completions.
fn completion_friendly_note(shell: CompletionShell) -> &'static str {
    match shell {
        CompletionShell::Zsh => {
            "Make sure ~/.zfunc is in your $fpath and `autoload -Uz compinit && compinit` has run. Typically added to ~/.zshrc."
        }
        CompletionShell::Bash => {
            "Sourced automatically if bash-completion is installed (apt install bash-completion on Debian/Ubuntu, brew install bash-completion@2 on macOS)."
        }
        CompletionShell::Fish => "Loaded automatically on next fish shell start.",
        CompletionShell::Powershell | CompletionShell::Elvish => {
            "Pipe the output to your shell's profile; no default install path."
        }
    }
}

/// Generate the dynamic-completion registration script for a shell. Invokes
/// the current binary with `COMPLETE=<shell>` which `clap_complete`'s
/// `CompleteEnv` intercepts and emits the script on stdout.
///
/// The script is a small wrapper that calls back into this binary at tab-time
/// with `COMPLETE=<shell> <args>`, so all completion values — including the
/// live pid list for `parent-pid-tree --pid` — are resolved dynamically.
fn generate_completion_script(shell: CompletionShell) -> Result<String> {
    let exe = std::env::current_exe().context("failed to locate rmux_helper binary")?;
    let output = Command::new(&exe)
        .env("COMPLETE", shell.as_str())
        .output()
        .with_context(|| {
            format!(
                "failed to spawn {} for completion generation",
                exe.display()
            )
        })?;
    if !output.status.success() {
        anyhow::bail!(
            "rmux_helper exited {} while generating {} completions: {}",
            output.status,
            shell.as_str(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn install_completions_cmd(
    shell: Option<CompletionShell>,
    print_only: bool,
    dry_run: bool,
) -> Result<()> {
    let resolved = match shell {
        Some(s) => s,
        None => {
            let env_shell = std::env::var("SHELL").ok();
            detect_shell_from_env(env_shell.as_deref()).ok_or_else(|| {
                anyhow::anyhow!(
                    "could not detect shell from $SHELL (got {:?}); pass --shell zsh|bash|fish|powershell|elvish",
                    env_shell
                )
            })?
        }
    };

    let script = generate_completion_script(resolved)?;

    if print_only {
        print!("{}", script);
        return Ok(());
    }

    let env = EnvSnapshot::from_env();
    let target = completion_install_path(resolved, &env).ok_or_else(|| {
        anyhow::anyhow!(
            "no default install path for {}; re-run with --print-only and pipe to your shell's profile",
            resolved.as_str()
        )
    })?;

    if dry_run {
        println!(
            "would install {} completions to {}",
            resolved.as_str(),
            target.display()
        );
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create completion dir {}", parent.display()))?;
    }
    fs::write(&target, &script).with_context(|| format!("failed to write {}", target.display()))?;
    println!(
        "installed {} completions to {}",
        resolved.as_str(),
        target.display()
    );
    println!("note: {}", completion_friendly_note(resolved));
    Ok(())
}

/// Dynamic completion callback for `parent-pid-tree --pid`.
///
/// Enumerates running pids from `/proc`, filters by the user's partial input,
/// and annotates each candidate with the process's `comm` for a readable
/// tab-complete menu. Capped at 500 candidates to avoid flooding the terminal
/// on a busy box.
///
/// Intentionally tolerant: `/proc` missing (macOS, BSD) or unreadable returns
/// an empty list — never panic during completion.
fn pid_completer(current: &OsStr) -> Vec<CompletionCandidate> {
    let current = current.to_string_lossy();
    let entries = enumerate_pid_candidates(&RealProcReader, Path::new("/proc"), 500);
    entries
        .into_iter()
        .filter(|(pid, _)| current.is_empty() || pid.to_string().starts_with(current.as_ref()))
        .map(|(pid, comm)| {
            let mut c = CompletionCandidate::new(pid.to_string());
            if let Some(name) = comm {
                c = c.help(Some(name.into()));
            }
            c
        })
        .collect()
}

/// Pure-ish helper that enumerates `(pid, comm)` pairs from a `/proc`-shaped
/// directory. Dependency-injected over `ProcReader` + a root path so tests
/// can feed a tempdir layout without touching the real filesystem.
///
/// Sort order: pid descending (newest first) so the most recently spawned
/// processes appear at the top of the completion menu. Truncated at `cap`.
fn enumerate_pid_candidates(
    proc: &dyn ProcReader,
    proc_root: &Path,
    cap: usize,
) -> Vec<(u32, Option<String>)> {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut pids: Vec<u32> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()))
        .collect();
    pids.sort_unstable_by(|a, b| b.cmp(a));
    pids.truncate(cap);
    pids.into_iter()
        .map(|pid| {
            let comm = proc.read_comm(pid);
            (pid, comm)
        })
        .collect()
}

fn main() -> Result<()> {
    // Intercept completion requests (COMPLETE=<shell> rmux_helper ...). When
    // the env var is set clap_complete handles the request and exits; when
    // unset this is a no-op and we proceed to regular arg parsing.
    clap_complete::CompleteEnv::with_factory(|| {
        use clap::CommandFactory;
        Cli::command()
    })
    .complete();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::RenameAll) => rename_all(),
        Some(Commands::Info) => info(),
        Some(Commands::Rotate) => rotate(),
        Some(Commands::Third { command }) => third(&command),
        Some(Commands::PickTui) => picker::pick_tui(),
        Some(Commands::SideEdit { file }) => side_edit(file.as_deref()),
        Some(Commands::SideRun { command, force }) => side_run(command.as_deref(), force),
        Some(Commands::DebugKeys) => debug_keys(),
        Some(Commands::PickLinks {
            json,
            enrich_deadline_ms,
        }) => link_picker::pick_links(json, enrich_deadline_ms),
        Some(Commands::ParentPidTree {
            json,
            pid,
            verbose,
            tree,
        }) => {
            let code = parent_pid_tree_cmd(json, pid, verbose, tree);
            std::process::exit(code);
        }
        Some(Commands::InstallCompletions {
            shell,
            print_only,
            dry_run,
        }) => install_completions_cmd(shell, print_only, dry_run),
        Some(Commands::AgentContinue { window, dry_run }) => {
            std::process::exit(agent_continue::cmd(false, window, dry_run));
        }
        Some(Commands::AgentYoloContinue { window, dry_run }) => {
            std::process::exit(agent_continue::cmd(true, window, dry_run));
        }
        None => {
            // Show help when no command given
            use clap::CommandFactory;
            Cli::command().print_long_help()?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_process_info(name: &str, cmdline: &str, children: Vec<ProcessInfo>) -> ProcessInfo {
        make_process_info_full(1, name, cmdline, None, children)
    }

    fn make_process_info_full(
        pid: u32,
        name: &str,
        cmdline: &str,
        exe: Option<&str>,
        children: Vec<ProcessInfo>,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cmdline: cmdline.to_string(),
            exe: exe.map(|s| s.to_string()),
            cwd: "/home/user/project".to_string(),
            children,
        }
    }

    #[test]
    fn test_process_tree_has_pattern_direct() {
        let info = make_process_info("claude", "claude --help", vec![]);
        assert!(process_tree_has_pattern(&info, &["claude"]));
        assert!(!process_tree_has_pattern(&info, &["vim"]));
    }

    #[test]
    fn test_process_tree_has_pattern_in_child() {
        let child = make_process_info("claude", "claude", vec![]);
        let parent = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert!(process_tree_has_pattern(&parent, &["claude"]));
    }

    #[test]
    fn test_generate_title_claude() {
        let child = make_process_info("claude", "@anthropic-ai/claude-code", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "myproject"),
            Some("cl myproject".to_string())
        );
        assert_eq!(generate_title(&info, "blog"), Some("cl blog".to_string()));
    }

    #[test]
    fn test_generate_title_codex() {
        let child = make_process_info("node", "@openai/codex", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "myproject"),
            Some("cx myproject".to_string())
        );
        assert_eq!(generate_title(&info, "blog"), Some("cx blog".to_string()));
    }

    #[test]
    fn test_generate_title_vim() {
        let child = make_process_info("nvim", "nvim file.rs", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "myproject"),
            Some("vi myproject".to_string())
        );
    }

    #[test]
    fn test_generate_title_plain_shell() {
        let info = make_process_info("zsh", "/bin/zsh", vec![]);
        assert_eq!(
            generate_title(&info, "myproject"),
            Some("z myproject".to_string())
        );
    }

    #[test]
    fn test_generate_title_docker() {
        let child = make_process_info("docker", "docker run nginx", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "myproject"),
            Some("docker myproject".to_string())
        );
    }

    #[test]
    fn test_generate_title_unknown_child_returns_none() {
        // Shell with unknown child should return None (use tmux fallback)
        let child = make_process_info("btm", "btm", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(generate_title(&info, "myproject"), None);
    }

    #[test]
    fn test_generate_title_just_with_subcommand() {
        // just with subcommand should show "j <subcommand> <path>"
        let child = make_process_info("just", "just dev", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "blog"),
            Some("j dev blog".to_string())
        );
    }

    #[test]
    fn test_generate_title_just_bare() {
        // just without subcommand should show "j <path>"
        let child = make_process_info("just", "just", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(generate_title(&info, "blog"), Some("j blog".to_string()));
    }

    #[test]
    fn test_generate_title_jekyll() {
        // jekyll should show "jekyll <path>"
        let child = make_process_info("jekyll", "jekyll serve", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "blog"),
            Some("jekyll blog".to_string())
        );
    }

    #[test]
    fn test_generate_title_just_jekyll_serve() {
        // "just jekyll-serve" should shorten to "j jekyll <path>"
        let child = make_process_info("just", "just jekyll-serve", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(
            generate_title(&info, "blog"),
            Some("j jekyll blog".to_string())
        );
    }

    #[test]
    fn test_generate_claude_title_with_pane_title() {
        // With pane title, format is path:pane
        let ctx = TitleContext {
            short_path: "blog",
            pane_title: Some("fix-auth"),
            window_width: 40,
        };
        assert_eq!(generate_ai_title(&ctx, "cl"), "blog:fix-auth");
    }

    #[test]
    fn test_generate_claude_title_strips_star_prefix() {
        // Should strip ✳ prefix from pane titles
        let ctx = TitleContext {
            short_path: "blog",
            pane_title: Some("✳ PR Review Workflow"),
            window_width: 40,
        };
        assert_eq!(generate_ai_title(&ctx, "cl"), "blog:PR Review Workflow");
    }

    #[test]
    fn test_generate_claude_title_compacts_long_path() {
        // Long paths get compacted to 2..2 format
        let ctx = TitleContext {
            short_path: "magic-monitor",
            pane_title: Some("Screen Effects"),
            window_width: 40,
        };
        // magic-monitor (13 chars) -> ma..or (6 chars)
        assert_eq!(generate_ai_title(&ctx, "cl"), "ma..or:Screen Effects");
    }

    #[test]
    fn test_generate_claude_title_no_pane_title() {
        // Without pane title, add label and show richer path
        let ctx = TitleContext {
            short_path: "blog",
            pane_title: None,
            window_width: 40,
        };
        assert_eq!(generate_ai_title(&ctx, "cl"), "cl blog");
    }

    #[test]
    fn test_generate_claude_title_narrow_window() {
        // With narrow window, should truncate pane title
        let ctx = TitleContext {
            short_path: "blog",
            pane_title: Some("fix-auth"),
            window_width: 15,
        };
        // blog: = 5 chars, leaves 10 for pane
        assert_eq!(generate_ai_title(&ctx, "cl"), "blog:fix-auth");
    }

    #[test]
    fn test_compact_path() {
        // Short paths stay as-is
        assert_eq!(compact_path("blog"), "blog");
        assert_eq!(compact_path("blog2"), "blog2");
        assert_eq!(compact_path("ab"), "ab");
        // Long paths get compacted
        assert_eq!(compact_path("magic-monitor"), "ma..or");
        assert_eq!(compact_path("settings"), "se..gs");
        assert_eq!(compact_path("idvorkin"), "id..in");
    }

    #[test]
    fn test_shorten_pane_title() {
        assert_eq!(shorten_pane_title("hello", 10), "hello");
        assert_eq!(shorten_pane_title("hello", 5), "hello");
        assert_eq!(shorten_pane_title("hello", 4), "hel…");
        assert_eq!(shorten_pane_title("hello", 1), "…");
        assert_eq!(shorten_pane_title("hello", 0), "");
        assert_eq!(shorten_pane_title("", 10), "");
    }

    #[test]
    fn test_shorten_path_middle_repo_depth() {
        assert_eq!(
            shorten_path_middle("repo/subdir/leaf", 30),
            "repo/subdir/leaf"
        );
        assert_eq!(shorten_path_middle("repo/subdir/leaf", 12), "repo/…/leaf");
    }

    #[test]
    fn test_shorten_path_middle_home_path() {
        assert_eq!(
            shorten_path_middle("~/deep/path/leaf", 20),
            "~/deep/path/leaf"
        );
        assert_eq!(shorten_path_middle("~/deep/path/leaf", 10), "~/…/leaf");
    }

    #[test]
    fn test_shorten_path_middle_absolute_path() {
        assert_eq!(shorten_path_middle("/var/www/app", 20), "/var/www/app");
        assert_eq!(shorten_path_middle("/var/www/app", 8), "/…/app");
    }

    #[test]
    fn test_shorten_path_middle_tight_widths() {
        assert_eq!(shorten_path_middle("repo/leaf", 1), "…");
        assert_eq!(shorten_path_middle("repo/leaf", 4), "repo");
        assert_eq!(shorten_path_middle("repo/leaf", 6), "repo");
    }

    #[test]
    fn test_format_ai_title_no_pane_rich_path() {
        assert_eq!(
            format_ai_title_no_pane("repo/subdir/leaf", "cx", 20),
            "cx repo/subdir/leaf"
        );
        assert_eq!(
            format_ai_title_no_pane("repo/subdir/leaf", "cx", 12),
            "cx repo/…/l…"
        );
    }

    #[test]
    fn test_parse_file_line_with_line() {
        assert_eq!(
            parse_file_line("foo.py:42"),
            ("foo.py".to_string(), Some(42))
        );
    }

    #[test]
    fn test_parse_file_line_no_line() {
        assert_eq!(parse_file_line("foo.py"), ("foo.py".to_string(), None));
    }

    #[test]
    fn test_parse_file_line_absolute_path() {
        assert_eq!(
            parse_file_line("/home/user/foo.py:10"),
            ("/home/user/foo.py".to_string(), Some(10))
        );
    }

    #[test]
    fn test_parse_file_line_not_a_number() {
        assert_eq!(
            parse_file_line("foo.py:bar"),
            ("foo.py:bar".to_string(), None)
        );
    }

    #[test]
    fn test_parse_file_line_zero() {
        // Line 0 is invalid, treat as no line
        assert_eq!(parse_file_line("foo.py:0"), ("foo.py:0".to_string(), None));
    }

    #[test]
    fn test_parse_file_line_colon_only() {
        assert_eq!(parse_file_line("foo.py:"), ("foo.py:".to_string(), None));
    }

    // ---- Side-pane vim detection ----

    #[test]
    fn test_basename_is_vim_plain() {
        assert!(basename_is_vim("nvim"));
        assert!(basename_is_vim("vim"));
        assert!(basename_is_vim("NVIM")); // case-insensitive
    }

    #[test]
    fn test_basename_is_vim_variants() {
        // Wrappers and packaged binaries should match.
        assert!(basename_is_vim("nvim.appimage"));
        assert!(basename_is_vim("nvim-qt"));
        assert!(basename_is_vim("vim.basic"));
        assert!(basename_is_vim("vim-tiny"));
    }

    #[test]
    fn test_basename_is_vim_negatives() {
        // Names that contain "vim" but aren't a vim binary should not match.
        assert!(!basename_is_vim("vimrc"));
        assert!(!basename_is_vim("nvimpager"));
        assert!(!basename_is_vim("notvim"));
        assert!(!basename_is_vim("zsh"));
        assert!(!basename_is_vim(""));
        // Hyphenated tools that are NOT editors must not match — these would
        // otherwise short-circuit `find_nvim_in_tree` and return the wrong pid.
        assert!(!basename_is_vim("vim-addon-manager"));
        assert!(!basename_is_vim("nvim-lsp-installer"));
        assert!(!basename_is_vim("vim-startuptime"));
        assert!(!basename_is_vim("nvim-treesitter-cli"));
    }

    #[test]
    fn test_find_nvim_in_tree_direct() {
        let info = make_process_info_full(42, "nvim", "nvim foo.rs", None, vec![]);
        assert_eq!(find_nvim_in_tree(&info), Some(42));
    }

    #[test]
    fn test_find_nvim_in_tree_in_child() {
        let child = make_process_info_full(99, "nvim", "nvim file.rs", None, vec![]);
        let parent = make_process_info_full(10, "zsh", "/bin/zsh", None, vec![child]);
        // Should return the child's pid, not the shell's.
        assert_eq!(find_nvim_in_tree(&parent), Some(99));
    }

    #[test]
    fn test_find_nvim_in_tree_appimage_via_exe() {
        // sysinfo's `name` may be a generic loader; rely on `exe` basename.
        let info = make_process_info_full(
            7,
            "AppRun",
            "/tmp/.mount/AppRun",
            Some("/opt/nvim/nvim.appimage"),
            vec![],
        );
        assert_eq!(find_nvim_in_tree(&info), Some(7));
    }

    #[test]
    fn test_find_nvim_in_tree_no_match_does_not_pick_shell() {
        // A shell whose cmdline accidentally contains "vim" must NOT be picked
        // as the nvim pid — that would yield a wrong /proc/<pid>/cmdline.
        let parent = make_process_info_full(10, "zsh", "/bin/zsh -c 'edit vimrc'", None, vec![]);
        assert_eq!(find_nvim_in_tree(&parent), None);
    }

    #[test]
    fn test_find_nvim_in_tree_empty() {
        let info = make_process_info_full(1, "zsh", "/bin/zsh", None, vec![]);
        assert_eq!(find_nvim_in_tree(&info), None);
    }

    // ---- pick_side_pane resolver ----
    //
    // The inspect closure is faked so we can exercise every branch without
    // touching tmux or sysinfo. Each test sets up a window pane list, an
    // optional stored id, and a closure that returns the desired Option<bool>.

    fn make_pane_ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_pick_stored_valid() {
        // Stored pane is in the window and not the caller — use it directly.
        let panes = make_pane_ids(&["%1", "%2", "%3"]);
        let result = pick_side_pane("%1", &panes, "%2", |p| {
            assert_eq!(p, "%2");
            Some(true)
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::Pane("%2".to_string()),
                nvim_running: Some(true),
            }
        );
    }

    #[test]
    fn test_pick_stored_not_in_window_falls_through() {
        // Stored id refers to a pane no longer in this window — fall through
        // to step 2 (single other pane).
        let panes = make_pane_ids(&["%1", "%2"]);
        let result = pick_side_pane("%1", &panes, "%99", |p| {
            assert_eq!(p, "%2");
            Some(false)
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::Pane("%2".to_string()),
                nvim_running: Some(false),
            }
        );
    }

    #[test]
    fn test_pick_stored_equals_caller_falls_through() {
        // Stored id equals caller — must not pick the caller's own pane.
        let panes = make_pane_ids(&["%1", "%2"]);
        let result = pick_side_pane("%1", &panes, "%1", |p| {
            assert_eq!(p, "%2");
            Some(true)
        });
        assert_eq!(result.pane, ResolvedSidePane::Pane("%2".to_string()));
    }

    #[test]
    fn test_pick_single_other_pane() {
        let panes = make_pane_ids(&["%1", "%2"]);
        let result = pick_side_pane("%1", &panes, "", |p| {
            assert_eq!(p, "%2");
            Some(true)
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::Pane("%2".to_string()),
                nvim_running: Some(true),
            }
        );
    }

    #[test]
    fn test_pick_no_other_panes_is_definite_false() {
        // Window has only the caller pane. We did "look" — there is nothing
        // to inspect, therefore there is definitely no nvim in a side pane.
        let panes = make_pane_ids(&["%1"]);
        let result = pick_side_pane("%1", &panes, "", |_| {
            panic!("inspect must not be called when there are no other panes")
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::None,
                nvim_running: Some(false),
            }
        );
    }

    #[test]
    fn test_pick_multi_no_vim() {
        // 3 other panes, none have vim — definite false (we walked all).
        let panes = make_pane_ids(&["%1", "%2", "%3", "%4"]);
        let result = pick_side_pane("%1", &panes, "", |_| Some(false));
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::None,
                nvim_running: Some(false),
            }
        );
    }

    #[test]
    fn test_pick_multi_unique_vim() {
        // 3 other panes, exactly one has vim, all others inspectable —
        // confidently pick that one.
        let panes = make_pane_ids(&["%1", "%2", "%3", "%4"]);
        let result = pick_side_pane("%1", &panes, "", |p| match p {
            "%3" => Some(true),
            _ => Some(false),
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::Pane("%3".to_string()),
                nvim_running: Some(true),
            }
        );
    }

    #[test]
    fn test_pick_multi_multiple_vim_is_ambiguous() {
        let panes = make_pane_ids(&["%1", "%2", "%3", "%4"]);
        let result = pick_side_pane("%1", &panes, "", |_| Some(true));
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::Ambiguous,
                nvim_running: Some(true),
            }
        );
    }

    #[test]
    fn test_pick_multi_all_uninspectable_is_unknown() {
        // Walked every pane but every inspection failed — answer is unknown,
        // not false.
        let panes = make_pane_ids(&["%1", "%2", "%3"]);
        let result = pick_side_pane("%1", &panes, "", |_| None);
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::None,
                nvim_running: None,
            }
        );
    }

    #[test]
    fn test_pick_multi_some_uninspectable_no_vim() {
        // Mixed: some panes uninspectable, others returned Some(false). No
        // confirmed vim anywhere — must report unknown, since one of the
        // uninspectable panes might have had vim.
        let panes = make_pane_ids(&["%1", "%2", "%3", "%4"]);
        let result = pick_side_pane("%1", &panes, "", |p| match p {
            "%2" => Some(false),
            "%3" => None,
            "%4" => Some(false),
            _ => panic!("unexpected pane {p}"),
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::None,
                nvim_running: None,
            }
        );
    }

    #[test]
    fn test_pick_multi_one_vim_with_uninspectable_is_ambiguous() {
        // The Fix #2 case: one pane confirmed vim, but another pane could not
        // be inspected. The uninspectable one MIGHT also have vim. Refuse to
        // route to a possibly-wrong pane — promote to Ambiguous.
        let panes = make_pane_ids(&["%1", "%2", "%3", "%4"]);
        let result = pick_side_pane("%1", &panes, "", |p| match p {
            "%2" => Some(true),
            "%3" => None,
            "%4" => Some(false),
            _ => panic!("unexpected pane {p}"),
        });
        assert_eq!(
            result,
            ResolvedStatus {
                pane: ResolvedSidePane::Ambiguous,
                nvim_running: Some(true),
            }
        );
    }

    // ---- format_pane_status wire format ----

    #[test]
    fn test_format_pane_status_unknown_prints_unknown_not_false() {
        // Regression guard: the whole point of Option<bool> is that None must
        // print as `unknown`, never as `false`.
        let s = SidePaneStatus {
            pane_id: "%5".to_string(),
            nvim_running: None,
            file: None,
        };
        let out = format_pane_status(&s);
        assert!(out.contains("nvim: unknown"), "got: {out}");
        assert!(!out.contains("nvim: false"), "got: {out}");
    }

    #[test]
    fn test_format_pane_status_true_with_file() {
        let s = SidePaneStatus {
            pane_id: "%5".to_string(),
            nvim_running: Some(true),
            file: Some("/tmp/foo.rs".to_string()),
        };
        assert_eq!(
            format_pane_status(&s),
            "pane_id: %5\nnvim: true\nfile: /tmp/foo.rs"
        );
    }

    #[test]
    fn test_format_pane_status_false_no_file() {
        let s = SidePaneStatus {
            pane_id: "none".to_string(),
            nvim_running: Some(false),
            file: None,
        };
        assert_eq!(format_pane_status(&s), "pane_id: none\nnvim: false\nfile: ");
    }

    #[test]
    fn test_format_pane_status_ambiguous() {
        let s = SidePaneStatus {
            pane_id: "ambiguous".to_string(),
            nvim_running: Some(true),
            file: None,
        };
        assert_eq!(
            format_pane_status(&s),
            "pane_id: ambiguous\nnvim: true\nfile: "
        );
    }

    // ---- parent-pid-tree ----

    #[test]
    fn test_parse_pane_pids_basic() {
        let input = "%35 2594534\n%65 331460\n";
        let map = parse_pane_pids(input);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&2594534).map(String::as_str), Some("%35"));
        assert_eq!(map.get(&331460).map(String::as_str), Some("%65"));
    }

    #[test]
    fn test_parse_pane_pids_handles_blank_and_malformed_lines() {
        let input = "\n%1 100\n   \ngarbage\n%2 notapid\n%3 300\n";
        let map = parse_pane_pids(input);
        // Only well-formed lines survive; malformed ones are skipped, not errored.
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&100).map(String::as_str), Some("%1"));
        assert_eq!(map.get(&300).map(String::as_str), Some("%3"));
    }

    /// Build a fake ppid reader from (child -> parent) pairs.
    fn fake_ppid(chain: &[(u32, u32)]) -> impl FnMut(u32) -> Option<u32> + '_ {
        move |pid: u32| {
            chain
                .iter()
                .find_map(|(c, p)| if *c == pid { Some(*p) } else { None })
        }
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_larry_scenario() {
        // Larry scenario: caller pid is 400284. Its ancestor chain is
        // 400284 -> 398200 -> 2594534. The pane_pid map has %35 -> 2594534
        // and %65 -> 331460. We expect %35.
        let mut pane_pids = HashMap::new();
        pane_pids.insert(2594534, "%35".to_string());
        pane_pids.insert(331460, "%65".to_string());
        let chain = [(400284u32, 398200u32), (398200, 2594534)];
        let result = resolve_pane_by_parent_chain(400284, &pane_pids, fake_ppid(&chain))
            .expect("expected a match");
        assert_eq!(result.pane_id, "%35");
        assert_eq!(result.pane_pid, 2594534);
        assert_eq!(result.ancestors_walked, vec![400284, 398200, 2594534]);
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_start_pid_already_matches() {
        // Edge case: start_pid itself is a pane_pid.
        let mut pane_pids = HashMap::new();
        pane_pids.insert(2594534, "%35".to_string());
        let chain: [(u32, u32); 0] = [];
        let result = resolve_pane_by_parent_chain(2594534, &pane_pids, fake_ppid(&chain))
            .expect("expected a match");
        assert_eq!(result.pane_id, "%35");
        assert_eq!(result.pane_pid, 2594534);
        assert_eq!(result.ancestors_walked, vec![2594534]);
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_no_match() {
        // Walker whose chain never hits any pane_pid — walk terminates at init.
        let mut pane_pids = HashMap::new();
        pane_pids.insert(9999, "%99".to_string());
        let chain = [(500u32, 400u32), (400, 300), (300, 1)];
        let result = resolve_pane_by_parent_chain(500, &pane_pids, fake_ppid(&chain));
        assert!(result.is_none(), "expected None, got {:?}", result);
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_vanished_parent_graceful() {
        // Walker whose parent disappears mid-walk (read_ppid returns None).
        // Should return None gracefully, not panic.
        let pane_pids = HashMap::new();
        let chain = [(500u32, 400u32)]; // 400 has no entry, so read_ppid(400) = None
        let result = resolve_pane_by_parent_chain(500, &pane_pids, fake_ppid(&chain));
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_vanished_parent_but_prior_match() {
        // Walker's first ancestor matches, so the match wins even though the
        // next read would fail. Verifies "first match wins" semantics.
        let mut pane_pids = HashMap::new();
        pane_pids.insert(400, "%7".to_string());
        let chain = [(500u32, 400u32)]; // read_ppid(400) would be None, but we match first
        let result = resolve_pane_by_parent_chain(500, &pane_pids, fake_ppid(&chain))
            .expect("expected a match");
        assert_eq!(result.pane_id, "%7");
        assert_eq!(result.pane_pid, 400);
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_depth_cap() {
        // Ensure a degenerate chain doesn't loop forever — capped at 64.
        let pane_pids = HashMap::new();
        // Every pid's parent is pid+1000, so we never hit 1. Walker must stop.
        let reader = |pid: u32| Some(pid + 1000);
        let result = resolve_pane_by_parent_chain(100, &pane_pids, reader);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_cycle_safe() {
        // Pathological cycle: 500 -> 400 -> 500. Walker must not infinite-loop.
        let pane_pids = HashMap::new();
        let chain = [(500u32, 400u32), (400, 500)];
        let result = resolve_pane_by_parent_chain(500, &pane_pids, fake_ppid(&chain));
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_start_from_pid_zero() {
        // Sentinel guard: starting at pid 0 must return None immediately without
        // consulting the reader. Protects against bogus input blowing up the walk.
        let pane_pids = HashMap::new();
        let mut calls = 0u32;
        let reader = |_pid: u32| {
            calls += 1;
            Some(42u32)
        };
        let result = resolve_pane_by_parent_chain(0, &pane_pids, reader);
        assert!(result.is_none());
        assert_eq!(calls, 0, "reader must not be called when start_pid is 0");
    }

    #[test]
    fn test_resolve_pane_by_parent_chain_start_from_pid_one() {
        // Same sentinel guard for init (pid 1). Walker must stop, not query reader.
        let pane_pids = HashMap::new();
        let mut calls = 0u32;
        let reader = |_pid: u32| {
            calls += 1;
            Some(42u32)
        };
        let result = resolve_pane_by_parent_chain(1, &pane_pids, reader);
        assert!(result.is_none());
        assert_eq!(calls, 0, "reader must not be called when start_pid is 1");
    }

    #[test]
    fn test_read_ppid_from_proc_pid_zero_guard() {
        // pid 0 is a sentinel — must return None without touching /proc.
        assert_eq!(read_ppid_from_proc(0), None);
    }

    #[test]
    fn test_read_ppid_from_proc_nonexistent_pid() {
        // A pid that (almost) certainly does not exist on any real system must
        // return None gracefully (fs::read fails → .ok()? short-circuits). Any
        // panic here would indicate the error path is broken.
        assert_eq!(read_ppid_from_proc(u32::MAX), None);
    }

    // --- Integration-ish tests against real /proc -------------------------------
    //
    // These exercise the live /proc stat parser (rfind(')')-based comm handling)
    // against real kernel data so the `comm` field with spaces or close-parens
    // doesn't silently break parsing. They're Linux-only; guarded with cfg.

    #[cfg(target_os = "linux")]
    #[test]
    fn test_read_ppid_from_proc_init_is_zero() {
        // init (pid 1) always exists on Linux and its ppid is 0 by kernel
        // convention. Verifies the parser works end-to-end on a real stat line.
        let ppid = read_ppid_from_proc(1).expect("/proc/1/stat must be readable on Linux");
        assert_eq!(ppid, 0, "init's ppid should be 0");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_read_ppid_from_proc_self_matches_process_parent() {
        // The current process's ppid from /proc must be Some(non-zero). This
        // catches regressions in field-offset math after the comm section — the
        // very bug the rfind(')') logic exists to prevent.
        let my_pid = std::process::id();
        let ppid =
            read_ppid_from_proc(my_pid).expect("/proc/<self>/stat must be readable and parseable");
        assert!(
            ppid > 0,
            "own ppid must be > 0 (actual pid tree should have a real parent)"
        );
    }

    // ---- CLI-level tests for parent-pid-tree via DI ---------------------------
    //
    // These drive `run_parent_pid_tree` — the humble-object testable core — with
    // in-memory mocks for `TmuxProvider` and `ProcReader`, so every exit code
    // (0/1/2/3) and flag combination (--json/--verbose/--pid) is reachable
    // without touching tmux or /proc. The previous test-coverage pass had to
    // reject these because the command function wrote to stdout via println!
    // and called tmux directly; Phase 1 (trait extraction) put the seam in
    // place.

    /// In-memory `TmuxProvider` mock. Constructed with a pre-built result —
    /// either a list of (pane_id, pane_pid) pairs, or an explicit `TmuxError`
    /// for testing the exit-2 paths.
    enum MockTmuxResult {
        Ok(Vec<(String, u32)>),
        NotRunning,
        ListFailed,
    }

    struct MockTmuxProvider {
        result: MockTmuxResult,
    }

    impl MockTmuxProvider {
        fn with_panes(pairs: &[(&str, u32)]) -> Self {
            let pairs = pairs
                .iter()
                .map(|(id, pid)| ((*id).to_string(), *pid))
                .collect();
            Self {
                result: MockTmuxResult::Ok(pairs),
            }
        }

        fn not_running() -> Self {
            Self {
                result: MockTmuxResult::NotRunning,
            }
        }

        fn list_failed() -> Self {
            Self {
                result: MockTmuxResult::ListFailed,
            }
        }
    }

    impl TmuxProvider for MockTmuxProvider {
        fn list_pane_pids(&self) -> Result<Vec<(String, u32)>, TmuxError> {
            match &self.result {
                MockTmuxResult::Ok(pairs) => Ok(pairs.clone()),
                MockTmuxResult::NotRunning => Err(TmuxError::NotRunning),
                MockTmuxResult::ListFailed => Err(TmuxError::ListFailed(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no such binary",
                ))),
            }
        }

        fn active_pane(&self) -> Result<Option<String>, TmuxError> {
            Ok(None)
        }

        fn capture_pane(&self, _pane_id: &str, _window: usize) -> Result<String, TmuxError> {
            Ok(String::new())
        }
    }

    /// In-memory `ProcReader` mock. Takes a list of `(child, parent)` pairs
    /// plus an optional set of pids that "fail" — these return `None` from
    /// `read_ppid` even if they have an entry in the chain. A pid with no
    /// chain entry and no failure marker falls off (returns None), matching
    /// real-world "process vanished" semantics.
    struct MockProcReader {
        chain: Vec<(u32, u32)>,
        fail_on: HashSet<u32>,
        cmdlines: HashMap<u32, String>,
        comms: HashMap<u32, String>,
        exes: HashMap<u32, PathBuf>,
    }

    impl MockProcReader {
        fn from_chain(chain: &[(u32, u32)]) -> Self {
            Self {
                chain: chain.to_vec(),
                fail_on: HashSet::new(),
                cmdlines: HashMap::new(),
                comms: HashMap::new(),
                exes: HashMap::new(),
            }
        }

        fn failing_on(mut self, pid: u32) -> Self {
            self.fail_on.insert(pid);
            self
        }

        fn with_cmdline(mut self, pid: u32, cmdline: &str) -> Self {
            self.cmdlines.insert(pid, cmdline.to_string());
            self
        }

        fn with_comm(mut self, pid: u32, comm: &str) -> Self {
            self.comms.insert(pid, comm.to_string());
            self
        }

        fn with_exe(mut self, pid: u32, exe: &str) -> Self {
            self.exes.insert(pid, PathBuf::from(exe));
            self
        }
    }

    impl ProcReader for MockProcReader {
        fn read_ppid(&self, pid: u32) -> Option<u32> {
            if self.fail_on.contains(&pid) {
                return None;
            }
            self.chain
                .iter()
                .find_map(|(c, p)| if *c == pid { Some(*p) } else { None })
        }

        fn read_cmdline(&self, pid: u32) -> Option<String> {
            self.cmdlines.get(&pid).cloned()
        }

        fn read_comm(&self, pid: u32) -> Option<String> {
            self.comms.get(&pid).cloned()
        }

        fn read_exe(&self, pid: u32) -> Option<PathBuf> {
            self.exes.get(&pid).cloned()
        }
    }

    fn args(json: bool, pid: Option<u32>, verbose: bool) -> ParentPidTreeArgs {
        ParentPidTreeArgs {
            json,
            pid,
            verbose,
            tree: false,
        }
    }

    fn tree_args(json: bool, pid: Option<u32>, verbose: bool) -> ParentPidTreeArgs {
        ParentPidTreeArgs {
            json,
            pid,
            verbose,
            tree: true,
        }
    }

    #[test]
    fn test_run_parent_pid_tree_default_json_output() {
        // Larry scenario shaped for the CLI: self_pid=999, its ppid is 2594534
        // (the pane_pid of %35). --json should emit the documented payload.
        let tmux = MockTmuxProvider::with_panes(&[("%35", 2594534), ("%65", 331460)]);
        let proc = MockProcReader::from_chain(&[(999, 2594534)]);
        let outcome = run_parent_pid_tree(args(true, None, false), 999, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr_lines, Vec::<String>::new());
        // Parse the emitted JSON to assert on each field.
        let payload: serde_json::Value =
            serde_json::from_str(&outcome.stdout).expect("stdout must be valid JSON");
        assert_eq!(payload["pane_id"], "%35");
        assert_eq!(payload["pane_pid"], 2594534);
        assert_eq!(payload["walked_from_pid"], 2594534);
        assert_eq!(payload["ancestors_walked"], serde_json::json!([2594534]));
    }

    #[test]
    fn test_run_parent_pid_tree_explicit_pid_override_walks_from_that_pid() {
        // --pid 999 must start the walk from 999, not from self_pid/ppid-of-self.
        // The self_pid we pass is deliberately unrelated — it must not be
        // consulted because args.pid is Some.
        let tmux = MockTmuxProvider::with_panes(&[("%42", 12345)]);
        // Chain: 999 -> 12345, which matches %42.
        let proc = MockProcReader::from_chain(&[(999, 12345)]);
        let outcome = run_parent_pid_tree(args(true, Some(999), false), 9_999_999, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let payload: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
        assert_eq!(payload["pane_id"], "%42");
        assert_eq!(payload["walked_from_pid"], 999);
        assert_eq!(payload["ancestors_walked"], serde_json::json!([999, 12345]));
    }

    #[test]
    fn test_run_parent_pid_tree_explicit_pid_does_not_read_self_ppid() {
        // If self_pid's ppid reader would fail, we'd get exit 3 via the
        // default path. Passing --pid must skip that read entirely. We assert
        // by setting self_pid to something that would fail AND passing an
        // explicit pid — result must be exit 0, not 3.
        let tmux = MockTmuxProvider::with_panes(&[("%7", 400)]);
        // Reader has a chain for 100 -> 400 but fails on pid 42. If the code
        // read_ppid(42) we'd take the "None" branch → exit 3.
        let proc = MockProcReader::from_chain(&[(100, 400)]).failing_on(42);
        let outcome = run_parent_pid_tree(args(false, Some(100), false), 42, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "%7");
    }

    #[test]
    fn test_run_parent_pid_tree_plain_output_is_just_pane_id() {
        // Default (no --json) stdout must be just the pane id — callers rely on
        // `pane=$(rmux_helper parent-pid-tree)` returning something assignable.
        let tmux = MockTmuxProvider::with_panes(&[("%7", 400)]);
        let proc = MockProcReader::from_chain(&[(500, 400)]);
        let outcome = run_parent_pid_tree(args(false, Some(500), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "%7");
    }

    #[test]
    fn test_run_parent_pid_tree_verbose_emits_walk_chain_to_stderr() {
        // --verbose must log both the starting pid and the walk chain with the
        // documented arrow format.
        let tmux = MockTmuxProvider::with_panes(&[("%35", 2594534)]);
        let proc = MockProcReader::from_chain(&[(999, 398200), (398200, 2594534)]);
        let outcome = run_parent_pid_tree(args(false, Some(999), true), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "%35");
        // First verbose line is the start announcement, second is the chain.
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("starting walk at pid 999")),
            "stderr missing start line: {:?}",
            outcome.stderr_lines
        );
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("999 -> 398200 -> 2594534") && l.contains("pane %35")),
            "stderr missing walk chain: {:?}",
            outcome.stderr_lines
        );
    }

    #[test]
    fn test_run_parent_pid_tree_exit_1_no_match() {
        // Walker walks from 999 -> 1 (init) and never hits a pane_pid. Exit 1,
        // empty stdout, human-readable stderr.
        let tmux = MockTmuxProvider::with_panes(&[("%99", 9999)]);
        let proc = MockProcReader::from_chain(&[(999, 500), (500, 1)]);
        let outcome = run_parent_pid_tree(args(false, Some(999), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, "");
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("no tmux pane found for pid 999")),
            "stderr missing no-match message: {:?}",
            outcome.stderr_lines
        );
    }

    #[test]
    fn test_run_parent_pid_tree_exit_2_not_running() {
        // TmuxProvider returns NotRunning → exit 2 with the canonical stderr
        // message. Must not print anything to stdout.
        let tmux = MockTmuxProvider::not_running();
        let proc = MockProcReader::from_chain(&[]);
        let outcome = run_parent_pid_tree(args(false, Some(1), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 2);
        assert_eq!(outcome.stdout, "");
        assert_eq!(
            outcome.stderr_lines,
            vec!["tmux not running or no panes".to_string()]
        );
    }

    #[test]
    fn test_run_parent_pid_tree_exit_2_list_failed_has_different_stderr() {
        // ListFailed is a distinct flavor of exit 2 — the stderr carries the
        // underlying io::Error rather than the plain "not running" message.
        let tmux = MockTmuxProvider::list_failed();
        let proc = MockProcReader::from_chain(&[]);
        let outcome = run_parent_pid_tree(args(false, Some(1), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 2);
        assert_eq!(outcome.stdout, "");
        let joined = outcome.stderr_lines.join("\n");
        assert!(
            joined.contains("tmux list-panes failed"),
            "stderr missing list-failed detail: {}",
            joined
        );
    }

    #[test]
    fn test_run_parent_pid_tree_exit_3_cannot_read_self_ppid() {
        // No --pid passed and ProcReader.read_ppid(self_pid) returns None → exit 3.
        let tmux = MockTmuxProvider::with_panes(&[("%1", 100)]);
        let proc = MockProcReader::from_chain(&[]); // any read returns None
        let outcome = run_parent_pid_tree(args(false, None, false), 42, &tmux, &proc);
        assert_eq!(outcome.exit_code, 3);
        assert_eq!(outcome.stdout, "");
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("could not read /proc/42/stat")),
            "stderr missing exit-3 message: {:?}",
            outcome.stderr_lines
        );
    }

    #[test]
    fn test_run_parent_pid_tree_json_and_pid_flags_combine() {
        // --json + --pid should emit JSON from the explicit pid's walk.
        let tmux = MockTmuxProvider::with_panes(&[("%8", 800)]);
        let proc = MockProcReader::from_chain(&[(700, 800)]);
        let outcome = run_parent_pid_tree(args(true, Some(700), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let payload: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
        assert_eq!(payload["pane_id"], "%8");
        assert_eq!(payload["walked_from_pid"], 700);
    }

    #[test]
    fn test_run_parent_pid_tree_json_and_verbose_both_emit() {
        // --json + --verbose: stdout gets the JSON payload, stderr still gets
        // the walk chain. The two streams must not pollute each other.
        let tmux = MockTmuxProvider::with_panes(&[("%9", 900)]);
        let proc = MockProcReader::from_chain(&[(800, 900)]);
        let outcome = run_parent_pid_tree(args(true, Some(800), true), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        // stdout is JSON.
        let payload: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
        assert_eq!(payload["pane_id"], "%9");
        // stderr has the walk chain — JSON must not leak into stderr lines.
        assert!(outcome
            .stderr_lines
            .iter()
            .any(|l| l.contains("800 -> 900") && l.contains("pane %9")));
        for line in &outcome.stderr_lines {
            assert!(
                !line.contains("\"pane_id\""),
                "stderr leaked JSON payload: {}",
                line
            );
        }
    }

    // ---- --tree flag tests -----------------------------------------------
    //
    // These exercise the humble-object core with populated cmdline/comm/exe
    // data and verify both the TreeEntry collection and the pure formatters.

    #[test]
    fn test_run_parent_pid_tree_tree_mode_collects_chain_metadata() {
        // Three-pid chain: 999 -> 4000 -> 2500 (pane_pid of %35). Each pid
        // has distinct cmdline/comm/exe, so we can verify the entries come
        // back in walk order with the per-pid metadata attached.
        let tmux = MockTmuxProvider::with_panes(&[("%35", 2500)]);
        let proc = MockProcReader::from_chain(&[(999, 4000), (4000, 2500)])
            .with_comm(999, "bash")
            .with_cmdline(999, "/usr/bin/bash -c rmux_helper parent-pid-tree --tree")
            .with_exe(999, "/usr/bin/bash")
            .with_comm(4000, "claude")
            .with_cmdline(4000, "claude /startup-larry")
            .with_exe(4000, "/home/developer/.local/bin/claude")
            .with_comm(2500, "zsh")
            .with_cmdline(2500, "/home/linuxbrew/.linuxbrew/bin/zsh")
            .with_exe(2500, "/home/linuxbrew/.linuxbrew/bin/zsh");
        let outcome = run_parent_pid_tree(tree_args(false, Some(999), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let tree = outcome.tree.as_ref().expect("tree should be populated");
        assert_eq!(tree.len(), 3, "expected 3 entries, got {}", tree.len());
        assert_eq!(tree[0].pid, 999);
        assert_eq!(tree[0].comm.as_deref(), Some("bash"));
        assert_eq!(
            tree[0].cmdline.as_deref(),
            Some("/usr/bin/bash -c rmux_helper parent-pid-tree --tree")
        );
        assert_eq!(
            tree[0].exe.as_deref(),
            Some(std::path::Path::new("/usr/bin/bash"))
        );
        assert_eq!(tree[1].pid, 4000);
        assert_eq!(tree[1].comm.as_deref(), Some("claude"));
        assert_eq!(tree[2].pid, 2500);
        assert_eq!(tree[2].comm.as_deref(), Some("zsh"));
        assert_eq!(outcome.tree_pane_id.as_deref(), Some("%35"));
        assert_eq!(outcome.tree_pane_pid, Some(2500));
        assert_eq!(outcome.tree_start_pid, Some(999));
    }

    #[test]
    fn test_run_parent_pid_tree_tree_mode_handles_missing_cmdline() {
        // A kernel-thread-like entry in the chain: comm populated, cmdline
        // empty. The TreeEntry must capture comm = Some("kthreadd") and
        // cmdline = None. The formatter will render this as [kthreadd].
        let tmux = MockTmuxProvider::with_panes(&[("%1", 100)]);
        let proc = MockProcReader::from_chain(&[(50, 100)])
            .with_comm(50, "kthreadd")
            // Intentionally NO cmdline for pid 50.
            .with_exe(50, "/usr/sbin/init")
            .with_comm(100, "zsh")
            .with_cmdline(100, "/bin/zsh")
            .with_exe(100, "/bin/zsh");
        let outcome = run_parent_pid_tree(tree_args(false, Some(50), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let tree = outcome.tree.as_ref().unwrap();
        assert_eq!(tree[0].pid, 50);
        assert_eq!(tree[0].comm.as_deref(), Some("kthreadd"));
        assert_eq!(tree[0].cmdline, None);
        // Text formatter should fall back to [kthreadd].
        assert!(
            outcome.stdout.contains("[kthreadd]"),
            "stdout missing kthreadd fallback: {}",
            outcome.stdout
        );
    }

    #[test]
    fn test_run_parent_pid_tree_tree_mode_handles_missing_exe() {
        // Middle pid has no exe readlink (permission denied / race). We must
        // not panic — TreeEntry.exe stays None and the text formatter prints
        // "exe: (unreadable)".
        let tmux = MockTmuxProvider::with_panes(&[("%2", 200)]);
        let proc = MockProcReader::from_chain(&[(100, 150), (150, 200)])
            .with_comm(100, "bash")
            .with_cmdline(100, "bash -l")
            .with_exe(100, "/bin/bash")
            .with_comm(150, "mystery")
            .with_cmdline(150, "/opt/mystery --flag")
            // Intentionally NO exe for pid 150.
            .with_comm(200, "zsh")
            .with_cmdline(200, "/bin/zsh")
            .with_exe(200, "/bin/zsh");
        let outcome = run_parent_pid_tree(tree_args(false, Some(100), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let tree = outcome.tree.as_ref().unwrap();
        assert_eq!(tree[1].pid, 150);
        assert_eq!(tree[1].exe, None);
        assert!(
            outcome.stdout.contains("exe: (unreadable)"),
            "stdout missing (unreadable) marker: {}",
            outcome.stdout
        );
    }

    #[test]
    fn test_run_parent_pid_tree_tree_and_json_combined() {
        // --tree + --json: stdout is structured JSON with start_pid, pane_id,
        // pane_pid, and a chain array where each entry has pid/comm/cmdline/exe.
        let tmux = MockTmuxProvider::with_panes(&[("%42", 420)]);
        let proc = MockProcReader::from_chain(&[(100, 420)])
            .with_comm(100, "bash")
            .with_cmdline(100, "/bin/bash -c foo")
            .with_exe(100, "/bin/bash")
            .with_comm(420, "zsh")
            .with_cmdline(420, "/bin/zsh")
            .with_exe(420, "/bin/zsh");
        let outcome = run_parent_pid_tree(tree_args(true, Some(100), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let payload: serde_json::Value =
            serde_json::from_str(&outcome.stdout).expect("stdout must be valid JSON");
        assert_eq!(payload["start_pid"], 100);
        assert_eq!(payload["pane_id"], "%42");
        assert_eq!(payload["pane_pid"], 420);
        let chain = payload["chain"].as_array().expect("chain must be an array");
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0]["pid"], 100);
        assert_eq!(chain[0]["comm"], "bash");
        assert_eq!(chain[0]["cmdline"], "/bin/bash -c foo");
        assert_eq!(chain[0]["exe"], "/bin/bash");
        assert_eq!(chain[1]["pid"], 420);
        assert_eq!(chain[1]["exe"], "/bin/zsh");
    }

    #[test]
    fn test_run_parent_pid_tree_tree_on_no_match() {
        // Walker never hits a pane_pid (walks from 999 -> 500 -> 1). Exit 1,
        // but tree data is still populated so the user can see the chain they
        // walked (useful when debugging why resolution failed).
        let tmux = MockTmuxProvider::with_panes(&[("%99", 9999)]);
        let proc = MockProcReader::from_chain(&[(999, 500), (500, 1)])
            .with_comm(999, "bash")
            .with_cmdline(999, "bash")
            .with_comm(500, "sshd")
            .with_cmdline(500, "sshd");
        let outcome = run_parent_pid_tree(tree_args(false, Some(999), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 1);
        let tree = outcome
            .tree
            .as_ref()
            .expect("tree populated even on no-match");
        // Chain includes start_pid plus whatever ancestors the walker reached
        // before the walker gave up at init (pid 1).
        let pids: Vec<u32> = tree.iter().map(|e| e.pid).collect();
        assert!(pids.contains(&999));
        assert!(pids.contains(&500));
        assert_eq!(outcome.tree_pane_id, None);
        assert_eq!(outcome.tree_pane_pid, None);
    }

    #[test]
    fn test_tree_formatter_truncates_long_cmdline() {
        // Cmdline >120 chars should be truncated with a `…` suffix in the
        // text view. The full value is still available via --json.
        let long_arg = "a".repeat(300);
        let entry = TreeEntry {
            pid: 42,
            comm: Some("bash".to_string()),
            cmdline: Some(format!("/bin/bash -c '{}'", long_arg)),
            exe: Some(PathBuf::from("/bin/bash")),
        };
        let out = format_tree_text(&[entry], None);
        assert!(out.contains("…"), "missing ellipsis: {}", out);
        // No line in the tree body should exceed ~200 chars (a generous
        // upper bound allowing for prefix + truncated cmdline).
        for line in out.lines() {
            assert!(
                line.chars().count() < 200,
                "line too long ({} chars): {}",
                line.chars().count(),
                line,
            );
        }
    }

    #[test]
    fn test_tree_formatter_ascii_box_drawing() {
        // Two-entry tree: first row uses `├─`, last row uses `└─`. The
        // continuation prefix on the non-leaf entry's exe line uses `│`.
        let entries = vec![
            TreeEntry {
                pid: 1,
                comm: Some("a".to_string()),
                cmdline: Some("a".to_string()),
                exe: Some(PathBuf::from("/a")),
            },
            TreeEntry {
                pid: 2,
                comm: Some("b".to_string()),
                cmdline: Some("b".to_string()),
                exe: Some(PathBuf::from("/b")),
            },
        ];
        let out = format_tree_text(&entries, None);
        assert!(out.contains("├─"), "missing ├─ branch marker: {}", out);
        assert!(out.contains("└─"), "missing └─ leaf marker: {}", out);
        assert!(out.contains("│"), "missing │ continuation marker: {}", out);
        // Ordering sanity: the ├─ line appears before the └─ line.
        let branch_idx = out.find("├─").unwrap();
        let leaf_idx = out.find("└─").unwrap();
        assert!(branch_idx < leaf_idx);
    }

    #[test]
    fn test_tree_formatter_shows_pane_id_at_leaf() {
        // When pane_id is Some, the final entry gets a "(pane shell)"
        // annotation AND a "tmux pane: %35" line. Use a 2-entry chain so the
        // leaf's annotation is the plain "(pane shell)" — a single-entry
        // chain collapses into "(start, pane shell)" which is covered
        // separately in test_format_tree_text_single_entry_chain.
        let entries = vec![
            TreeEntry {
                pid: 2,
                comm: Some("bash".to_string()),
                cmdline: Some("/bin/bash".to_string()),
                exe: Some(PathBuf::from("/bin/bash")),
            },
            TreeEntry {
                pid: 1,
                comm: Some("zsh".to_string()),
                cmdline: Some("/bin/zsh".to_string()),
                exe: Some(PathBuf::from("/bin/zsh")),
            },
        ];
        let out = format_tree_text(&entries, Some("%35"));
        assert!(
            out.contains("(pane shell)"),
            "missing (pane shell): {}",
            out
        );
        assert!(
            out.contains("tmux pane: %35"),
            "missing tmux pane line: {}",
            out
        );
    }

    #[test]
    fn test_tree_formatter_omits_pane_id_when_none() {
        // When pane_id is None (no-match case), neither annotation appears.
        let entries = vec![TreeEntry {
            pid: 1,
            comm: Some("zsh".to_string()),
            cmdline: Some("/bin/zsh".to_string()),
            exe: Some(PathBuf::from("/bin/zsh")),
        }];
        let out = format_tree_text(&entries, None);
        assert!(!out.contains("(pane shell)"));
        assert!(!out.contains("tmux pane:"));
    }

    #[test]
    fn test_real_proc_reader_cmdline_for_self() {
        // Real-proc read for our own pid: cmdline should be non-empty and
        // include something binary-ish. Loosely asserted — this is just a
        // smoke test to make sure the RealProcReader wiring works.
        let reader = RealProcReader;
        let pid = std::process::id();
        let cmdline = reader.read_cmdline(pid);
        assert!(cmdline.is_some(), "own cmdline must be readable");
        let comm = reader.read_comm(pid);
        assert!(comm.is_some(), "own comm must be readable");
        let exe = reader.read_exe(pid);
        assert!(exe.is_some(), "own exe must be readable");
    }

    // ---- install-completions ----

    #[test]
    fn test_detect_shell_from_env_basenames() {
        assert_eq!(
            detect_shell_from_env(Some("/bin/zsh")),
            Some(CompletionShell::Zsh)
        );
        assert_eq!(
            detect_shell_from_env(Some("/usr/bin/bash")),
            Some(CompletionShell::Bash)
        );
        assert_eq!(
            detect_shell_from_env(Some("fish")),
            Some(CompletionShell::Fish)
        );
        assert_eq!(
            detect_shell_from_env(Some("/opt/homebrew/bin/pwsh")),
            Some(CompletionShell::Powershell)
        );
        assert_eq!(
            detect_shell_from_env(Some("elvish")),
            Some(CompletionShell::Elvish)
        );
    }

    #[test]
    fn test_detect_shell_from_env_rejects_unknown_and_empty() {
        assert_eq!(detect_shell_from_env(Some("/weird/custom")), None);
        assert_eq!(detect_shell_from_env(Some("")), None);
        assert_eq!(detect_shell_from_env(None), None);
    }

    #[test]
    fn test_completion_install_path_zsh_uses_zdotdir_when_set() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            zdotdir: Some("/home/user/dotfiles/zsh".into()),
            ..Default::default()
        };
        let p = completion_install_path(CompletionShell::Zsh, &env).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/home/user/dotfiles/zsh/.zfunc/_rmux_helper")
        );
    }

    #[test]
    fn test_completion_install_path_zsh_falls_back_to_home() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            ..Default::default()
        };
        let p = completion_install_path(CompletionShell::Zsh, &env).unwrap();
        assert_eq!(p, PathBuf::from("/home/user/.zfunc/_rmux_helper"));
    }

    #[test]
    fn test_completion_install_path_bash_respects_xdg_data_home() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            xdg_data_home: Some("/custom/data".into()),
            ..Default::default()
        };
        let p = completion_install_path(CompletionShell::Bash, &env).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/custom/data/bash-completion/completions/rmux_helper")
        );
    }

    #[test]
    fn test_completion_install_path_bash_default_xdg() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            ..Default::default()
        };
        let p = completion_install_path(CompletionShell::Bash, &env).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/home/user/.local/share/bash-completion/completions/rmux_helper")
        );
    }

    #[test]
    fn test_completion_install_path_fish_respects_xdg_config_home() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            xdg_config_home: Some("/custom/cfg".into()),
            ..Default::default()
        };
        let p = completion_install_path(CompletionShell::Fish, &env).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/custom/cfg/fish/completions/rmux_helper.fish")
        );
    }

    #[test]
    fn test_completion_install_path_fish_default() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            ..Default::default()
        };
        let p = completion_install_path(CompletionShell::Fish, &env).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/home/user/.config/fish/completions/rmux_helper.fish")
        );
    }

    #[test]
    fn test_completion_install_path_powershell_none() {
        let env = EnvSnapshot {
            home: Some("/home/user".into()),
            ..Default::default()
        };
        assert_eq!(
            completion_install_path(CompletionShell::Powershell, &env),
            None
        );
        assert_eq!(completion_install_path(CompletionShell::Elvish, &env), None);
    }

    #[test]
    fn test_completion_install_path_no_home_no_xdg_returns_none() {
        // $HOME unset with no XDG override -> can't resolve zsh/bash/fish path.
        let env = EnvSnapshot::default();
        assert_eq!(completion_install_path(CompletionShell::Zsh, &env), None);
        assert_eq!(completion_install_path(CompletionShell::Bash, &env), None);
        assert_eq!(completion_install_path(CompletionShell::Fish, &env), None);
    }

    #[test]
    fn test_enumerate_pid_candidates_sorted_desc_and_truncated() {
        // Build a tempdir `/proc`-like layout with numeric-named subdirs.
        let tmp =
            std::env::temp_dir().join(format!("rmux_helper_pid_enum_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        for pid in [1u32, 5, 42, 100, 500, 9999] {
            fs::create_dir(tmp.join(pid.to_string())).unwrap();
        }
        // Also create a non-numeric dir — should be ignored.
        fs::create_dir(tmp.join("self")).unwrap();

        // Mock reader that returns a fixed comm for known pids.
        let proc = MockProcReader::from_chain(&[])
            .with_comm(42, "claude")
            .with_comm(100, "zsh");

        let out = enumerate_pid_candidates(&proc, &tmp, 3);
        // Descending order, capped at 3.
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].0, 9999);
        assert_eq!(out[1].0, 500);
        assert_eq!(out[2].0, 100);
        assert_eq!(out[2].1.as_deref(), Some("zsh"));

        // Uncapped path: all six pids, comms attached where known.
        let full = enumerate_pid_candidates(&proc, &tmp, 100);
        assert_eq!(full.len(), 6);
        let pids: Vec<u32> = full.iter().map(|(p, _)| *p).collect();
        assert_eq!(pids, vec![9999, 500, 100, 42, 5, 1]);
        assert_eq!(full[3].1.as_deref(), Some("claude"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_enumerate_pid_candidates_missing_proc_returns_empty() {
        // /proc-equivalent path doesn't exist — return empty, never panic.
        let proc = MockProcReader::from_chain(&[]);
        let missing = PathBuf::from("/nonexistent/proc/path/xyz_rmux_test");
        let out = enumerate_pid_candidates(&proc, &missing, 100);
        assert!(out.is_empty());
    }

    #[test]
    fn test_install_completions_cli_parses_conflicting_flags() {
        // --print-only and --dry-run are mutually exclusive; clap should reject.
        use clap::Parser;
        let result = Cli::try_parse_from([
            "rmux_helper",
            "install-completions",
            "--shell",
            "zsh",
            "--print-only",
            "--dry-run",
        ]);
        assert!(result.is_err(), "expected conflicts_with to reject this");
    }

    #[test]
    fn test_install_completions_cli_rejects_unknown_shell() {
        use clap::Parser;
        let result = Cli::try_parse_from([
            "rmux_helper",
            "install-completions",
            "--shell",
            "nosuchshell",
        ]);
        assert!(result.is_err(), "unknown shell should be rejected by clap");
    }

    // ---- --verbose + --json rich chain output ---------------------------
    //
    // Exercises the rule that either --tree OR --verbose triggers the full
    // chain[] array in JSON, while --json alone keeps the minimal payload.
    // Plain text mode with --verbose stays on the single-line pane-id output.

    #[test]
    fn test_run_parent_pid_tree_verbose_and_json_includes_rich_chain() {
        // --verbose --json (no --tree): JSON stdout gets the full chain[]
        // array AND stderr keeps its walk lines for human inspection.
        let tmux = MockTmuxProvider::with_panes(&[("%35", 2500)]);
        let proc = MockProcReader::from_chain(&[(999, 4000), (4000, 2500)])
            .with_comm(999, "bash")
            .with_cmdline(
                999,
                "/bin/bash -c rmux_helper parent-pid-tree --verbose --json",
            )
            .with_exe(999, "/usr/bin/bash")
            .with_comm(4000, "claude")
            .with_cmdline(4000, "claude /startup-larry")
            .with_exe(4000, "/home/developer/.local/bin/claude")
            .with_comm(2500, "zsh")
            .with_cmdline(2500, "/bin/zsh")
            .with_exe(2500, "/bin/zsh");
        let mut a = args(true, Some(999), true);
        a.tree = false;
        let outcome = run_parent_pid_tree(a, 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        // stdout is valid JSON with chain[].
        let payload: serde_json::Value =
            serde_json::from_str(&outcome.stdout).expect("stdout must be valid JSON");
        let chain = payload["chain"]
            .as_array()
            .expect("chain[] must be present on --verbose --json");
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0]["pid"], 999);
        assert_eq!(chain[0]["comm"], "bash");
        assert_eq!(chain[0]["role"], "start");
        assert_eq!(chain[2]["pid"], 2500);
        assert_eq!(chain[2]["role"], "pane_shell");
        assert_eq!(payload["pane_id"], "%35");
        assert_eq!(payload["pane_pid"], 2500);
        assert_eq!(payload["start_pid"], 999);
        // --verbose stderr walk lines still present — verbose's signature
        // human-inspection stream is preserved even with rich JSON stdout.
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("starting walk at pid 999")),
            "stderr missing verbose start line: {:?}",
            outcome.stderr_lines
        );
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("999 -> 4000 -> 2500")),
            "stderr missing walk chain: {:?}",
            outcome.stderr_lines
        );
    }

    #[test]
    fn test_run_parent_pid_tree_json_alone_stays_minimal() {
        // --json alone (no --verbose, no --tree): minimal payload, NO
        // chain[] field — preserves the scriptable contract.
        let tmux = MockTmuxProvider::with_panes(&[("%35", 2500)]);
        let proc = MockProcReader::from_chain(&[(999, 2500)])
            .with_comm(999, "bash")
            .with_cmdline(999, "/bin/bash")
            .with_exe(999, "/usr/bin/bash")
            .with_comm(2500, "zsh");
        let outcome = run_parent_pid_tree(args(true, Some(999), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let payload: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
        assert!(
            payload.get("chain").is_none(),
            "minimal --json must NOT include chain[]: {}",
            outcome.stdout
        );
        // Historical keys still present.
        assert_eq!(payload["pane_id"], "%35");
        assert_eq!(payload["pane_pid"], 2500);
        assert_eq!(payload["walked_from_pid"], 999);
        assert_eq!(payload["ancestors_walked"], serde_json::json!([999, 2500]));
    }

    #[test]
    fn test_run_parent_pid_tree_verbose_text_mode_unchanged() {
        // --verbose in plain text mode (no --json, no --tree) must still
        // emit just the pane id on stdout. The stderr walk lines are the
        // only visible effect of --verbose in text mode.
        let tmux = MockTmuxProvider::with_panes(&[("%7", 400)]);
        let proc = MockProcReader::from_chain(&[(500, 400)]);
        let outcome = run_parent_pid_tree(args(false, Some(500), true), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "%7");
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("starting walk at pid 500")),
            "stderr missing verbose start line: {:?}",
            outcome.stderr_lines
        );
    }

    #[test]
    fn test_format_tree_text_marks_root_with_start() {
        // 3-entry chain — first entry's line must include "(start)".
        let entries = vec![
            TreeEntry {
                pid: 10,
                comm: Some("bash".to_string()),
                cmdline: Some("bash".to_string()),
                exe: Some(PathBuf::from("/bin/bash")),
            },
            TreeEntry {
                pid: 20,
                comm: Some("claude".to_string()),
                cmdline: Some("claude".to_string()),
                exe: Some(PathBuf::from("/bin/claude")),
            },
            TreeEntry {
                pid: 30,
                comm: Some("zsh".to_string()),
                cmdline: Some("/bin/zsh".to_string()),
                exe: Some(PathBuf::from("/bin/zsh")),
            },
        ];
        let out = format_tree_text(&entries, Some("%35"));
        // Find the line for pid 10 and assert it has (start).
        let start_line = out
            .lines()
            .find(|l| l.contains("[pid 10 "))
            .unwrap_or_else(|| panic!("no line for pid 10 in:\n{}", out));
        assert!(
            start_line.contains("(start)"),
            "start line missing (start): {}",
            start_line,
        );
        // Intermediate entry (pid 20) must NOT be annotated.
        let mid_line = out
            .lines()
            .find(|l| l.contains("[pid 20 "))
            .unwrap_or_else(|| panic!("no line for pid 20 in:\n{}", out));
        assert!(
            !mid_line.contains("(start)") && !mid_line.contains("(pane shell)"),
            "intermediate line should be unannotated: {}",
            mid_line,
        );
    }

    #[test]
    fn test_format_tree_text_single_entry_chain() {
        // Single-entry chain where start IS the pane shell: the one
        // annotation combines both roles into "(start, pane shell)".
        let entries = vec![TreeEntry {
            pid: 7,
            comm: Some("zsh".to_string()),
            cmdline: Some("/bin/zsh".to_string()),
            exe: Some(PathBuf::from("/bin/zsh")),
        }];
        let out = format_tree_text(&entries, Some("%9"));
        assert!(
            out.contains("(start, pane shell)"),
            "single-entry chain missing combined annotation: {}",
            out,
        );
    }

    #[test]
    fn test_format_tree_text_no_match_marks_leaf() {
        // 2-entry chain with pane_id=None: last line has "(no pane found)",
        // first line has "(start)".
        let entries = vec![
            TreeEntry {
                pid: 1,
                comm: Some("bash".to_string()),
                cmdline: Some("bash".to_string()),
                exe: Some(PathBuf::from("/bin/bash")),
            },
            TreeEntry {
                pid: 2,
                comm: Some("init".to_string()),
                cmdline: Some("init".to_string()),
                exe: Some(PathBuf::from("/sbin/init")),
            },
        ];
        let out = format_tree_text(&entries, None);
        assert!(out.contains("(start)"), "missing (start): {}", out);
        assert!(
            out.contains("(no pane found)"),
            "missing (no pane found): {}",
            out,
        );
        // Single-entry variant collapses into "(start, no pane found)".
        let solo = vec![TreeEntry {
            pid: 1,
            comm: Some("bash".to_string()),
            cmdline: Some("bash".to_string()),
            exe: Some(PathBuf::from("/bin/bash")),
        }];
        let solo_out = format_tree_text(&solo, None);
        assert!(
            solo_out.contains("(start, no pane found)"),
            "single-entry no-match missing combined annotation: {}",
            solo_out,
        );
    }

    #[test]
    fn test_format_tree_json_roles() {
        // 4-entry chain where last pid matches pane_pid: roles are
        // start / ancestor / ancestor / pane_shell.
        let entries = vec![
            TreeEntry {
                pid: 10,
                comm: Some("bash".to_string()),
                cmdline: Some("bash".to_string()),
                exe: Some(PathBuf::from("/bin/bash")),
            },
            TreeEntry {
                pid: 20,
                comm: Some("claude".to_string()),
                cmdline: Some("claude".to_string()),
                exe: Some(PathBuf::from("/bin/claude")),
            },
            TreeEntry {
                pid: 30,
                comm: Some("larry".to_string()),
                cmdline: Some("larry".to_string()),
                exe: Some(PathBuf::from("/bin/larry")),
            },
            TreeEntry {
                pid: 40,
                comm: Some("zsh".to_string()),
                cmdline: Some("/bin/zsh".to_string()),
                exe: Some(PathBuf::from("/bin/zsh")),
            },
        ];
        let json = format_tree_json(10, Some("%35"), Some(40), &entries);
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        let chain = payload["chain"].as_array().unwrap();
        assert_eq!(chain[0]["role"], "start");
        assert_eq!(chain[1]["role"], "ancestor");
        assert_eq!(chain[2]["role"], "ancestor");
        assert_eq!(chain[3]["role"], "pane_shell");
    }

    #[test]
    fn test_format_tree_json_no_match_case() {
        // pane_id/pane_pid both None: last entry gets "walked_past_root".
        let entries = vec![
            TreeEntry {
                pid: 10,
                comm: Some("bash".to_string()),
                cmdline: Some("bash".to_string()),
                exe: Some(PathBuf::from("/bin/bash")),
            },
            TreeEntry {
                pid: 1,
                comm: Some("init".to_string()),
                cmdline: Some("init".to_string()),
                exe: Some(PathBuf::from("/sbin/init")),
            },
        ];
        let json = format_tree_json(10, None, None, &entries);
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(payload["pane_id"].is_null());
        assert!(payload["pane_pid"].is_null());
        let chain = payload["chain"].as_array().unwrap();
        assert_eq!(chain[0]["role"], "start");
        assert_eq!(chain[1]["role"], "walked_past_root");
    }

    #[test]
    fn test_format_tree_json_single_entry_start_is_pane() {
        // 1-entry chain where start pid IS the pane_pid.
        let entries = vec![TreeEntry {
            pid: 42,
            comm: Some("zsh".to_string()),
            cmdline: Some("/bin/zsh".to_string()),
            exe: Some(PathBuf::from("/bin/zsh")),
        }];
        let json = format_tree_json(42, Some("%9"), Some(42), &entries);
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        let chain = payload["chain"].as_array().unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0]["role"], "start_and_pane_shell");
    }

    #[test]
    fn test_format_tree_json_single_entry_no_match() {
        // 1-entry no-match: role collapses into "walked_past_root".
        let entries = vec![TreeEntry {
            pid: 42,
            comm: Some("init".to_string()),
            cmdline: Some("init".to_string()),
            exe: Some(PathBuf::from("/sbin/init")),
        }];
        let json = format_tree_json(42, None, None, &entries);
        let payload: serde_json::Value = serde_json::from_str(&json).unwrap();
        let chain = payload["chain"].as_array().unwrap();
        assert_eq!(chain[0]["role"], "walked_past_root");
    }

    #[test]
    fn test_chain_entry_role_pure_helper() {
        // Direct coverage of the pure role-assignment policy.
        // Multi-entry, leaf matches pane_pid.
        assert_eq!(chain_entry_role(0, 3, 10, Some(30)), "start");
        assert_eq!(chain_entry_role(1, 3, 20, Some(30)), "ancestor");
        assert_eq!(chain_entry_role(2, 3, 30, Some(30)), "pane_shell");
        // Single-entry, start is pane.
        assert_eq!(chain_entry_role(0, 1, 5, Some(5)), "start_and_pane_shell");
        // Single-entry, no match.
        assert_eq!(chain_entry_role(0, 1, 5, None), "walked_past_root");
        // Multi-entry, no match: first is start, middle is ancestor, last is
        // walked_past_root.
        assert_eq!(chain_entry_role(0, 3, 10, None), "start");
        assert_eq!(chain_entry_role(1, 3, 20, None), "ancestor");
        assert_eq!(chain_entry_role(2, 3, 30, None), "walked_past_root");
    }

    #[test]
    fn test_run_parent_pid_tree_tree_and_json_adds_roles() {
        // --tree --json end-to-end: chain[0].role == "start",
        // chain[-1].role == "pane_shell".
        let tmux = MockTmuxProvider::with_panes(&[("%42", 420)]);
        let proc = MockProcReader::from_chain(&[(100, 200), (200, 420)])
            .with_comm(100, "bash")
            .with_cmdline(100, "/bin/bash")
            .with_exe(100, "/bin/bash")
            .with_comm(200, "claude")
            .with_cmdline(200, "claude /startup")
            .with_exe(200, "/bin/claude")
            .with_comm(420, "zsh")
            .with_cmdline(420, "/bin/zsh")
            .with_exe(420, "/bin/zsh");
        let outcome = run_parent_pid_tree(tree_args(true, Some(100), false), 1, &tmux, &proc);
        assert_eq!(outcome.exit_code, 0);
        let payload: serde_json::Value = serde_json::from_str(&outcome.stdout).unwrap();
        let chain = payload["chain"].as_array().unwrap();
        assert_eq!(chain[0]["role"], "start");
        assert_eq!(chain[2]["role"], "pane_shell");
    }

    #[test]
    fn test_capture_pane_args_shape() {
        let args = capture_pane_args("%12", 75);
        assert_eq!(
            args,
            vec![
                "capture-pane".to_string(),
                "-p".to_string(),
                "-J".to_string(),
                "-S".to_string(),
                "-75".to_string(),
                "-E".to_string(),
                "-".to_string(),
                "-t".to_string(),
                "%12".to_string(),
            ]
        );
    }
}
