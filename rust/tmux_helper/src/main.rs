mod picker;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::{HashMap, HashSet};
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
