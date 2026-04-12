//! Ratatui TUI for the link picker. See spec §Layout & Display and §Navigation.

use crate::link_picker::detect::{Category, GhState, Row};
use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::io;

/// Action returned from the TUI to the orchestrator. See spec §Actions.
#[derive(Debug)]
pub enum Action {
    Quit,
    Yank(Row),
    Open(Row),
    GhWeb(Row),
    Ssh(Row),
    SwapToPickTui,
}

struct App {
    rows: Vec<Row>,
    filtered: Vec<usize>, // indices into rows in display order (including separators)
    categories_present: Vec<Category>, // non-empty categories
    list_state: ListState,
    query: String,
    drilled_in: Option<Category>,
    horizontal: bool,
    action: Option<Action>,
    error_msg: Option<String>,
}

impl App {
    fn new(rows: Vec<Row>) -> Self {
        let mut app = Self {
            rows,
            filtered: Vec::new(),
            categories_present: Vec::new(),
            list_state: ListState::default(),
            query: String::new(),
            drilled_in: None,
            horizontal: true,
            action: None,
            error_msg: None,
        };
        app.rebuild_filter();
        app
    }

    /// Rebuild `filtered` + `categories_present` based on query + drill state.
    fn rebuild_filter(&mut self) {
        self.categories_present.clear();
        self.filtered.clear();

        let tokens = tokenize(&self.query);
        let matches: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| match_row(r, &tokens))
            .filter(|(_, r)| self.drilled_in.map_or(true, |c| r.category == c))
            .map(|(i, _)| i)
            .collect();

        // Sentinel indexing: we insert separators as usize::MAX entries and category
        // headers as (usize::MAX - 1 - category_idx). We disambiguate when rendering.
        let mut last_cat: Option<Category> = None;
        for idx in &matches {
            let cat = self.rows[*idx].category;
            if Some(cat) != last_cat {
                if self.drilled_in.is_none() {
                    self.filtered.push(header_sentinel(cat));
                }
                self.categories_present.push(cat);
                last_cat = Some(cat);
            }
            self.filtered.push(*idx);
        }

        // Snap selection to the first leaf (if any).
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            let first_leaf = self
                .filtered
                .iter()
                .position(|&i| i < SENTINEL_BASE)
                .unwrap_or(0);
            self.list_state.select(Some(first_leaf));
        }
    }
}

/// Sentinel values above the real index space mean "not a leaf".
const SENTINEL_BASE: usize = usize::MAX - 100;
fn header_sentinel(cat: Category) -> usize {
    SENTINEL_BASE + (cat as usize)
}
fn sentinel_to_category(s: usize) -> Option<Category> {
    if s < SENTINEL_BASE {
        return None;
    }
    match s - SENTINEL_BASE {
        1 => Some(Category::PullRequest),
        2 => Some(Category::Issue),
        3 => Some(Category::Commit),
        4 => Some(Category::File),
        5 => Some(Category::Repo),
        6 => Some(Category::Blog),
        7 => Some(Category::OtherLink),
        8 => Some(Category::Server),
        9 => Some(Category::Ip),
        _ => None,
    }
}

// ----- Filter tokenization (see spec §Filtering semantics, Divergence 1 & 2) -----

/// Divergence 1 from pick-tui: multi-digit tokens are NOT split per-digit,
/// because splitting would cause PR-number matches to misfire.
pub(crate) fn tokenize(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in query.split_whitespace() {
        // Split letter/digit boundaries ONCE per transition (not per character).
        let mut cur = String::new();
        let mut cur_is_digit = None;
        for ch in word.chars() {
            let this_is_digit = ch.is_ascii_digit();
            if cur_is_digit.map_or(false, |d: bool| d != this_is_digit) && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(ch);
            cur_is_digit = Some(this_is_digit);
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

/// Divergence 2: the category short-name tag is prepended to the row's
/// search string with a `\x1f` unit separator so substring matches can't
/// leak from the tag into the body.
pub(crate) fn row_search_string(r: &Row) -> String {
    // tag \x1f key repo-or-host context canonical
    format!(
        "{}\x1f{} {} {} {}",
        r.category.tag(),
        r.key,
        r.repo_or_host,
        r.context,
        r.canonical
    )
}

pub(crate) fn match_row(r: &Row, tokens: &[String]) -> bool {
    if tokens.is_empty() {
        return true;
    }
    let hay = row_search_string(r).to_lowercase();
    // Key column alone for digit tokens:
    let key_lc = r.key.to_lowercase();
    for tok in tokens {
        let t = tok.to_lowercase();
        if tok.chars().all(|c| c.is_ascii_digit()) {
            if !key_lc.contains(&t) {
                return false;
            }
        } else if !hay.contains(&t) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    fn mk(cat: Category, key: &str, context: &str) -> Row {
        Row {
            category: cat,
            canonical: format!("https://github.com/a/b/pull/{key}"),
            key: key.to_string(),
            repo_or_host: "b".to_string(),
            context: context.to_string(),
            enriched: None,
            count: 1,
            most_recent_line: 0,
        }
    }

    #[test]
    fn tokenize_splits_letter_digit_boundary_once() {
        assert_eq!(tokenize("pr68"), vec!["pr", "68"]);
        assert_eq!(tokenize("14 cl"), vec!["14", "cl"]);
    }

    #[test]
    fn multi_digit_token_not_split_per_digit() {
        assert_eq!(tokenize("1234"), vec!["1234"]);
    }

    #[test]
    fn tag_leak_is_prevented_by_unit_separator() {
        let r = mk(Category::PullRequest, "#68", "writing prose all day");
        // "pr" tag should NOT leak into matching against "prose"
        let s = row_search_string(&r);
        assert!(s.starts_with("pr\x1f"));
        assert!(!s[..3].contains("prose"));
    }

    #[test]
    fn digit_token_matches_key_only() {
        let r = mk(Category::PullRequest, "#68", "context 68 somewhere");
        assert!(match_row(&r, &[String::from("68")])); // in key
        let r2 = mk(Category::PullRequest, "#99", "context 68 somewhere");
        assert!(!match_row(&r2, &[String::from("68")])); // not in key
    }

    #[test]
    fn category_tag_matches_at_start() {
        let r = mk(Category::PullRequest, "#1", "");
        assert!(match_row(&r, &[String::from("pr")]));
        let r2 = mk(Category::Issue, "#1", "");
        assert!(!match_row(&r2, &[String::from("pr")]));
    }
}

// ----- Run loop + rendering -----

pub fn run(rows: Vec<Row>) -> Result<Action> {
    if rows.is_empty() {
        return Ok(Action::Quit);
    }
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(rows);
    let result = event_loop(&mut app, &mut terminal);

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    drop(terminal);

    result
}

fn event_loop<B: Backend>(app: &mut App, terminal: &mut Terminal<B>) -> Result<Action> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            handle_key(app, k.modifiers, k.code);
            if let Some(action) = app.action.take() {
                return Ok(action);
            }
        }
    }
}

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let main = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(5)])
        .split(area);

    // Top bar with breadcrumb or flat hints.
    let top = if let Some(cat) = app.drilled_in {
        format!(
            "pick> {}_  │ Links › {}  │ ↑↓ Enter:act ←:back F2:sess ?:help",
            app.query,
            cat.display()
        )
    } else {
        format!(
            "pick> {}_  │ ↑↓ Enter:act →:drill y:yank o:open g:gh F2:sess ?:help",
            app.query
        )
    };
    f.render_widget(
        Paragraph::new(top).style(Style::default().fg(Color::Yellow)),
        main[0],
    );

    // Split content horizontally or vertically
    let content_chunks = if app.horizontal {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(main[1])
    };

    // List
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&idx| render_item(app, idx))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Links"))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, content_chunks[0], &mut app.list_state);

    // Preview
    let preview_text = preview_for_selection(app);
    let preview = Paragraph::new(preview_text)
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, content_chunks[1]);
}

fn render_item(app: &App, idx: usize) -> ListItem<'static> {
    if let Some(cat) = sentinel_to_category(idx) {
        let count = app
            .filtered
            .iter()
            .filter(|i| **i < SENTINEL_BASE && app.rows[**i].category == cat)
            .count();
        return ListItem::new(format!("⊟ {} ({count})", cat.display()))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    }
    let r = &app.rows[idx];
    let tree = "├─ ";
    let glyph = r
        .enriched
        .as_ref()
        .map(|e| state_glyph(e.state))
        .unwrap_or("");
    let title = r
        .enriched
        .as_ref()
        .map(|e| e.title.clone())
        .unwrap_or_else(|| r.context.clone());
    let count = if r.count > 1 {
        format!("  ×{}", r.count)
    } else {
        String::new()
    };
    let spans = vec![
        Span::styled(tree, Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:<8} ", r.key),
            Style::default().fg(Color::LightYellow),
        ),
        Span::styled(
            format!("{:<16} ", r.repo_or_host),
            Style::default().fg(Color::LightGreen),
        ),
        Span::styled(
            format!("{glyph} "),
            Style::default().fg(glyph_color(r.enriched.as_ref().map(|e| e.state))),
        ),
        Span::styled(title, Style::default().fg(Color::LightMagenta)),
        Span::styled(count, Style::default().fg(Color::LightCyan)),
    ];
    ListItem::new(Line::from(spans))
}

fn state_glyph(s: GhState) -> &'static str {
    match s {
        GhState::Open => "◉",
        GhState::MergedPr => "●",
        GhState::Closed => "✕",
        GhState::Draft => "◐",
        GhState::Commit => "⎇",
    }
}

fn glyph_color(s: Option<GhState>) -> Color {
    match s {
        Some(GhState::Open) => Color::LightGreen,
        Some(GhState::MergedPr) => Color::LightMagenta,
        Some(GhState::Closed) => Color::DarkGray,
        Some(GhState::Draft) => Color::LightYellow,
        Some(GhState::Commit) => Color::LightBlue,
        None => Color::Reset,
    }
}

fn preview_for_selection(app: &App) -> Text<'static> {
    let Some(pos) = app.list_state.selected() else {
        return Text::raw("");
    };
    let Some(&idx) = app.filtered.get(pos) else {
        return Text::raw("");
    };
    if let Some(cat) = sentinel_to_category(idx) {
        return Text::raw(format!("Category: {} (header)", cat.display()));
    }
    let r = &app.rows[idx];
    let body = format!(
        "{}\n\ncanonical: {}\nkey: {}\nrepo/host: {}\ncount: {}",
        r.context, r.canonical, r.key, r.repo_or_host, r.count
    );
    body.clone()
        .into_text()
        .unwrap_or_else(|_| Text::raw(body))
}

fn handle_key(app: &mut App, mods: KeyModifiers, code: KeyCode) {
    // Esc: drill-out or quit
    if matches!(code, KeyCode::Esc) {
        if app.drilled_in.is_some() {
            app.drilled_in = None;
            app.rebuild_filter();
        } else {
            app.action = Some(Action::Quit);
        }
        return;
    }

    // Ctrl-C: clear query or quit
    if matches!(code, KeyCode::Char('c')) && mods.contains(KeyModifiers::CONTROL) {
        if !app.query.is_empty() {
            app.query.clear();
            app.rebuild_filter();
        } else {
            app.action = Some(Action::Quit);
        }
        return;
    }

    // Navigation
    match code {
        KeyCode::Down | KeyCode::Char('\x0e') => app.move_selection(1),
        KeyCode::Up | KeyCode::Char('\x10') => app.move_selection(-1),
        KeyCode::Right => app.drill_in(),
        KeyCode::Left => {
            if app.drilled_in.is_some() {
                app.drilled_in = None;
                app.rebuild_filter();
            }
        }
        KeyCode::Enter => app.on_enter(),
        KeyCode::F(2) => app.action = Some(Action::SwapToPickTui),
        KeyCode::F(1) => { /* help overlay — TODO v1.1 */ }
        KeyCode::Backspace => {
            app.query.pop();
            app.rebuild_filter();
        }
        KeyCode::Char('l') if mods.contains(KeyModifiers::CONTROL) => {
            app.horizontal = !app.horizontal;
        }
        KeyCode::Char('n') if mods.contains(KeyModifiers::CONTROL) => app.move_selection(1),
        KeyCode::Char('p') if mods.contains(KeyModifiers::CONTROL) => app.move_selection(-1),
        // Digit 1-9: jump to Nth category drilled-in view
        KeyCode::Char(c @ '1'..='9') if mods.is_empty() && app.query.is_empty() => {
            let n = (c as u8 - b'0') as usize;
            if let Some(cat) = app.categories_present.get(n - 1) {
                app.drilled_in = Some(*cat);
                app.rebuild_filter();
            }
        }
        // Override keys (on selected leaf only). Gated on empty query so they
        // don't collide with typing search queries containing these letters.
        KeyCode::Char('y') if mods.is_empty() && app.query.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Yank(row));
            }
        }
        KeyCode::Char('o') if mods.is_empty() && app.query.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Open(row));
            }
        }
        KeyCode::Char('g') if mods.is_empty() && app.query.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                if matches!(
                    row.category,
                    Category::PullRequest
                        | Category::Issue
                        | Category::Commit
                        | Category::File
                        | Category::Repo
                ) {
                    app.action = Some(Action::GhWeb(row));
                } else {
                    app.error_msg = Some("g: not a GitHub row".into());
                }
            }
        }
        KeyCode::Char('s') if mods.is_empty() && app.query.is_empty() => {
            if let Some(row) = app.selected_leaf() {
                app.action = Some(Action::Ssh(row));
            }
        }
        KeyCode::Char(c) if c.is_ascii_graphic() || c == ' ' => {
            app.query.push(c);
            app.rebuild_filter();
        }
        _ => {}
    }
}

impl App {
    /// Move selection by `delta`. Headers are selectable per spec — do NOT skip them.
    /// (Plan deviation: the plan's version skipped headers, contradicting the spec.)
    fn move_selection(&mut self, delta: i32) {
        let Some(cur) = self.list_state.selected() else {
            return;
        };
        let len = self.filtered.len() as i32;
        if len == 0 {
            return;
        }
        let next = (cur as i32 + delta).clamp(0, len - 1);
        self.list_state.select(Some(next as usize));
    }

    fn drill_in(&mut self) {
        if self.drilled_in.is_some() {
            return;
        }
        let Some(cur) = self.list_state.selected() else {
            return;
        };
        let Some(&idx) = self.filtered.get(cur) else {
            return;
        };
        let cat = if let Some(c) = sentinel_to_category(idx) {
            c
        } else {
            self.rows[idx].category
        };
        self.drilled_in = Some(cat);
        self.rebuild_filter();
    }

    fn selected_leaf(&self) -> Option<Row> {
        let cur = self.list_state.selected()?;
        let &idx = self.filtered.get(cur)?;
        if idx >= SENTINEL_BASE {
            return None;
        }
        Some(self.rows[idx].clone())
    }

    fn on_enter(&mut self) {
        let Some(cur) = self.list_state.selected() else {
            return;
        };
        let Some(&idx) = self.filtered.get(cur) else {
            return;
        };
        if let Some(cat) = sentinel_to_category(idx) {
            // Header: drill in
            self.drilled_in = Some(cat);
            self.rebuild_filter();
            return;
        }
        // Leaf: default action
        let row = self.rows[idx].clone();
        self.action = Some(default_action(&row));
    }
}

/// Default Enter action per category (see spec §Actions → Default).
pub(crate) fn default_action(row: &Row) -> Action {
    match row.category {
        Category::Server | Category::Ip => Action::Ssh(row.clone()),
        _ => Action::Yank(row.clone()),
    }
}
