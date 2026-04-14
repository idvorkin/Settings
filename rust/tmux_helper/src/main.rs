mod picker;
mod link_picker;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use sysinfo::{Pid, ProcessRefreshKind, System};

pub const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_HASH"),
    ")"
);

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
        #[arg(long)]
        pid: Option<u32>,
        /// Log the walk (ancestor chain, pane match) to stderr for debugging
        #[arg(long)]
        verbose: bool,
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
    let normalized_current = if current_title.eq_ignore_ascii_case(&hostname)
        || current_title.is_empty()
    {
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
    let path_mappings: HashMap<&str, &str> =
        [("idvorkin.github.io", "blog"), ("idvorkin", "me")]
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
    let last: String = chars[chars.len()-2..].iter().collect();
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

            generate_title_with_context(&process_info, &ctx)
                .unwrap_or_else(|| generate_title_from_tmux(&pane.pane_current_command, &short_path))
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
                .args(["resize-pane", "-t", &panes[0], "-x", &target_width.to_string()])
                .output();
            set_tmux_option(THIRD_STATE_OPTION, STATE_THIRD_HORIZONTAL);
        } else {
            let target_height = (window_height as f32 * 0.33) as i32;
            let _ = Command::new("tmux")
                .args(["resize-pane", "-t", &panes[0], "-y", &target_height.to_string()])
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
    let pid_str = run_tmux_command(&["display-message", "-t", pane_id, "-p", "#{pane_pid}"])
        .ok()?;
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
    let stored_valid = !stored.is_empty()
        && window_panes.iter().any(|p| p == stored)
        && stored != caller_pane_id;
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
        "show-option", "-wqv", "-t", caller_pane_id, SIDE_EDIT_PANE_OPTION,
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
        "show-option", "-wqv", "-t", caller_pane_id, SIDE_EDIT_PANE_OPTION,
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
            let new_id = create_side_pane_shell(caller_pane_id)
                .context("Failed to create side pane.")?;
            let _ = Command::new("tmux")
                .args(["set-option", "-w", "-t", caller_pane_id, SIDE_EDIT_PANE_OPTION, &new_id])
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
                .args(["set-option", "-w", "-t", caller_pane_id, SIDE_EDIT_PANE_OPTION, &adopted])
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
                    .args(["resize-pane", "-t", caller_pane_id, "-x", &target.to_string()])
                    .output();
            }
        }
    }

    // Poll up to 500ms for the new pane's shell to be ready
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        let pid_str = run_tmux_command(&["display-message", "-t", &new_pane_id, "-p", "#{pane_pid}"])
            .unwrap_or_default();
        if pid_str.trim().parse::<u32>().map(|p| p > 0).unwrap_or(false) {
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
enum TmuxError {
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
trait TmuxProvider {
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
}

/// Humble shell over `/proc/<pid>/stat`. Tests inject a mock that returns a
/// pre-built chain without touching the filesystem.
trait ProcReader {
    /// Return the parent pid of `pid` (field 4 of `/proc/<pid>/stat`). Returns
    /// `None` for pid 0, a vanished process, or an unreadable/unparseable stat
    /// file. `None` is non-fatal to the walker — it just means "stop here".
    fn read_ppid(&self, pid: u32) -> Option<u32>;
}

/// Production implementation of `TmuxProvider` — shells out to the `tmux`
/// binary via `std::process::Command`.
struct RealTmuxProvider;

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
        if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
    }
}

/// Production implementation of `ProcReader`. Delegates to
/// `read_ppid_from_proc`, whose `rfind(')')`-based parser is load-bearing for
/// `comm` fields containing parens or spaces — do NOT reimplement it inline.
struct RealProcReader;

impl ProcReader for RealProcReader {
    fn read_ppid(&self, pid: u32) -> Option<u32> {
        read_ppid_from_proc(pid)
    }
}

/// Result of a successful parent-pid walk.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneMatch {
    pane_id: String,
    pane_pid: u32,
    ancestors_walked: Vec<u32>,
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
fn resolve_pane_by_parent_chain<F>(
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
            };
        }
        Err(e) => {
            stderr_lines.push(format!("tmux not running or no panes: {}", e));
            return ParentPidTreeOutcome {
                stdout: String::new(),
                stderr_lines,
                exit_code: 2,
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
                };
            }
        },
    };

    if args.verbose {
        stderr_lines.push(format!("parent-pid-tree: starting walk at pid {}", start_pid));
    }

    // 3. Walk the chain. The walker takes its own read_ppid closure, which we
    //    adapt from the injected `ProcReader`.
    let result = resolve_pane_by_parent_chain(start_pid, &pane_pids, |p| proc.read_ppid(p));

    match result {
        Some(m) => {
            if args.verbose {
                let chain: Vec<String> = m
                    .ancestors_walked
                    .iter()
                    .map(|p| p.to_string())
                    .collect();
                stderr_lines.push(format!(
                    "parent-pid-tree: walked {} (pane_pid) -> pane {}",
                    chain.join(" -> "),
                    m.pane_id
                ));
            }
            let stdout = if args.json {
                let payload = serde_json::json!({
                    "pane_id": m.pane_id,
                    "pane_pid": m.pane_pid,
                    "walked_from_pid": start_pid,
                    "ancestors_walked": m.ancestors_walked,
                });
                payload.to_string()
            } else {
                m.pane_id.clone()
            };
            ParentPidTreeOutcome {
                stdout,
                stderr_lines,
                exit_code: 0,
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
            ParentPidTreeOutcome {
                stdout: String::new(),
                stderr_lines,
                exit_code: 1,
            }
        }
    }
}

/// Thin wrapper that wires the real humble-shell impls into the testable core
/// and performs the actual stdout/stderr writes. `main()` calls this; tests
/// call `run_parent_pid_tree` directly with mocks.
fn parent_pid_tree_cmd(json: bool, pid: Option<u32>, verbose: bool) -> i32 {
    let args = ParentPidTreeArgs { json, pid, verbose };
    let tmux = RealTmuxProvider;
    let proc = RealProcReader;
    let outcome = run_parent_pid_tree(args, std::process::id(), &tmux, &proc);
    for line in &outcome.stderr_lines {
        eprintln!("{}", line);
    }
    if !outcome.stdout.is_empty() {
        println!("{}", outcome.stdout);
    }
    outcome.exit_code
}

fn main() -> Result<()> {
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
        Some(Commands::PickLinks { json, enrich_deadline_ms }) => {
            link_picker::pick_links(json, enrich_deadline_ms)
        }
        Some(Commands::ParentPidTree { json, pid, verbose }) => {
            let code = parent_pid_tree_cmd(json, pid, verbose);
            std::process::exit(code);
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
        assert_eq!(generate_title(&info, "myproject"), Some("vi myproject".to_string()));
    }

    #[test]
    fn test_generate_title_plain_shell() {
        let info = make_process_info("zsh", "/bin/zsh", vec![]);
        assert_eq!(generate_title(&info, "myproject"), Some("z myproject".to_string()));
    }

    #[test]
    fn test_generate_title_docker() {
        let child = make_process_info("docker", "docker run nginx", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(generate_title(&info, "myproject"), Some("docker myproject".to_string()));
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
        assert_eq!(generate_title(&info, "blog"), Some("j dev blog".to_string()));
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
        assert_eq!(generate_title(&info, "blog"), Some("jekyll blog".to_string()));
    }

    #[test]
    fn test_generate_title_just_jekyll_serve() {
        // "just jekyll-serve" should shorten to "j jekyll <path>"
        let child = make_process_info("just", "just jekyll-serve", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(generate_title(&info, "blog"), Some("j jekyll blog".to_string()));
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
        assert_eq!(
            shorten_path_middle("repo/subdir/leaf", 12),
            "repo/…/leaf"
        );
    }

    #[test]
    fn test_shorten_path_middle_home_path() {
        assert_eq!(shorten_path_middle("~/deep/path/leaf", 20), "~/deep/path/leaf");
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
        assert_eq!(
            parse_file_line("foo.py:0"),
            ("foo.py:0".to_string(), None)
        );
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
        let parent = make_process_info_full(
            10,
            "zsh",
            "/bin/zsh -c 'edit vimrc'",
            None,
            vec![],
        );
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
        assert_eq!(
            format_pane_status(&s),
            "pane_id: none\nnvim: false\nfile: "
        );
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
        move |pid: u32| chain.iter().find_map(|(c, p)| if *c == pid { Some(*p) } else { None })
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
                MockTmuxResult::ListFailed => Err(TmuxError::ListFailed(
                    std::io::Error::new(std::io::ErrorKind::NotFound, "no such binary"),
                )),
            }
        }

        fn active_pane(&self) -> Result<Option<String>, TmuxError> {
            Ok(None)
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
    }

    impl MockProcReader {
        fn from_chain(chain: &[(u32, u32)]) -> Self {
            Self {
                chain: chain.to_vec(),
                fail_on: HashSet::new(),
            }
        }

        fn failing_on(mut self, pid: u32) -> Self {
            self.fail_on.insert(pid);
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
    }

    fn args(json: bool, pid: Option<u32>, verbose: bool) -> ParentPidTreeArgs {
        ParentPidTreeArgs { json, pid, verbose }
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
        let outcome =
            run_parent_pid_tree(args(true, Some(999), false), 9_999_999, &tmux, &proc);
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
            outcome.stderr_lines.iter().any(|l| l.contains("starting walk at pid 999")),
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
        assert!(
            outcome
                .stderr_lines
                .iter()
                .any(|l| l.contains("800 -> 900") && l.contains("pane %9"))
        );
        for line in &outcome.stderr_lines {
            assert!(
                !line.contains("\"pane_id\""),
                "stderr leaked JSON payload: {}",
                line
            );
        }
    }
}
