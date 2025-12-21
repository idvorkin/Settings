use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::{HashMap, HashSet};
use std::process::Command;
use sysinfo::{Pid, ProcessRefreshKind, System};

#[derive(Parser)]
#[command(name = "rmux_helper")]
#[command(about = "A fast Tmux helper utility (Rust)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
        "#{pane_id}\t#{window_id}\t#{window_name}\t#{pane_pid}",
    ])?;

    let mut panes = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 4 {
            panes.push(PaneInfo {
                pane_id: parts[0].to_string(),
                window_id: parts[1].to_string(),
                window_name: parts[2].to_string(),
                pane_pid: parts[3].parse().unwrap_or(0),
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

fn generate_title(info: &ProcessInfo, short_path: &str) -> String {
    if process_tree_has_pattern(info, &["aider"]) {
        format!("ai {}", short_path)
    } else if process_tree_has_pattern(info, &["@anthropic-ai/claude-code", "claude"]) {
        format!("claude {}", short_path)
    } else if process_tree_has_pattern(info, &["vim", "nvim"]) {
        format!("vi {}", short_path)
    } else if process_tree_has_pattern(info, &["docker"]) {
        format!("docker {}", short_path)
    } else if info.name == "zsh" && info.children.is_empty() {
        format!("z {}", short_path)
    } else {
        info.name.clone()
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

        if pane.pane_pid == 0 {
            continue;
        }

        let Some(process_info) = get_process_info(&system, pane.pane_pid) else {
            continue;
        };

        let cwd = &process_info.cwd;
        let git_repo = get_git_repo_name(cwd, &mut git_cache);
        let short_path = get_short_path(cwd, git_repo.as_deref());
        let title = generate_title(&process_info, &short_path);

        set_tmux_title(&title, &pane.pane_id, &pane.window_id, &pane.window_name);
    }

    Ok(())
}

fn info() -> Result<()> {
    // Get current pane PID
    let pane_pid: u32 = run_tmux_command(&["display-message", "-p", "#{pane_pid}"])?
        .parse()
        .unwrap_or(0);

    let mut system = System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );

    let Some(process_info) = get_process_info(&system, pane_pid) else {
        println!("{{}}");
        return Ok(());
    };

    let cwd = &process_info.cwd;
    let mut git_cache = HashMap::new();
    let git_repo = get_git_repo_name(cwd, &mut git_cache);
    let short_path = get_short_path(cwd, git_repo.as_deref());
    let title = generate_title(&process_info, &short_path);

    println!(
        r#"{{"cwd":"{}","short_path":"{}","app":"{}","title":"{}","git_repo":{}}}"#,
        cwd,
        short_path,
        process_info.name,
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
    run_tmux_command(&["show-option", "-gqv", option]).unwrap_or_default()
}

fn set_tmux_option(option: &str, value: &str) {
    let _ = Command::new("tmux")
        .args(["set-option", "-g", option, value])
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::RenameAll => rename_all(),
        Commands::Info => info(),
        Commands::Rotate => rotate(),
        Commands::Third { command } => third(&command),
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
        assert_eq!(generate_title(&info, "myproject"), "claude myproject");
    }

    #[test]
    fn test_generate_title_vim() {
        let child = make_process_info("nvim", "nvim file.rs", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(generate_title(&info, "myproject"), "vi myproject");
    }

    #[test]
    fn test_generate_title_plain_shell() {
        let info = make_process_info("zsh", "/bin/zsh", vec![]);
        assert_eq!(generate_title(&info, "myproject"), "z myproject");
    }

    #[test]
    fn test_generate_title_docker() {
        let child = make_process_info("docker", "docker run nginx", vec![]);
        let info = make_process_info("zsh", "/bin/zsh", vec![child]);
        assert_eq!(generate_title(&info, "myproject"), "docker myproject");
    }
}
