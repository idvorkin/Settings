mod picker;

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

/// True if a binary basename is vim/nvim or a recognizable variant
/// (`nvim.appimage`, `nvim-qt`, `vim.basic`, ...).
fn basename_is_vim(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "vim"
        || lower == "nvim"
        || lower.starts_with("nvim.")
        || lower.starts_with("nvim-")
        || lower.starts_with("vim.")
        || lower.starts_with("vim-")
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
    /// or `"ambiguous"` if multiple candidates are running vim/nvim.
    pane_id: String,
    /// `Some(true|false)` after a real inspection; `None` if we couldn't inspect
    /// (e.g., pid query failed). Callers should treat `None` as "unknown",
    /// not "false".
    nvim_running: Option<bool>,
    file: Option<String>,
}

/// Get the current side pane status.
///
/// Resolution priority:
/// 1. Stored `@side_edit_pane_id` window option, if still in this window
/// 2. The single "other" pane in this window
/// 3. Walk every other pane in the window and pick the unique one running vim/nvim
///
/// Importantly, when there's no obvious side pane this still **looks** at every
/// candidate pane and reports `Some(false)` if none have vim — only failing
/// inspections produce `None`/"unknown".
fn get_side_pane_status(caller_pane_id: &str) -> SidePaneStatus {
    // Query the window-local option scoped to the caller's window (not tmux's "current" window)
    let stored = run_tmux_command(&[
        "show-option", "-wqv", "-t", caller_pane_id, SIDE_EDIT_PANE_OPTION,
    ])
    .unwrap_or_default();
    let window_panes = get_panes_in_window(caller_pane_id);
    let others: Vec<String> = window_panes
        .iter()
        .filter(|p| p.as_str() != caller_pane_id)
        .cloned()
        .collect();

    let mut sys = System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let stored_valid = !stored.is_empty()
        && window_panes.contains(&stored)
        && stored != caller_pane_id;

    // Helper: build status for a single resolved pane.
    let build_for_pane = |pane: String, sys: &System| -> SidePaneStatus {
        let nvim_running = inspect_pane_for_vim(&pane, sys);
        let file = if nvim_running == Some(true) {
            find_nvim_pid_in_pane(&pane, sys).and_then(get_nvim_current_file)
        } else {
            None
        };
        SidePaneStatus {
            pane_id: pane,
            nvim_running,
            file,
        }
    };

    if stored_valid {
        return build_for_pane(stored, &sys);
    }

    if others.len() == 1 {
        return build_for_pane(others[0].clone(), &sys);
    }

    if others.is_empty() {
        // No other panes in this window — definitively no side pane,
        // and therefore definitively no nvim in a side pane. We did "look".
        return SidePaneStatus {
            pane_id: "none".to_string(),
            nvim_running: Some(false),
            file: None,
        };
    }

    // Multiple "other" panes and no stored id — actually look at each one
    // instead of giving up at the resolution step.
    let mut vim_panes: Vec<String> = Vec::new();
    let mut any_uninspectable = false;
    for pane in &others {
        match inspect_pane_for_vim(pane, &sys) {
            Some(true) => vim_panes.push(pane.clone()),
            Some(false) => {}
            None => any_uninspectable = true,
        }
    }

    match vim_panes.len() {
        1 => {
            let pane = vim_panes.remove(0);
            let file = find_nvim_pid_in_pane(&pane, &sys).and_then(get_nvim_current_file);
            SidePaneStatus {
                pane_id: pane,
                nvim_running: Some(true),
                file,
            }
        }
        0 => SidePaneStatus {
            pane_id: "none".to_string(),
            // We inspected every pane we could; only call it "unknown"
            // if at least one inspection actually failed.
            nvim_running: if any_uninspectable { None } else { Some(false) },
            file: None,
        },
        _ => SidePaneStatus {
            // Multiple panes have vim — we don't know which is "the" side pane,
            // but we definitely *did* find vim running.
            pane_id: "ambiguous".to_string(),
            nvim_running: Some(true),
            file: None,
        },
    }
}

/// Print pane status to stdout.
fn print_pane_status(status: &SidePaneStatus) {
    println!("pane_id: {}", status.pane_id);
    let nvim_str = match status.nvim_running {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    };
    println!("nvim: {}", nvim_str);
    println!("file: {}", status.file.as_deref().unwrap_or(""));
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

    if is_vim_in_pane(&side_pane_id, &system) {
        if !force {
            eprintln!(
                "nvim is running in the side pane ({}). You may lose unsaved work.\n\
                 Use --force to kill it and run your command.",
                side_pane_id
            );
            // Still print status so caller gets pane info
            let status = get_side_pane_status(&caller_pane_id);
            print_pane_status(&status);
            anyhow::bail!("nvim is running in side pane; use --force to override");
        }
        // Force: send :qa! to nvim
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
}
