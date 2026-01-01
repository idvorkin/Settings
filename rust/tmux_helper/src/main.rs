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
    name: String,
    cmdline: String,
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
        name: process.name().to_string_lossy().to_string(),
        cmdline: process
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        cwd: process
            .cwd()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        children,
    })
}

fn process_tree_has_pattern(info: &ProcessInfo, patterns: &[&str]) -> bool {
    let cmdline_lower = info.cmdline.to_lowercase();
    if patterns.iter().any(|p| cmdline_lower.contains(p)) {
        return true;
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
/// When running Claude and there's room, includes shortened pane title:
/// - "claude <pane_title> <path>" if width allows
/// - Falls back to "claude <path>" if not enough room
///
/// Title format rules (in priority order):
/// 1. Known dev tools with path: "ai <path>", "claude [pane] <path>", "vi <path>", "docker <path>"
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
        return Some(generate_claude_title(ctx));
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

/// Generate a Claude title with dynamic width and optional pane name
///
/// Format: "path:pane_name" - compact and informative
/// - Path is compacted to 2..2 format if longer than 6 chars
/// - Pane title has ✳ prefix stripped
fn generate_claude_title(ctx: &TitleContext) -> String {
    let compact = compact_path(ctx.short_path);

    // If no pane title, just return path
    let pane_title = match ctx.pane_title {
        Some(t) if !t.is_empty() => clean_pane_title(t),
        _ => return compact,
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
        // Just use compacted path (no pane title in fallback)
        compact_path(short_path)
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

fn get_current_panes() -> Vec<String> {
    run_tmux_command(&["list-panes", "-F", "#{pane_id}"])
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}

fn ensure_two_panes(command: Option<&str>) -> (Vec<String>, bool) {
    let panes = get_current_panes();
    if panes.len() == 1 {
        let mut args = vec!["split-window", "-h", "-c", "#{pane_current_path}"];
        if let Some(cmd) = command.filter(|c| !c.is_empty()) {
            args.push(cmd);
        }
        let _ = Command::new("tmux").args(&args).output();
        let _ = Command::new("tmux").args(["select-layout", "even-horizontal"]).output();
        return (get_current_panes(), true);
    }
    (panes, false)
}

fn get_layout_orientation() -> Option<String> {
    let pane_info = run_tmux_command(&["list-panes", "-F", "#{pane_left},#{pane_top}"]).ok()?;
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
    let (panes, created_new) = ensure_two_panes(None);
    if panes.is_empty() {
        return Ok(());
    }

    if created_new {
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL);
        return Ok(());
    }

    let current_state = get_tmux_option(LAYOUT_STATE_OPTION);

    if current_state == STATE_HORIZONTAL {
        let _ = Command::new("tmux")
            .args(["select-layout", "even-vertical"])
            .output();
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_VERTICAL);
    } else {
        let _ = Command::new("tmux")
            .args(["select-layout", "even-horizontal"])
            .output();
        set_tmux_option(LAYOUT_STATE_OPTION, STATE_HORIZONTAL);
    }

    Ok(())
}

fn third(command: &str) -> Result<()> {
    let cmd_opt = if command.is_empty() {
        None
    } else {
        Some(command)
    };
    let (panes, created_new) = ensure_two_panes(cmd_opt);
    if panes.len() != 2 {
        return Ok(());
    }

    let orientation = match get_layout_orientation() {
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
        // Get window dimensions
        let window_info =
            run_tmux_command(&["display-message", "-p", "#{window_width},#{window_height}"])?;
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
        ProcessInfo {
            name: name.to_string(),
            cmdline: cmdline.to_string(),
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
        // No prefix, just compacted path
        // "myproject" (9 chars) -> "my..ct" (6 chars)
        assert_eq!(generate_title(&info, "myproject"), Some("my..ct".to_string()));
        // Short paths stay as-is
        assert_eq!(generate_title(&info, "blog"), Some("blog".to_string()));
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
        assert_eq!(generate_claude_title(&ctx), "blog:fix-auth");
    }

    #[test]
    fn test_generate_claude_title_strips_star_prefix() {
        // Should strip ✳ prefix from pane titles
        let ctx = TitleContext {
            short_path: "blog",
            pane_title: Some("✳ PR Review Workflow"),
            window_width: 40,
        };
        assert_eq!(generate_claude_title(&ctx), "blog:PR Review Workflow");
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
        assert_eq!(generate_claude_title(&ctx), "ma..or:Screen Effects");
    }

    #[test]
    fn test_generate_claude_title_no_pane_title() {
        // Without pane title, just path
        let ctx = TitleContext {
            short_path: "blog",
            pane_title: None,
            window_width: 40,
        };
        assert_eq!(generate_claude_title(&ctx), "blog");
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
        assert_eq!(generate_claude_title(&ctx), "blog:fix-auth");
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
}
