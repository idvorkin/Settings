use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
// nucleo available for future fuzzy matching enhancements
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::collections::{HashMap, HashSet};
use std::io;
use std::process::Command;
use sysinfo::{Pid, ProcessRefreshKind, System};

const VERSION: &str = concat!(
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
}

// Layout state constants
const LAYOUT_STATE_OPTION: &str = "@layout_state";
const THIRD_STATE_OPTION: &str = "@third_state";
const STATE_HORIZONTAL: &str = "horizontal";
const STATE_VERTICAL: &str = "vertical";
const STATE_THIRD_HORIZONTAL: &str = "third_horizontal";
const STATE_THIRD_VERTICAL: &str = "third_vertical";
const STATE_NORMAL: &str = "normal";

#[derive(Debug)]
struct PaneInfo {
    pane_id: String,
    window_id: String,
    window_name: String,
    pane_pid: u32,
    pane_current_command: String,
    pane_current_path: String,
}

#[derive(Debug)]
struct ProcessInfo {
    name: String,
    cmdline: String,
    cwd: String,
    children: Vec<ProcessInfo>,
}

fn run_tmux_command(args: &[&str]) -> Result<String> {
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
        "#{pane_id}\t#{window_id}\t#{window_name}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}",
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

fn get_git_repo_name(cwd: &str, cache: &mut HashMap<String, Option<String>>) -> Option<String> {
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

fn get_short_path(cwd: &str, git_repo: Option<&str>) -> String {
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

/// Generate a window title from process info.
///
/// Title format rules (in priority order):
/// 1. Known dev tools with path: "ai <path>", "cl <path>", "vi <path>", "docker <path>"
/// 2. Plain shell (no children): "z <path>"
/// 3. Shell with known child commands: handled by get_child_command_title()
///    - just: "j <subcommand> <path>" (e.g., "j dev blog")
///    - jekyll: "jekyll <path>"
/// 4. Shell with unknown children: returns None (caller uses tmux fallback)
/// 5. Other processes: just the process name
///
/// Returns None when we can't determine a good title (caller should use tmux fallback)
fn generate_title(info: &ProcessInfo, short_path: &str) -> Option<String> {
    // Priority 1: Known dev tools - check entire process tree for patterns
    if process_tree_has_pattern(info, &["aider"]) {
        return Some(format!("ai {}", short_path));
    }
    if process_tree_has_pattern(info, &["@anthropic-ai/claude-code", "claude"]) {
        return Some(format!("cl {}", short_path));
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
fn generate_title_from_tmux(command: &str, short_path: &str) -> String {
    let cmd_lower = command.to_lowercase();
    if cmd_lower.contains("aider") {
        format!("ai {}", short_path)
    } else if cmd_lower.contains("claude") {
        format!("cl {}", short_path)
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

    for pane in panes {
        // Skip if already renamed this window
        if renamed_windows.contains(&pane.window_id) {
            continue;
        }
        renamed_windows.insert(pane.window_id.clone());

        // Try to get process info from system, with tmux fallback
        let title = if let Some(process_info) = get_process_info(&system, pane.pane_pid) {
            let cwd = &process_info.cwd;
            let git_repo = get_git_repo_name(cwd, &mut git_cache);
            let short_path = get_short_path(cwd, git_repo.as_deref());
            generate_title(&process_info, &short_path)
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
        // Create split with optional command
        if let Some(cmd) = command {
            if !cmd.is_empty() {
                let _ = Command::new("tmux")
                    .args(["split-window", "-h", "-c", "#{pane_current_path}", cmd])
                    .output();
            } else {
                let _ = Command::new("tmux")
                    .args(["split-window", "-h", "-c", "#{pane_current_path}"])
                    .output();
            }
        } else {
            let _ = Command::new("tmux")
                .args(["split-window", "-h", "-c", "#{pane_current_path}"])
                .output();
        }
        let _ = Command::new("tmux")
            .args(["select-layout", "even-horizontal"])
            .output();
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

// ============================================================================
// Ratatui-based TUI Picker
// ============================================================================

/// Entry in the picker list
#[derive(Clone)]
struct PickerEntry {
    target: String,      // e.g., "session:1.2" for switching
    display: String,     // Display text (without ANSI, for matching)
    is_session: bool,    // Is this a session header?
    is_separator: bool,  // Is this a separator line?
    is_current: bool,    // Is this the current pane?
    indent: usize,       // Indentation level (0=session, 1=window, 2=pane)
    session_name: String,
    is_current_session: bool,
}

/// Picker application state
struct PickerApp {
    entries: Vec<PickerEntry>,
    filtered_indices: Vec<usize>,
    list_state: ListState,
    search_input: String,
    preview_content: String,
    show_help: bool,
    should_quit: bool,
    selected_target: Option<String>,
}

impl PickerApp {
    fn new(entries: Vec<PickerEntry>) -> Self {
        let filtered_indices: Vec<usize> = (0..entries.len()).collect();
        let mut list_state = ListState::default();
        // Start at first non-separator entry
        let first_valid = filtered_indices.iter()
            .position(|&i| !entries[i].is_separator)
            .unwrap_or(0);
        list_state.select(Some(first_valid));

        let mut app = Self {
            entries,
            filtered_indices,
            list_state,
            search_input: String::new(),
            preview_content: String::new(),
            show_help: false,
            should_quit: false,
            selected_target: None,
        };
        app.update_preview();
        app
    }

    fn selected_entry(&self) -> Option<&PickerEntry> {
        self.list_state.selected()
            .and_then(|i| self.filtered_indices.get(i))
            .and_then(|&idx| self.entries.get(idx))
    }

    fn move_selection(&mut self, delta: i32) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let mut new_pos = current + delta;

        // Wrap around
        let len = self.filtered_indices.len() as i32;
        if new_pos < 0 { new_pos = len - 1; }
        if new_pos >= len { new_pos = 0; }

        // Skip separators
        let mut attempts = 0;
        while attempts < len {
            let idx = self.filtered_indices[new_pos as usize];
            if !self.entries[idx].is_separator {
                break;
            }
            new_pos += if delta > 0 { 1 } else { -1 };
            if new_pos < 0 { new_pos = len - 1; }
            if new_pos >= len { new_pos = 0; }
            attempts += 1;
        }

        self.list_state.select(Some(new_pos as usize));
        self.update_preview();
    }

    fn filter_entries(&mut self) {
        if self.search_input.is_empty() {
            self.filtered_indices = (0..self.entries.len()).collect();
        } else {
            let query = self.search_input.to_lowercase();
            self.filtered_indices = self.entries.iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.is_separator || e.display.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Reset selection to first non-separator
        let first_valid = self.filtered_indices.iter()
            .position(|&i| !self.entries[i].is_separator)
            .unwrap_or(0);
        self.list_state.select(Some(first_valid));
        self.update_preview();
    }

    fn update_preview(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if entry.is_session || entry.is_separator {
                self.preview_content = format!("Session: {}", entry.session_name);
            } else {
                // Capture pane content
                if let Ok(output) = Command::new("tmux")
                    .args(["capture-pane", "-ep", "-t", &entry.target])
                    .output()
                {
                    self.preview_content = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .take(50)
                        .collect::<Vec<_>>()
                        .join("\n");
                }
            }
        }
    }

    fn select_current(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if !entry.is_session && !entry.is_separator {
                self.selected_target = Some(entry.target.clone());
                self.should_quit = true;
            }
        }
    }
}

fn parse_pick_entries() -> Result<Vec<PickerEntry>> {
    // Get current pane
    let current_pane = run_tmux_command(&[
        "display-message", "-p", "#{session_name}:#{window_index}.#{pane_index}",
    ])?.trim().to_string();
    let current_session_name = current_pane.split(':').next().unwrap_or("").to_string();

    // Get all panes
    let output = run_tmux_command(&[
        "list-panes", "-a", "-F",
        "#{session_name}\t#{window_index}\t#{pane_index}\t#{window_name}\t#{pane_title}\t#{pane_current_path}",
    ])?;

    let mut entries = Vec::new();
    let mut current_session = String::new();
    let mut current_window = String::new();
    let mut git_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut is_first_session = true;
    let mut session_idx = 0usize;

    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 6 { continue; }

        let session = parts[0];
        let window_idx = parts[1];
        let pane_idx = parts[2];
        let window_name = parts[3];
        let pane_title = parts[4];
        let pane_path = parts[5];

        let git_repo = get_git_repo_name(pane_path, &mut git_cache);
        let short_path = get_short_path(pane_path, git_repo.as_deref());
        let target = format!("{}:{}.{}", session, window_idx, pane_idx);
        let window_key = format!("{}:{}", session, window_idx);
        let is_current_pane = target == current_pane;
        let is_current_session = session == current_session_name;

        // Session header
        if session != current_session {
            session_idx += 1;

            if !is_first_session {
                entries.push(PickerEntry {
                    target: "---".to_string(),
                    display: String::new(),
                    is_session: false,
                    is_separator: true,
                    is_current: false,
                    indent: 0,
                    session_name: session.to_string(),
                    is_current_session: false,
                });
            }
            is_first_session = false;

            entries.push(PickerEntry {
                target: format!("{}:*", session),
                display: format!("⊟ {} {}", session_idx, session),
                is_session: true,
                is_separator: false,
                is_current: false,
                indent: 0,
                session_name: session.to_string(),
                is_current_session,
            });
            current_session = session.to_string();
            current_window.clear();
        }

        // Window/pane entry
        if window_key != current_window {
            current_window = window_key;
        }

        let marker = if is_current_pane { " ◀" } else { "" };
        let display = if pane_idx == "1" {
            // Show session_idx:window_idx without session name
            format!("⊡ {}:{} {} {} │ {}{}", session_idx, window_idx, window_name, pane_title, short_path, marker)
        } else {
            format!("⊙ {} │ {}{}", pane_title, short_path, marker)
        };

        entries.push(PickerEntry {
            target,
            display,
            is_session: false,
            is_separator: false,
            is_current: is_current_pane,
            indent: if pane_idx == "1" { 1 } else { 2 },
            session_name: session.to_string(),
            is_current_session,
        });
    }

    Ok(entries)
}

fn run_picker_tui(mut app: PickerApp) -> Result<Option<String>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(|f| draw_picker(f, &mut app))?;

        if app.should_quit {
            break;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Handle help overlay first
            if app.show_help {
                app.show_help = false;
                continue;
            }

            match (key.modifiers, key.code) {
                (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => app.should_quit = true,
                (_, KeyCode::Enter) => app.select_current(),
                (_, KeyCode::F(1)) | (KeyModifiers::CONTROL, KeyCode::Char('/')) => app.show_help = true,
                (KeyModifiers::CONTROL, KeyCode::Char('n')) | (_, KeyCode::Down) => app.move_selection(1),
                (KeyModifiers::CONTROL, KeyCode::Char('p')) | (_, KeyCode::Up) => app.move_selection(-1),
                (_, KeyCode::Backspace) => {
                    app.search_input.pop();
                    app.filter_entries();
                }
                (_, KeyCode::Char('?')) => app.show_help = true,
                (_, KeyCode::Char(c)) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.search_input.push(c);
                    app.filter_entries();
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(app.selected_target)
}

fn draw_picker(f: &mut Frame, app: &mut PickerApp) {
    let area = f.area();

    // Layout: header (3), search (3), list (flex), preview (40%), footer (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(3),  // Search input
            Constraint::Min(5),     // List
            Constraint::Percentage(35), // Preview
            Constraint::Length(1),  // Footer
        ])
        .split(area);

    // Header
    let header_text = format!(
        "rmux_helper {} │ https://github.com/idvorkin/settings\n⊟=session ⊡=window ⊙=pane ◀=current",
        VERSION
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // Search input
    let search = Paragraph::new(format!("pick> {}_", app.search_input))
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::BOTTOM).title("Search"));
    f.render_widget(search, chunks[1]);

    // List with tree lines
    let items: Vec<ListItem> = app.filtered_indices.iter()
        .enumerate()
        .map(|(pos, &idx)| {
            let entry = &app.entries[idx];
            if entry.is_separator {
                ListItem::new("").style(Style::default().fg(Color::DarkGray))
            } else {
                // Determine tree characters
                let tree_prefix = if entry.is_session {
                    String::new()
                } else {
                    // Check if this is the last item in its session
                    let is_last = app.filtered_indices.get(pos + 1)
                        .map(|&next_idx| {
                            let next = &app.entries[next_idx];
                            next.is_separator || next.is_session
                        })
                        .unwrap_or(true);

                    if entry.indent == 1 {
                        if is_last { "└─ ".to_string() } else { "├─ ".to_string() }
                    } else {
                        if is_last { "│  └─ ".to_string() } else { "│  ├─ ".to_string() }
                    }
                };

                let style = if entry.is_current {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else if entry.is_session {
                    if entry.is_current_session {
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    }
                } else if entry.is_current_session {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                ListItem::new(format!("{}{}", tree_prefix, entry.display)).style(style)
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Sessions"))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, chunks[2], &mut app.list_state);

    // Preview
    let preview = Paragraph::new(app.preview_content.as_str())
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, chunks[3]);

    // Footer with colored keybindings
    let footer_spans = Line::from(vec![
        Span::styled("?", Style::default().fg(Color::Yellow)),
        Span::styled(":help ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::styled("C-p/n", Style::default().fg(Color::Yellow)),
        Span::styled(":nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(":select ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::styled("C-c", Style::default().fg(Color::Yellow)),
        Span::styled(":quit ", Style::default().fg(Color::DarkGray)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("type", Style::default().fg(Color::Yellow)),
        Span::styled(":filter", Style::default().fg(Color::DarkGray)),
    ]);
    let footer = Paragraph::new(footer_spans);
    f.render_widget(footer, chunks[4]);

    // Help overlay
    if app.show_help {
        draw_help_overlay(f, area);
    }
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let help_text = format!(r#"
  rmux_helper pick - Tmux Session/Window/Pane Picker
  Version: {}

  NAVIGATION
    C-n / ↓         Move down
    C-p / ↑         Move up
    Enter           Switch to selected pane
    Esc / C-c       Cancel and quit
    Type            Filter by text
    ? / C-/         Show this help

  DISPLAY
    ⊟ Session       Session header (cyan)
    ├─ ⊡ Window     Window with first pane (green)
    │  └─ ⊙ Pane    Additional pane
    ◀               Current pane marker
    Bold            Current session

  Source: https://github.com/idvorkin/settings
  Path:   rust/tmux_helper

  Press any key to close..."#, VERSION);

    // Center the popup
    let popup_width = 60;
    let popup_height = 22;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let popup = Paragraph::new(help_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .style(Style::default().bg(Color::Black)));

    // Clear the background
    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(popup, popup_area);
}

fn pick_tui() -> Result<()> {
    let entries = parse_pick_entries()?;
    if entries.is_empty() {
        return Ok(());
    }

    let app = PickerApp::new(entries);

    if let Some(target) = run_picker_tui(app)? {
        let _ = Command::new("tmux")
            .args(["switch-client", "-t", &target])
            .output();
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::RenameAll) => rename_all(),
        Some(Commands::Info) => info(),
        Some(Commands::Rotate) => rotate(),
        Some(Commands::Third { command }) => third(&command),
        Some(Commands::PickTui) => pick_tui(),
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
        assert_eq!(generate_title(&info, "myproject"), Some("cl myproject".to_string()));
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
}
