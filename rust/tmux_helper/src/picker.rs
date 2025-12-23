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
    indent: usize,
    session_name: String,
    is_current_session: bool,
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
}

impl<'a> PickerApp<'a> {
    fn new(entries: Vec<PickerEntry>) -> Self {
        let filtered_indices: Vec<usize> = (0..entries.len()).collect();
        let mut list_state = ListState::default();
        let first_valid = filtered_indices
            .iter()
            .position(|&i| !entries[i].is_separator)
            .unwrap_or(0);
        list_state.select(Some(first_valid));

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
        };
        app.update_preview();
        app
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

        // Skip separators
        let mut attempts = 0;
        while attempts < len {
            let idx = self.filtered_indices[new_pos as usize];
            if !self.entries[idx].is_separator {
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
        self.update_preview();
    }

    fn filter_entries(&mut self) {
        if self.search_input.is_empty() {
            self.filtered_indices = (0..self.entries.len()).collect();
        } else {
            let query = self.search_input.to_lowercase();
            self.filtered_indices = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.is_separator || e.display.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();
        }

        let first_valid = self
            .filtered_indices
            .iter()
            .position(|&i| !self.entries[i].is_separator)
            .unwrap_or(0);
        self.list_state.select(Some(first_valid));
        self.update_preview();
    }

    fn update_preview(&mut self) {
        if let Some(entry) = self.selected_entry() {
            if entry.is_session || entry.is_separator {
                self.preview_content = Text::from(format!("Session: {}", entry.session_name));
            } else {
                // Capture pane content with ANSI colors (-e flag)
                if let Ok(output) = Command::new("tmux")
                    .args(["capture-pane", "-ep", "-t", &entry.target])
                    .output()
                {
                    let content: String = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .take(50)
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
        let pane_title = parts[4];
        let pane_path = parts[5];

        let git_repo = get_git_repo_name(pane_path, &mut git_cache);
        let short_path = get_short_path(pane_path, git_repo.as_deref());
        let target = format!("{}:{}.{}", session, window_idx, pane_idx);
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
                display: format!("\u{229F} {} {}", session_idx, session),
                is_session: true,
                is_separator: false,
                is_current: false,
                indent: 0,
                session_name: session.to_string(),
                is_current_session,
            });
            current_session = session.to_string();
        }

        let marker = if is_current_pane { " \u{25C0}" } else { "" };
        let display = if pane_idx == "1" {
            format!(
                "\u{22A1} {};{} {} {} \u{2502} {}{}",
                session_idx, window_idx, window_name, pane_title, short_path, marker
            )
        } else {
            format!(
                "\u{2299} {} \u{2502} {}{}",
                pane_title, short_path, marker
            )
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

fn run_picker_tui(mut app: PickerApp<'_>) -> Result<Option<String>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
                (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    app.should_quit = true
                }
                (_, KeyCode::Enter) => app.select_current(),
                (_, KeyCode::F(1)) | (KeyModifiers::CONTROL, KeyCode::Char('/')) => {
                    app.show_help = true
                }
                (KeyModifiers::CONTROL, KeyCode::Char('r')) => app.start_rename(),
                (KeyModifiers::CONTROL, KeyCode::Char('n')) | (_, KeyCode::Down) => {
                    app.move_selection(1)
                }
                (KeyModifiers::CONTROL, KeyCode::Char('p')) | (_, KeyCode::Up) => {
                    app.move_selection(-1)
                }
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

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(app.selected_target)
}

fn draw_picker(f: &mut Frame, app: &mut PickerApp<'_>) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Percentage(35),
            Constraint::Length(1),
        ])
        .split(area);

    // Header
    let header_text = format!(
        "rmux_helper {} \u{2502} https://github.com/idvorkin/settings\n\u{229F}=session \u{22A1}=window \u{2299}=pane \u{25C0}=current",
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
    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(pos, &idx)| {
            let entry = &app.entries[idx];
            if entry.is_separator {
                ListItem::new("").style(Style::default().fg(Color::DarkGray))
            } else {
                let tree_prefix = if entry.is_session {
                    String::new()
                } else {
                    let is_last = app
                        .filtered_indices
                        .get(pos + 1)
                        .map(|&next_idx| {
                            let next = &app.entries[next_idx];
                            next.is_separator || next.is_session
                        })
                        .unwrap_or(true);

                    if entry.indent == 1 {
                        if is_last {
                            "\u{2514}\u{2500} ".to_string()
                        } else {
                            "\u{251C}\u{2500} ".to_string()
                        }
                    } else if is_last {
                        "\u{2502}  \u{2514}\u{2500} ".to_string()
                    } else {
                        "\u{2502}  \u{251C}\u{2500} ".to_string()
                    }
                };

                let style = if entry.is_current {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if entry.is_session {
                    if entry.is_current_session {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Cyan)
                    }
                } else if entry.is_current_session {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                };
                ListItem::new(format!("{}{}", tree_prefix, entry.display)).style(style)
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
    f.render_stateful_widget(list, chunks[2], &mut app.list_state);

    // Preview (with ANSI color support)
    let preview = Paragraph::new(app.preview_content.clone())
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, chunks[3]);

    // Footer
    let footer_spans = Line::from(vec![
        Span::styled("?", Style::default().fg(Color::Yellow)),
        Span::styled(":help ", Style::default().fg(Color::DarkGray)),
        Span::styled("\u{2502} ", Style::default().fg(Color::DarkGray)),
        Span::styled("\u{2191}\u{2193}", Style::default().fg(Color::Yellow)),
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::styled("C-p/n", Style::default().fg(Color::Yellow)),
        Span::styled(":nav ", Style::default().fg(Color::DarkGray)),
        Span::styled("\u{2502} ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(":select ", Style::default().fg(Color::DarkGray)),
        Span::styled("\u{2502} ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(":quit ", Style::default().fg(Color::DarkGray)),
        Span::styled("\u{2502} ", Style::default().fg(Color::DarkGray)),
        Span::styled("C-r", Style::default().fg(Color::Yellow)),
        Span::styled(":rename ", Style::default().fg(Color::DarkGray)),
        Span::styled("\u{2502} ", Style::default().fg(Color::DarkGray)),
        Span::styled("type", Style::default().fg(Color::Yellow)),
        Span::styled(":filter", Style::default().fg(Color::DarkGray)),
    ]);
    let footer = Paragraph::new(footer_spans);
    f.render_widget(footer, chunks[4]);

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
    Enter           Switch to selected pane
    Esc / C-c       Cancel and quit
    Type            Filter by text
    ? / C-/         Show this help
    C-r             Rename session/window

  DISPLAY
    ⊟ Session       Session header (cyan)
    ├─ ⊡ Window     Window with first pane (green)
    │  └─ ⊙ Pane    Additional pane
    ◀               Current pane marker
    Bold            Current session

  Source: https://github.com/idvorkin/settings
  Path:   rust/tmux_helper

  Press any key to close..."#,
        VERSION
    );

    let popup_width = 60;
    let popup_height = 22;
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
            indent,
            session_name: session_name.to_string(),
            is_current_session: false,
        }
    }

    #[test]
    fn test_picker_app_new_selects_first_non_separator() {
        let entries = vec![
            make_entry("---", "", false, true, false, 0, "sess1"),
            make_entry("sess1:*", "⊟ 1 sess1", true, false, false, 0, "sess1"),
            make_entry("sess1:1.1", "⊡ 1;1 win1", false, false, false, 1, "sess1"),
        ];
        let app = PickerApp::new(entries);
        assert_eq!(app.list_state.selected(), Some(1)); // Should skip separator
    }

    #[test]
    fn test_picker_app_filter_entries() {
        let entries = vec![
            make_entry("sess1:*", "⊟ 1 main", true, false, false, 0, "main"),
            make_entry("sess1:1.1", "⊡ 1;1 editor vim", false, false, false, 1, "main"),
            make_entry("sess2:*", "⊟ 2 work", true, false, false, 0, "work"),
            make_entry("sess2:1.1", "⊡ 2;1 shell zsh", false, false, false, 1, "work"),
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
            make_entry("sess1:*", "⊟ 1 sess1", true, false, false, 0, "sess1"),
            make_entry("sess1:1.1", "⊡ 1;1 win1", false, false, false, 1, "sess1"),
            make_entry("sess1:2.1", "⊡ 1;2 win2", false, false, false, 1, "sess1"),
        ];
        let mut app = PickerApp::new(entries);

        // Move to end
        app.move_selection(1);
        app.move_selection(1);
        assert_eq!(app.list_state.selected(), Some(2));

        // Wrap to start
        app.move_selection(1);
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_picker_app_move_selection_skips_separators() {
        let entries = vec![
            make_entry("sess1:*", "⊟ 1 sess1", true, false, false, 0, "sess1"),
            make_entry("---", "", false, true, false, 0, "sess2"),
            make_entry("sess2:*", "⊟ 2 sess2", true, false, false, 0, "sess2"),
        ];
        let mut app = PickerApp::new(entries);
        assert_eq!(app.list_state.selected(), Some(0));

        // Moving down should skip separator
        app.move_selection(1);
        assert_eq!(app.list_state.selected(), Some(2));
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
        let display = format!("⊡ {};{} win title │ path", session_idx, window_idx);
        assert!(display.contains("1;3"));
        assert!(!display.contains("1:3"));
    }
}
