//! Ratatui-based TUI picker for tmux sessions/windows/panes

use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::collections::HashMap;
use std::io;
use std::process::Command;

use crate::{get_git_repo_name, get_short_path, run_tmux_command, VERSION};

/// Entry in the picker list
#[derive(Clone)]
struct PickerEntry {
    target: String,
    display: String,
    is_session: bool,
    is_separator: bool,
    is_current: bool,
    is_last: bool,          // Previous/last pane (for quick toggle)
    indent: usize,
    session_name: String,
    is_current_session: bool,
    // Structured column data for colored display
    col_index: String,      // e.g., "1;1"
    col_window: String,     // window name
    col_pane: String,       // pane title
    col_path: String,       // short path
}

/// Picker application state
struct PickerApp<'a> {
    entries: Vec<PickerEntry>,
    filtered_indices: Vec<usize>,
    list_state: ListState,
    search_input: String,
    preview_content: Text<'a>,
    show_help: bool,
    show_rename: bool,
    rename_input: String,
    should_quit: bool,
    selected_target: Option<String>,
    horizontal_layout: bool, // true = side-by-side, false = stacked
    preview_width: u16,
    preview_height: u16,
    // Dynamic column widths (calculated from content)
    col_width_index: usize,
    col_width_window: usize,
    col_width_path: usize,
}

impl<'a> PickerApp<'a> {
    fn new(entries: Vec<PickerEntry>) -> Self {
        let filtered_indices: Vec<usize> = (0..entries.len()).collect();
        let mut list_state = ListState::default();
        // Start on current pane, fallback to first window/pane (skip sessions)
        let initial_pos = filtered_indices
            .iter()
            .position(|&i| entries[i].is_current)
            .or_else(|| filtered_indices.iter().position(|&i| !entries[i].is_separator && !entries[i].is_session))
            .unwrap_or(0);
        list_state.select(Some(initial_pos));

        // Calculate dynamic column widths from content (with minimums)
        let calc_col_width = |extract: fn(&PickerEntry) -> &str, min: usize| {
            entries
                .iter()
                .filter(|e| !e.is_session && !e.is_separator)
                .map(|e| extract(e).chars().count())
                .max()
                .unwrap_or(min)
                .max(min)
        };
        let col_width_index = calc_col_width(|e| &e.col_index, 4);
        let col_width_window = calc_col_width(|e| &e.col_window, 3);
        let col_width_path = calc_col_width(|e| &e.col_path, 6);

        let mut app = Self {
            entries,
            filtered_indices,
            list_state,
            search_input: String::new(),
            preview_content: Text::default(),
            show_help: false,
            show_rename: false,
            rename_input: String::new(),
            should_quit: false,
            selected_target: None,
            horizontal_layout: true, // default to side-by-side
            preview_width: 80,
            preview_height: 40,
            col_width_index,
            col_width_window,
            col_width_path,
        };
        app.refresh_preview();
        app
    }

    fn toggle_layout(&mut self) {
        self.horizontal_layout = !self.horizontal_layout;
    }

    fn start_rename(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if !entry.is_separator {
                // Pre-fill with current name
                self.rename_input = if entry.is_session {
                    entry.session_name.clone()
                } else {
                    // Extract window name from display (after the session;window prefix)
                    entry.display
                        .split_whitespace()
                        .nth(2)
                        .unwrap_or("")
                        .to_string()
                };
                self.show_rename = true;
            }
        }
    }

    fn execute_rename(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            let new_name = self.rename_input.trim();
            if new_name.is_empty() {
                self.show_rename = false;
                self.rename_input.clear();
                return;
            }

            if entry.is_session {
                // Rename session
                let _ = Command::new("tmux")
                    .args(["rename-session", "-t", &entry.session_name, new_name])
                    .output();
            } else {
                // Rename window - target is "session:window.pane", we need "session:window"
                let window_target: String = entry.target
                    .rsplit_once('.')
                    .map(|(w, _)| w.to_string())
                    .unwrap_or(entry.target.clone());
                let _ = Command::new("tmux")
                    .args(["rename-window", "-t", &window_target, new_name])
                    .output();
            }

            self.show_rename = false;
            self.rename_input.clear();
            // Quit and let user reopen to see changes
            self.should_quit = true;
        }
    }

    fn selected_entry(&self) -> Option<&PickerEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered_indices.get(i))
            .and_then(|&idx| self.entries.get(idx))
    }

    fn move_selection(&mut self, delta: i32) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let mut new_pos = current + delta;
        let len = self.filtered_indices.len() as i32;

        // Wrap around
        if new_pos < 0 {
            new_pos = len - 1;
        }
        if new_pos >= len {
            new_pos = 0;
        }

        // Skip separators and sessions (only stop on windows/panes)
        let mut attempts = 0;
        while attempts < len {
            let idx = self.filtered_indices[new_pos as usize];
            if !self.entries[idx].is_separator && !self.entries[idx].is_session {
                break;
            }
            new_pos += if delta > 0 { 1 } else { -1 };
            if new_pos < 0 {
                new_pos = len - 1;
            }
            if new_pos >= len {
                new_pos = 0;
            }
            attempts += 1;
        }

        self.list_state.select(Some(new_pos as usize));
        self.refresh_preview();
    }

    fn filter_entries(&mut self) {
        if self.search_input.is_empty() {
            self.filtered_indices = (0..self.entries.len()).collect();
        } else {
            let query = self.search_input.to_lowercase();
            let tokens = tokenize_query(&query);
            self.filtered_indices = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.is_separator || fuzzy_match(&e.display.to_lowercase(), &tokens))
                .map(|(i, _)| i)
                .collect();
        }

        // Prefer current pane, fallback to first window/pane (skip sessions)
        let initial_pos = self
            .filtered_indices
            .iter()
            .position(|&i| self.entries[i].is_current)
            .or_else(|| self.filtered_indices.iter().position(|&i| !self.entries[i].is_separator && !self.entries[i].is_session))
            .unwrap_or(0);
        self.list_state.select(Some(initial_pos));
        self.refresh_preview();
    }

    fn refresh_preview(&mut self) {
        // Use stored dimensions
        let w = self.preview_width;
        let h = self.preview_height;
        self.update_preview(w, h);
    }

    fn set_preview_size(&mut self, width: u16, height: u16) {
        if width != self.preview_width || height != self.preview_height {
            self.preview_width = width;
            self.preview_height = height;
            self.refresh_preview();
        }
    }

    fn update_preview(&mut self, preview_width: u16, preview_height: u16) {
        if let Some(entry) = self.selected_entry() {
            if entry.is_session || entry.is_separator {
                self.preview_content = Text::from(format!("Session: {}", entry.session_name));
            } else {
                // Capture pane content with ANSI colors (-e flag)
                if let Ok(output) = Command::new("tmux")
                    .args(["capture-pane", "-ep", "-t", &entry.target])
                    .output()
                {
                    // Adjust lines based on preview shape
                    // Vertical (wide/short): fewer lines, full width
                    // Horizontal (narrow/tall): more lines
                    let max_lines = if preview_width > preview_height * 2 {
                        // Wide preview (vertical layout) - fewer lines
                        (preview_height.saturating_sub(2)) as usize
                    } else {
                        // Tall preview (horizontal layout) - more lines
                        (preview_height.saturating_sub(2)).max(30) as usize
                    };

                    // Optionally truncate long lines for narrow previews
                    let max_width = (preview_width.saturating_sub(2)) as usize;

                    let content: String = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .take(max_lines)
                        .map(|line| {
                            // For very narrow previews, truncate lines to reduce wrap chaos
                            if max_width < 60 && line.chars().count() > max_width {
                                let truncated: String = line.chars().take(max_width.saturating_sub(1)).collect();
                                format!("{}…", truncated)
                            } else {
                                line.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Convert ANSI escape codes to ratatui styled text
                    self.preview_content = content.into_text().unwrap_or_else(|_| Text::from(content));
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

    fn jump_to_last(&mut self) {
        // Find and select the "last" (previous) pane entry
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .position(|&idx| self.entries[idx].is_last)
        {
            self.list_state.select(Some(pos));
            self.refresh_preview();
        }
    }

    fn jump_to_current(&mut self) {
        // Find and select the current pane entry
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .position(|&idx| self.entries[idx].is_current)
        {
            self.list_state.select(Some(pos));
            self.refresh_preview();
        }
    }
}

/// Tokenize a search query for fuzzy matching.
/// Splits on whitespace AND at letter/digit boundaries.
/// Examples:
///   "se4" -> ["se", "4"]
///   "1;4" -> ["1", "4"]
///   "cl set" -> ["cl", "set"]
///   "vim blog" -> ["vim", "blog"]
fn tokenize_query(query: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut last_was_digit = None;

    for c in query.chars() {
        if c.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            last_was_digit = None;
        } else if c.is_ascii_digit() {
            // Split if transitioning from letter to digit
            if last_was_digit == Some(false) && !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            current.push(c);
            last_was_digit = Some(true);
        } else if c.is_alphabetic() {
            // Split if transitioning from digit to letter
            if last_was_digit == Some(true) && !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            current.push(c);
            last_was_digit = Some(false);
        } else {
            // Non-alphanumeric (like ';') - include in current token
            current.push(c);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Fuzzy match: all tokens must be found as substrings in the text.
/// For pure digit tokens (like "14"), also tries matching each digit separately.
fn fuzzy_match(text: &str, tokens: &[String]) -> bool {
    tokens.iter().all(|token| {
        // Direct substring match
        if text.contains(token) {
            return true;
        }
        // For pure digit tokens, try matching each digit (e.g., "14" matches "1;4")
        if token.chars().all(|c| c.is_ascii_digit()) && token.len() > 1 {
            return token.chars().all(|c| text.contains(c));
        }
        false
    })
}

/// Extract app prefix from window name, avoiding path duplication.
/// If window name is "<prefix> <path>" and path matches short_path, return just prefix.
/// Otherwise return the full window name.
fn extract_window_prefix(window_name: &str, short_path: &str) -> String {
    // Known prefixes from rename-all: cl, vi, ai, z, docker, j, jekyll
    if let Some((prefix, rest)) = window_name.split_once(' ') {
        // Check if the rest of the window name matches or contains the short_path
        let rest_trimmed = rest.trim_end_matches('/');
        let path_trimmed = short_path.trim_end_matches('/');
        if rest_trimmed == path_trimmed || rest_trimmed.ends_with(path_trimmed) {
            return prefix.to_string();
        }
    }
    // No match or no space - return full window name
    window_name.to_string()
}

fn parse_pick_entries() -> Result<Vec<PickerEntry>> {
    let current_pane = run_tmux_command(&[
        "display-message",
        "-p",
        "#{session_name}:#{window_index}.#{pane_index}",
    ])?
    .trim()
    .to_string();
    let current_session_name = current_pane.split(':').next().unwrap_or("").to_string();

    // Get the "last" (previous) pane target
    let last_pane = run_tmux_command(&["display-message", "-p", "-t", "{last}", "#{session_name}:#{window_index}.#{pane_index}"])
        .unwrap_or_default()
        .trim()
        .to_string();

    // Get hostname to filter out default pane titles
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_default();

    let output = run_tmux_command(&[
        "list-panes",
        "-a",
        "-F",
        "#{session_name}\t#{window_index}\t#{pane_index}\t#{window_name}\t#{pane_title}\t#{pane_current_path}",
    ])?;

    let mut entries = Vec::new();
    let mut current_session = String::new();
    let mut git_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut is_first_session = true;
    let mut session_idx = 0usize;

    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 6 {
            continue;
        }

        let session = parts[0];
        let window_idx = parts[1];
        let pane_idx = parts[2];
        let window_name = parts[3];
        let pane_title_raw = parts[4];
        let pane_path = parts[5];

        // Filter out pane title if it's just the hostname
        let pane_title = if pane_title_raw.eq_ignore_ascii_case(&hostname) {
            ""
        } else {
            pane_title_raw
        };

        let git_repo = get_git_repo_name(pane_path, &mut git_cache);
        let short_path = get_short_path(pane_path, git_repo.as_deref());
        let target = format!("{}:{}.{}", session, window_idx, pane_idx);
        let is_current_pane = target == current_pane;
        let is_last_pane = target == last_pane && !is_current_pane;
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
                    is_last: false,
                    indent: 0,
                    session_name: session.to_string(),
                    is_current_session: false,
                    col_index: String::new(),
                    col_window: String::new(),
                    col_pane: String::new(),
                    col_path: String::new(),
                });
            }
            is_first_session = false;

            entries.push(PickerEntry {
                target: format!("{}:*", session),
                display: format!("{} {}", session_idx, session),
                is_session: true,
                is_separator: false,
                is_current: false,
                is_last: false,
                indent: 0,
                session_name: session.to_string(),
                is_current_session,
                col_index: session_idx.to_string(),
                col_window: String::new(),
                col_pane: String::new(),
                col_path: String::new(),
            });
            current_session = session.to_string();
        }

        // Store structured column data for colored display
        let col_index = if pane_idx == "1" {
            format!("{};{}", session_idx, window_idx)
        } else {
            String::new()
        };
        // Extract app prefix from window name if it follows "<prefix> <path>" pattern
        // This avoids duplicating the path (already shown in col_path)
        let col_window = if pane_idx == "1" {
            extract_window_prefix(window_name, &short_path)
        } else {
            String::new()
        };
        let col_pane = pane_title.to_string();
        let col_path = short_path.clone();

        // Build display string for filtering/searching
        let marker = if is_current_pane {
            " \u{25C0}"  // ◀ current
        } else if is_last_pane {
            " \u{25C1}"  // ◁ last/previous
        } else {
            ""
        };
        let display = format!(
            "{} {} {} {}{}",
            col_index, col_window, col_pane, col_path, marker
        );

        entries.push(PickerEntry {
            target,
            display,
            is_session: false,
            is_separator: false,
            is_current: is_current_pane,
            is_last: is_last_pane,
            indent: if pane_idx == "1" { 1 } else { 2 },
            session_name: session.to_string(),
            is_current_session,
            col_index,
            col_window,
            col_pane,
            col_path,
        });
    }

    Ok(entries)
}

fn run_picker_tui(mut app: PickerApp<'_>) -> Result<Option<String>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Drain any stale events from launching the popup
    while event::poll(std::time::Duration::from_millis(1))? {
        let _ = event::read();
    }

    loop {
        terminal.draw(|f| draw_picker(f, &mut app))?;

        if app.should_quit {
            break;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Handle overlays first
            if app.show_help {
                app.show_help = false;
                continue;
            }

            if app.show_rename {
                match key.code {
                    KeyCode::Esc => {
                        app.show_rename = false;
                        app.rename_input.clear();
                    }
                    KeyCode::Enter => app.execute_rename(),
                    KeyCode::Backspace => {
                        app.rename_input.pop();
                    }
                    KeyCode::Char(c) => {
                        app.rename_input.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            match (key.modifiers, key.code) {
                (_, KeyCode::Esc) => {
                    app.should_quit = true;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    // C-c: clear search first, quit on second press (if search already empty)
                    if app.search_input.is_empty() {
                        app.should_quit = true;
                    } else {
                        app.search_input.clear();
                        app.filter_entries();
                    }
                }
                (_, KeyCode::Enter) => app.select_current(),
                (_, KeyCode::F(1)) | (KeyModifiers::CONTROL, KeyCode::Char('/')) => {
                    app.show_help = true
                }
                (KeyModifiers::CONTROL, KeyCode::Char('r')) => app.start_rename(),
                (KeyModifiers::CONTROL, KeyCode::Char('l')) => app.toggle_layout(),
                (KeyModifiers::CONTROL, KeyCode::Char('o')) | (_, KeyCode::Tab) => {
                    // Toggle between current and last pane
                    if app.selected_entry().map(|e| e.is_current).unwrap_or(false) {
                        app.jump_to_last();
                    } else {
                        app.jump_to_current();
                    }
                }
                // C-n/Down - handle modifier style, raw control char, and case variations
                (KeyModifiers::CONTROL, KeyCode::Char('n' | 'N'))
                | (_, KeyCode::Char('\x0e'))
                | (_, KeyCode::Down) => app.move_selection(1),
                // C-p/Up - handle modifier style, raw control char, and case variations
                (KeyModifiers::CONTROL, KeyCode::Char('p' | 'P'))
                | (_, KeyCode::Char('\x10'))
                | (_, KeyCode::Up) => app.move_selection(-1),
                (_, KeyCode::Backspace) => {
                    app.search_input.pop();
                    app.filter_entries();
                }
                (_, KeyCode::Char('?')) => app.show_help = true,
                // Only add printable chars to search (ignore control chars like \x10 from C-p)
                (_, KeyCode::Char(c)) if c.is_ascii_graphic() || c == ' ' => {
                    app.search_input.push(c);
                    app.filter_entries();
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(app.selected_target)
}

fn draw_picker(f: &mut Frame, app: &mut PickerApp<'_>) {
    let area = f.area();

    // Use dynamic column widths from app (calculated from content)
    let col_index_width = app.col_width_index;
    let col_window_width = app.col_width_window;
    let col_path_width = app.col_width_path;

    // Calculate actual max width needed for sessions list from content
    let max_entry_width = app
        .entries
        .iter()
        .map(|e| {
            if e.is_separator {
                0
            } else if e.is_session {
                e.display.chars().count() + 4 // session + padding
            } else {
                // tree_prefix(6) + index + space + window + space + path + pane_space + pane + marker(3)
                let pane_width = if e.col_pane.is_empty() { 0 } else { e.col_pane.chars().count() + 1 };
                6 + col_index_width + 1
                    + col_window_width + 1
                    + col_path_width
                    + pane_width
                    + 3
            }
        })
        .max()
        .unwrap_or(50) as u16;

    // Use horizontal if sessions fit in 50% without wrapping (+ borders/padding)
    let sessions_width_needed = max_entry_width + 6; // borders + highlight symbol
    let use_horizontal = app.horizontal_layout && area.width / 2 >= sessions_width_needed;

    // Main vertical layout: search+help, content (no footer - help has details)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Search + minimal help
            Constraint::Min(5),    // Content area (sessions + preview)
        ])
        .split(area);

    // Split content area based on layout mode
    let (sessions_area, preview_area) = if use_horizontal {
        // Horizontal: sessions get minimum needed, preview gets the rest
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sessions_width_needed), Constraint::Min(1)])
            .split(main_chunks[1]);
        (content_chunks[0], content_chunks[1])
    } else {
        // Vertical: sessions on top sized to fit content, preview gets rest
        // +2 for borders, +1 for some padding
        let session_height = (app.filtered_indices.len() as u16 + 3).min(main_chunks[1].height.saturating_sub(5));
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(session_height), Constraint::Min(5)])
            .split(main_chunks[1]);
        (content_chunks[0], content_chunks[1])
    };

    // Update preview size if changed (triggers re-parse for new dimensions)
    app.set_preview_size(preview_area.width, preview_area.height);

    // Combined search + help line
    let search_help = Line::from(vec![
        Span::styled(format!("pick> {}", app.search_input), Style::default().fg(Color::Yellow)),
        Span::styled("_ ", Style::default().fg(Color::Yellow)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("↑↓", Style::default().fg(Color::DarkGray)),
        Span::styled(" Tab", Style::default().fg(Color::DarkGray)),
        Span::styled(":◀◁ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::styled(":sel ", Style::default().fg(Color::DarkGray)),
        Span::styled("?", Style::default().fg(Color::Yellow)),
        Span::styled(":help", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(search_help), main_chunks[0]);

    // List with tree lines and colored columns
    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(pos, &idx)| {
            let entry = &app.entries[idx];
            if entry.is_separator {
                ListItem::new("").style(Style::default().fg(Color::DarkGray))
            } else if entry.is_session {
                // Session header - simple colored display
                let style = if entry.is_current_session {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                ListItem::new(entry.display.clone()).style(style)
            } else {
                // Window/pane entry - build colored spans
                let tree_prefix = {
                    let is_last = app
                        .filtered_indices
                        .get(pos + 1)
                        .map(|&next_idx| {
                            let next = &app.entries[next_idx];
                            next.is_separator || next.is_session
                        })
                        .unwrap_or(true);

                    if entry.indent == 1 {
                        if is_last { "└─ " } else { "├─ " }
                    } else if is_last {
                        "│  └─ "
                    } else {
                        "│  ├─ "
                    }
                };

                // Pad columns to dynamic width + 1 for spacing
                let idx_padded = format!("{:<width$} ", entry.col_index, width = col_index_width);
                let win_padded = format!("{:<width$} ", entry.col_window, width = col_window_width);
                let path_padded = format!("{:<width$}", entry.col_path, width = col_path_width);

                // Order: Index, Window, Path, Pane (pane last, often empty/hostname)
                let mut spans = vec![
                    Span::styled(tree_prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(idx_padded, Style::default().fg(Color::LightYellow)),
                    Span::styled(win_padded, Style::default().fg(Color::LightGreen)),
                    Span::styled(path_padded, Style::default().fg(Color::LightMagenta)),
                ];
                // Only show pane if not empty (with leading space)
                if !entry.col_pane.is_empty() {
                    spans.push(Span::styled(format!(" {}", entry.col_pane), Style::default().fg(Color::LightCyan)));
                }
                // Add marker for current/last
                let marker = if entry.is_current {
                    " ◀"
                } else if entry.is_last {
                    " ◁"
                } else {
                    ""
                };
                if !marker.is_empty() {
                    let marker_color = if entry.is_current { Color::White } else { Color::Yellow };
                    spans.push(Span::styled(marker, Style::default().fg(marker_color).add_modifier(Modifier::BOLD)));
                }

                // Highlight current/last window/pane with background color
                let item = ListItem::new(Line::from(spans));
                if entry.is_current {
                    item.style(Style::default().bg(Color::Rgb(40, 40, 60)))
                } else if entry.is_last {
                    item.style(Style::default().bg(Color::Rgb(50, 40, 30)))
                } else {
                    item
                }
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Sessions"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("\u{25B6} ");
    f.render_stateful_widget(list, sessions_area, &mut app.list_state);

    // Preview (with ANSI color support)
    let preview = Paragraph::new(app.preview_content.clone())
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, preview_area);

    // Overlays
    if app.show_rename {
        draw_rename_overlay(f, area, app);
    } else if app.show_help {
        draw_help_overlay(f, area);
    }
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let help_text = format!(
        r#"
  rmux_helper pick - Tmux Session/Window/Pane Picker
  Version: {}

  NAVIGATION
    C-n / ↓         Move down
    C-p / ↑         Move up
    Tab / C-o       Jump between ◀current and ◁last
    Enter           Switch to selected pane
    Esc             Cancel and quit
    C-c             Clear search (quit if empty)
    Type            Filter by text (numbers match indices)
    ? / C-/         Show this help
    C-r             Rename session/window
    C-l             Toggle layout (horizontal/vertical)

  DISPLAY
    Session         Session header (cyan)
    ├─ Window       Window entry (green name)
    │  └─ Pane      Additional pane in window
    ◀               Current pane (highlighted)
    ◁               Last/previous pane (highlighted)
    Bold            Current session

  GitHub: https://github.com/idvorkin/settings/tree/main/rust/tmux_helper

  Press any key to close..."#,
        VERSION
    );

    // Dynamic sizing based on content
    let line_count = help_text.lines().count() as u16;
    let max_line_width = help_text.lines().map(|l| l.chars().count()).max().unwrap_or(60) as u16;
    let popup_width = (max_line_width + 4).min(area.width.saturating_sub(4));
    let popup_height = (line_count + 2).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let popup = Paragraph::new(help_text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .style(Style::default().bg(Color::Black)),
        );

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(popup, popup_area);
}

fn draw_rename_overlay(f: &mut Frame, area: Rect, app: &PickerApp<'_>) {
    let is_session = app
        .selected_entry()
        .map(|e| e.is_session)
        .unwrap_or(false);
    let title = if is_session {
        " Rename Session "
    } else {
        " Rename Window "
    };

    let popup_width = 50;
    let popup_height = 5;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().bg(Color::Black));

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(block, popup_area);

    let input = Paragraph::new(format!("{}_", app.rename_input))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(input, inner[0]);

    let hint = Paragraph::new("Enter: confirm  Esc: cancel")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, inner[2]);
}

/// Run the TUI picker and switch to the selected pane
pub fn pick_tui() -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        target: &str,
        display: &str,
        is_session: bool,
        is_separator: bool,
        is_current: bool,
        indent: usize,
        session_name: &str,
    ) -> PickerEntry {
        PickerEntry {
            target: target.to_string(),
            display: display.to_string(),
            is_session,
            is_separator,
            is_current,
            is_last: false,
            indent,
            session_name: session_name.to_string(),
            is_current_session: false,
            col_index: String::new(),
            col_window: String::new(),
            col_pane: String::new(),
            col_path: String::new(),
        }
    }

    fn make_pane_entry(col_index: &str, col_window: &str, col_path: &str, col_pane: &str) -> PickerEntry {
        PickerEntry {
            target: "sess:1.1".to_string(),
            display: format!("{} {} {} {}", col_index, col_window, col_path, col_pane),
            is_session: false,
            is_separator: false,
            is_current: false,
            is_last: false,
            indent: 1,
            session_name: "sess".to_string(),
            is_current_session: false,
            col_index: col_index.to_string(),
            col_window: col_window.to_string(),
            col_pane: col_pane.to_string(),
            col_path: col_path.to_string(),
        }
    }

    /// Calculate entry width with given column widths (mirrors logic in draw_picker)
    fn calc_entry_width_with_cols(e: &PickerEntry, col_index: usize, col_window: usize, col_path: usize) -> usize {
        if e.is_separator {
            0
        } else if e.is_session {
            e.display.chars().count() + 4
        } else {
            // tree_prefix(6) + index + space + window + space + path + pane_space + pane + marker(3)
            let pane_width = if e.col_pane.is_empty() { 0 } else { e.col_pane.chars().count() + 1 };
            6 + col_index + 1 + col_window + 1 + col_path + pane_width + 3
        }
    }

    #[test]
    fn test_picker_app_new_selects_first_window_pane() {
        let entries = vec![
            make_entry("---", "", false, true, false, 0, "sess1"),
            make_entry("sess1:*", "1 sess1", true, false, false, 0, "sess1"),
            make_entry("sess1:1.1", "1;1 win1", false, false, false, 1, "sess1"),
        ];
        let app = PickerApp::new(entries);
        assert_eq!(app.list_state.selected(), Some(2)); // Should skip separator AND session
    }

    #[test]
    fn test_picker_app_filter_entries() {
        let entries = vec![
            make_entry("sess1:*", "1 main", true, false, false, 0, "main"),
            make_entry("sess1:1.1", "1;1 editor vim", false, false, false, 1, "main"),
            make_entry("sess2:*", "2 work", true, false, false, 0, "work"),
            make_entry("sess2:1.1", "2;1 shell zsh", false, false, false, 1, "work"),
        ];
        let mut app = PickerApp::new(entries);

        app.search_input = "vim".to_string();
        app.filter_entries();

        // Should only show entries containing "vim" (plus separators)
        assert!(app.filtered_indices.len() < 4);
    }

    #[test]
    fn test_picker_app_move_selection_wraps() {
        let entries = vec![
            make_entry("sess1:*", "1 sess1", true, false, false, 0, "sess1"),
            make_entry("sess1:1.1", "1;1 win1", false, false, false, 1, "sess1"),
            make_entry("sess1:2.1", "1;2 win2", false, false, false, 1, "sess1"),
        ];
        let mut app = PickerApp::new(entries);
        // Starts on first window (index 1), not session
        assert_eq!(app.list_state.selected(), Some(1));

        // Move to second window
        app.move_selection(1);
        assert_eq!(app.list_state.selected(), Some(2));

        // Wrap to first window (skipping session)
        app.move_selection(1);
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_picker_app_move_selection_skips_sessions_and_separators() {
        let entries = vec![
            make_entry("sess1:*", "1 sess1", true, false, false, 0, "sess1"),
            make_entry("sess1:1.1", "1;1 win1", false, false, false, 1, "sess1"),
            make_entry("---", "", false, true, false, 0, "sess2"),
            make_entry("sess2:*", "2 sess2", true, false, false, 0, "sess2"),
            make_entry("sess2:1.1", "2;1 win2", false, false, false, 1, "sess2"),
        ];
        let mut app = PickerApp::new(entries);
        assert_eq!(app.list_state.selected(), Some(1)); // First window

        // Moving down should skip separator AND session header
        app.move_selection(1);
        assert_eq!(app.list_state.selected(), Some(4)); // Second session's window
    }

    #[test]
    fn test_window_target_extraction() {
        // Test the window target extraction logic used in execute_rename
        let target = "mysession:3.2";
        let window_target: String = target
            .rsplit_once('.')
            .map(|(w, _)| w.to_string())
            .unwrap_or(target.to_string());
        assert_eq!(window_target, "mysession:3");
    }

    #[test]
    fn test_display_format_uses_semicolon() {
        // Verify the display format uses semicolon separator
        let session_idx = 1;
        let window_idx = "3";
        let display = format!("{};{} win title │ path", session_idx, window_idx);
        assert!(display.contains("1;3"));
        assert!(!display.contains("1:3"));
    }

    #[test]
    fn test_ansi_to_tui_tig_style() {
        use ansi_to_tui::IntoText;
        // Tig-style ANSI codes: bold, white on green background
        let tig_line = "\x1b[1m\x1b[37m\x1b[42m77243d4 commit\x1b[0m\x1b[37m\x1b[42m\n\x1b[35m\x1b[49m8126872 another\x1b[0m";
        let result = tig_line.into_text();
        assert!(result.is_ok(), "ansi-to-tui should parse tig output: {:?}", result.err());
        let text = result.unwrap();
        assert_eq!(text.lines.len(), 2, "Should have 2 lines");
        // Check first line has spans with styles
        assert!(!text.lines[0].spans.is_empty(), "First line should have spans");
    }

    #[test]
    fn test_ansi_to_tui_live_tig() {
        use ansi_to_tui::IntoText;
        use ratatui::style::Modifier;
        // Real tig output exactly as captured - note the trailing [0m[37m[42m before newline
        let tig_content = "\x1b[1m\x1b[37m\x1b[42m77243d4 54 seconds ago Aidvorkin o [ratatui-picker] refactor(tmux): extract picker to module + add ANSI color preview\x1b[0m\x1b[37m\x1b[42m\n\x1b[35m\x1b[49m8126872 \x1b[34m18 minutes ago \x1b[32mAidvorkin \x1b[34mo\x1b[39m feat(tmux): add ratatui-based session/window/pane picker\n\x1b[35m0ddfb46 \x1b[34m    2 days ago \x1b[32mIDvorkin  \x1b[34mM\x1b[33m─┐\x1b[39m \x1b[36m[main]\x1b[39m \x1b[36m[sessionx-keybindings]\x1b[39m Merge";
        let result = tig_content.into_text();
        assert!(result.is_ok(), "ansi-to-tui should parse complex tig output");
        let text = result.unwrap();
        assert_eq!(text.lines.len(), 3, "Should have 3 lines");

        // Print debug info
        for (i, line) in text.lines.iter().enumerate() {
            eprintln!("Line {}: {} spans", i, line.spans.len());
            for (j, span) in line.spans.iter().enumerate() {
                let bold = span.style.add_modifier.contains(Modifier::BOLD);
                eprintln!("  Span {}: {:?} bg={:?} fg={:?} bold={}", j, span.content, span.style.bg, span.style.fg, bold);
            }
        }
    }

    #[test]
    fn test_entry_width_with_dynamic_columns() {
        // With dynamic columns, width depends on the column widths provided
        let entry = make_pane_entry("1;1", "vim", "blog", "");

        // Example with small columns: index=3, window=3, path=4
        // tree(6) + index(3) + space(1) + window(3) + space(1) + path(4) + pane(0) + marker(3) = 21
        assert_eq!(calc_entry_width_with_cols(&entry, 3, 3, 4), 21);

        // Example with larger columns: index=4, window=6, path=10
        // tree(6) + 4 + 1 + 6 + 1 + 10 + 0 + 3 = 31
        assert_eq!(calc_entry_width_with_cols(&entry, 4, 6, 10), 31);
    }

    #[test]
    fn test_entry_width_varies_with_pane_title() {
        let no_pane = make_pane_entry("1;1", "vim", "blog", "");
        let with_pane = make_pane_entry("1;1", "vim", "blog", "my-title");

        // Pane title adds to width (8 chars + 1 space = 9 difference)
        let w1 = calc_entry_width_with_cols(&no_pane, 4, 4, 4);
        let w2 = calc_entry_width_with_cols(&with_pane, 4, 4, 4);
        assert_eq!(w2 - w1, 9);
    }

    #[test]
    fn test_session_width_from_display() {
        let session = make_entry("sess:*", "1 my-session", true, false, false, 0, "my-session");

        // Session width = display length + 4 padding (independent of column widths)
        assert_eq!(calc_entry_width_with_cols(&session, 4, 4, 4), 12 + 4);
    }

    #[test]
    fn test_separator_has_zero_width() {
        let sep = make_entry("---", "", false, true, false, 0, "");
        assert_eq!(calc_entry_width_with_cols(&sep, 4, 4, 4), 0);
    }

    #[test]
    fn test_tokenize_query() {
        // Split at letter/digit boundaries
        assert_eq!(tokenize_query("se4"), vec!["se", "4"]);
        assert_eq!(tokenize_query("4se"), vec!["4", "se"]);
        assert_eq!(tokenize_query("cl2set"), vec!["cl", "2", "set"]);

        // Semicolon stays with adjacent chars
        assert_eq!(tokenize_query("1;4"), vec!["1;4"]);

        // Whitespace splits
        assert_eq!(tokenize_query("cl set"), vec!["cl", "set"]);
        assert_eq!(tokenize_query("vim blog"), vec!["vim", "blog"]);

        // Combined
        assert_eq!(tokenize_query("se4 blog"), vec!["se", "4", "blog"]);
    }

    #[test]
    fn test_fuzzy_match() {
        let text = "1;4 cl settings rmux";

        // Single tokens
        assert!(fuzzy_match(text, &["4".to_string()]));
        assert!(fuzzy_match(text, &["cl".to_string()]));
        assert!(fuzzy_match(text, &["set".to_string()]));

        // Multiple tokens - all must match
        assert!(fuzzy_match(text, &["se".to_string(), "4".to_string()]));
        assert!(fuzzy_match(text, &["cl".to_string(), "set".to_string()]));

        // Token not found
        assert!(!fuzzy_match(text, &["vim".to_string()]));
        assert!(!fuzzy_match(text, &["cl".to_string(), "vim".to_string()]));

        // Exact index match
        assert!(fuzzy_match(text, &["1;4".to_string()]));

        // Pure digit token "14" should match "1;4" (each digit found)
        assert!(fuzzy_match(text, &["14".to_string()]));
        // But "15" should NOT match (no 5 in text)
        assert!(!fuzzy_match(text, &["15".to_string()]));
    }

    #[test]
    fn test_filter_matches_window_numbers() {
        // Verify that typing a number filters to matching window indices
        let entries = vec![
            make_entry("sess1:*", "1 main", true, false, false, 0, "main"),
            make_pane_entry("1;1", "vim", "blog", ""),          // session 1, window 1
            make_pane_entry("1;4", "cl", "settings", ""),       // session 1, window 4
            make_entry("sess2:*", "2 work", true, false, false, 0, "work"),
            make_pane_entry("2;1", "z", "home", ""),            // session 2, window 1
            make_pane_entry("2;3", "docker", "app", ""),        // session 2, window 3
        ];
        let mut app = PickerApp::new(entries);

        // Typing "4" should match entry with "1;4" in col_index
        app.search_input = "4".to_string();
        app.filter_entries();
        // Should find: the entry with "1;4" (and separators are included)
        let matched_displays: Vec<_> = app.filtered_indices.iter()
            .map(|&i| app.entries[i].display.clone())
            .filter(|d| !d.is_empty() && d.contains("4"))
            .collect();
        assert!(matched_displays.iter().any(|d| d.contains("1;4")),
            "Typing '4' should match entry with col_index '1;4', got {:?}", matched_displays);

        // Typing "1" should match entries with "1" in session or window index
        app.search_input = "1".to_string();
        app.filter_entries();
        let matched_count = app.filtered_indices.iter()
            .filter(|&&i| !app.entries[i].is_separator && !app.entries[i].is_session)
            .filter(|&&i| app.entries[i].display.contains("1"))
            .count();
        assert!(matched_count >= 3, "Typing '1' should match multiple entries, got {}", matched_count);

        // Typing "se4" should match entry with "settings" AND "4"
        app.search_input = "se4".to_string();
        app.filter_entries();
        let matched: Vec<_> = app.filtered_indices.iter()
            .filter(|&&i| !app.entries[i].is_separator && !app.entries[i].is_session)
            .map(|&i| app.entries[i].display.clone())
            .collect();
        assert!(matched.iter().any(|d| d.contains("settings") && d.contains("1;4")),
            "Typing 'se4' should match entry with settings and 1;4, got {:?}", matched);

        // Typing "1;4" should match exactly that index
        app.search_input = "1;4".to_string();
        app.filter_entries();
        let matched: Vec<_> = app.filtered_indices.iter()
            .filter(|&&i| !app.entries[i].is_separator && !app.entries[i].is_session)
            .map(|&i| app.entries[i].display.clone())
            .collect();
        assert!(matched.iter().any(|d| d.contains("1;4")),
            "Typing '1;4' should match entry with that index, got {:?}", matched);
    }

    #[test]
    fn test_clear_search_resets_filter() {
        // Verify that clearing search shows all entries again
        let entries = vec![
            make_entry("sess1:*", "1 main", true, false, false, 0, "main"),
            make_pane_entry("1;1", "vim", "blog", ""),
            make_pane_entry("1;2", "cl", "settings", ""),
        ];
        let mut app = PickerApp::new(entries.clone());
        let original_count = app.filtered_indices.len();

        // Filter down
        app.search_input = "vim".to_string();
        app.filter_entries();
        assert!(app.filtered_indices.len() < original_count);

        // Clear search (simulates C-c behavior)
        app.search_input.clear();
        app.filter_entries();
        assert_eq!(app.filtered_indices.len(), original_count,
            "Clearing search should restore all entries");
    }

    #[test]
    fn test_extract_window_prefix() {
        // Window name with path that matches short_path -> extract prefix
        assert_eq!(extract_window_prefix("cl settings/rust", "settings/rust"), "cl");
        assert_eq!(extract_window_prefix("vi blog/", "blog"), "vi");
        assert_eq!(extract_window_prefix("z ~/projects", "~/projects"), "z");

        // Window name with non-matching path -> keep full name
        assert_eq!(extract_window_prefix("cl other-path", "settings/rust"), "cl other-path");

        // Window name without space -> keep full name
        assert_eq!(extract_window_prefix("btm", "settings"), "btm");
    }
}
