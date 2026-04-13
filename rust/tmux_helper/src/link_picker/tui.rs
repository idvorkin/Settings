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
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
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
    show_help: bool,
    action: Option<Action>,
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
            show_help: false,
            action: None,
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
        6 => Some(Category::Gist),
        7 => Some(Category::Blog),
        8 => Some(Category::OtherLink),
        9 => Some(Category::Server),
        10 => Some(Category::Ip),
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
        // A Server row whose key (hostname) starts with "ose-..." must NOT match
        // the query "serverose" — the `\x1f` separator between the `server` tag
        // and the key prevents the two from being seen as a single substring.
        // Without the separator the search string would be `serverose-host ...`
        // and the substring `serverose` would match; with the separator we get
        // `server\x1fose-host ...` and the boundary breaks the match.
        let r = mk(Category::Server, "ose-host", "context unrelated");
        let s = row_search_string(&r);
        assert!(s.starts_with("server\x1f"), "tag should be at the start");
        assert!(
            s.contains("server\x1fose"),
            "separator must sit between tag and key"
        );
        // The whole concatenated string lowercased must NOT contain "serverose"
        // because the `\x1f` byte breaks the substring.
        assert!(
            !s.to_lowercase().contains("serverose"),
            "unit separator must prevent `server` + `ose` from forming `serverose`"
        );
        // End-to-end: match_row for the `serverose` token should return false.
        assert!(!match_row(&r, &[String::from("serverose")]));
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

#[cfg(test)]
mod help_overlay_tests {
    use super::*;

    fn app_with_one_row() -> App {
        let row = Row {
            category: Category::Server,
            canonical: "c-5001".into(),
            key: "c-5001".into(),
            repo_or_host: "—".into(),
            context: "ssh c-5001".into(),
            enriched: None,
            count: 1,
            most_recent_line: 0,
        };
        App::new(vec![row])
    }

    #[test]
    fn question_mark_opens_help_overlay() {
        let mut app = app_with_one_row();
        assert!(!app.show_help, "help starts hidden");
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Char('?'));
        assert!(app.show_help, "? opens help");
        assert!(app.action.is_none(), "? must not trigger an action");
    }

    #[test]
    fn f1_opens_help_overlay() {
        let mut app = app_with_one_row();
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::F(1));
        assert!(app.show_help, "F1 opens help");
        assert!(app.action.is_none());
    }

    #[test]
    fn any_key_dismisses_help_without_side_effects() {
        let mut app = app_with_one_row();
        app.show_help = true;
        // Arrow key should dismiss help, NOT move selection, NOT trigger action.
        let selection_before = app.list_state.selected();
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Down);
        assert!(!app.show_help, "Down dismisses help");
        assert_eq!(
            app.list_state.selected(),
            selection_before,
            "selection untouched"
        );
        assert!(app.action.is_none(), "dismiss must not trigger action");
    }

    #[test]
    fn esc_dismisses_help_without_quitting() {
        let mut app = app_with_one_row();
        app.show_help = true;
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Esc);
        assert!(!app.show_help, "Esc dismisses help");
        assert!(app.action.is_none(), "Esc on help overlay must not quit");
    }

    #[test]
    fn enter_dismisses_help_without_firing_default_action() {
        let mut app = app_with_one_row();
        app.show_help = true;
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Enter);
        assert!(!app.show_help);
        assert!(
            app.action.is_none(),
            "Enter on help overlay must not fire default action"
        );
    }

    #[test]
    fn question_mark_does_not_leak_into_query() {
        // Regression guard: `?` must be intercepted before the generic
        // ascii_graphic char handler that types into the query field.
        // Pre-populate the query so the assertion fails loudly if the
        // `?` handler starts appending (rather than passing vacuously
        // when the query is already empty).
        let mut app = app_with_one_row();
        app.query = "abc".to_string();
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::Char('?'));
        assert_eq!(app.query, "abc", "? must not type into query");
    }

    #[test]
    fn ctrl_c_while_help_shown_dismisses_help_not_quits() {
        // Pins the airtightness-order claim in handle_key: the
        // `if app.show_help { … return; }` branch MUST run before the
        // Ctrl+C → Quit branch. Otherwise a future refactor that moves
        // the Ctrl+C handler above the dismiss check would silently
        // change Ctrl+C-while-help from "dismiss modal" to "quit picker"
        // — a surprising behavior change reviewers would miss.
        //
        // Current decision: first Ctrl+C dismisses help; user can press
        // Ctrl+C again from the flat view to quit. If we ever want
        // "Ctrl+C always quits", flip this assertion and update
        // handle_key.
        let mut app = app_with_one_row();
        app.show_help = true;
        handle_key(&mut app, KeyModifiers::CONTROL, KeyCode::Char('c'));
        assert!(!app.show_help, "Ctrl+C dismisses help modal");
        assert!(app.action.is_none(), "Ctrl+C on help overlay does NOT quit");
    }

    #[test]
    fn f2_while_help_shown_dismisses_help_not_swaps() {
        // Same airtightness contract for F2: must dismiss help first,
        // not punch through to the pick-tui swap. Otherwise pressing F2
        // while help is shown would leak the swap action and tear down
        // the TUI mid-help-display.
        let mut app = app_with_one_row();
        app.show_help = true;
        handle_key(&mut app, KeyModifiers::NONE, KeyCode::F(2));
        assert!(!app.show_help, "F2 dismisses help modal");
        assert!(
            app.action.is_none(),
            "F2 on help overlay does NOT fire SwapToPickTui"
        );
    }

    #[test]
    fn help_text_documents_every_actionable_key() {
        // If someone removes a documented key's mention from help_lines,
        // this test fires. It does NOT enforce the inverse direction
        // (adding a brand-new handler still requires updating the needle
        // list below) — that's a harder coupling to automate without
        // parsing handle_key's source.
        let body: String = help_lines()
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.to_string()))
            .collect::<Vec<_>>()
            .join(" ");
        for needle in [
            "Enter",
            "Esc",
            "F2",
            "C-c",
            "C-l",
            "y",
            "o",
            "g",
            "s",
            "Backspace",
            "drill",
            "yank",
            "ssh",
        ] {
            assert!(
                body.contains(needle),
                "help panel must document `{needle}` — got:\n{body}"
            );
        }
    }

    #[test]
    fn help_overlay_actually_renders_title_when_show_help_is_true() {
        // Guards against a refactor that removes the
        // `if app.show_help { draw_help_overlay(...) }` branch in draw():
        // state-machine tests would still pass (show_help toggles
        // correctly) but the user would see an invisible modal. Drive
        // draw() via TestBackend and assert the help title is painted.
        use ratatui::backend::TestBackend;
        let mut app = app_with_one_row();
        app.show_help = true;
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            rendered.contains("Help") && rendered.contains("pick-links"),
            "help overlay title must be visible when show_help=true; got buffer:\n{rendered}"
        );
    }

    #[test]
    fn help_overlay_does_not_render_when_show_help_is_false() {
        // Inverse of the above: the title must NOT appear in the rendered
        // buffer when show_help=false. Otherwise the if-branch could be
        // deleted and the overlay would always show.
        use ratatui::backend::TestBackend;
        let mut app = app_with_one_row();
        assert!(!app.show_help, "help starts hidden");
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            !rendered.contains("Help — pick-links"),
            "help overlay title must NOT be visible when show_help=false"
        );
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

    // First draw so the terminal is in its intended state, then drain any
    // queued events. Tmux popups inject terminal-state responses (cursor
    // position, device attributes, focus events) shortly after the pty comes
    // up, and crossterm's parser can treat some of these as key events. Poll
    // with a longer window than picker.rs's 1ms to catch late arrivals.
    terminal.draw(|f| draw(f, &mut app))?;
    for _ in 0..16 {
        while event::poll(std::time::Duration::from_millis(5))? {
            let _ = event::read();
        }
    }

    let result = event_loop(&mut app, &mut terminal);

    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
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
            "pick> {}_  │ Links › {}  │ ↑↓ Enter:act ←:back ?:help F2:sess",
            app.query,
            cat.display()
        )
    } else {
        format!(
            "pick> {}_  │ ↑↓ Enter:act →:drill y:yank o:open g:gh ?:help F2:sess",
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

    // Help overlay (modal): render last so it paints on top of the list/preview.
    if app.show_help {
        draw_help_overlay(f, area);
    }
}

/// Build the help overlay content. Pulled out so the exact key list is easy
/// to eyeball and keep in sync with `handle_key`.
fn help_lines() -> Vec<Line<'static>> {
    let hdr = Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let key = Style::default()
        .fg(Color::LightYellow)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::Gray);

    let kv = |k: &'static str, v: &'static str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {k:<14}"), key),
            Span::styled(v.to_string(), dim),
        ])
    };

    vec![
        Line::from(Span::styled("Navigation", hdr)),
        kv("↑ ↓ / C-p C-n", "move selection (headers selectable)"),
        kv("→ / Enter", "drill into category header"),
        kv("← / Esc", "drill out (first press), then quit"),
        kv("1 – 9", "jump into Nth non-empty category (empty query)"),
        Line::from(""),
        Line::from(Span::styled("Actions (empty query)", hdr)),
        kv("Enter", "default: yank URL / ssh server or IP"),
        kv("y", "yank canonical via OSC 52"),
        kv("o", "open / xdg-open in browser"),
        kv("g", "gh view --web (GitHub rows only)"),
        kv("s", "force ssh in new tmux window"),
        Line::from(""),
        Line::from(Span::styled("Filter", hdr)),
        kv("a – z 0 – 9", "type into filter query"),
        kv("Backspace", "delete last character"),
        kv("C-c", "clear query (or quit if empty)"),
        Line::from(""),
        Line::from(Span::styled("Display & picker swap", hdr)),
        kv("C-l", "toggle horizontal/vertical split"),
        kv("F2", "swap to rmux_helper pick-tui (session picker)"),
        kv("? / F1", "toggle this help overlay"),
        Line::from(""),
        Line::from(Span::styled("  Press any key to dismiss.", dim)),
    ]
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 80, area);
    // Clear under the popup so the list/preview don't bleed through.
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help — pick-links ")
        .style(Style::default().fg(Color::LightYellow));
    let para = Paragraph::new(help_lines())
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, popup);
}

/// Centered rect: `percent_x`/`percent_y` of the parent `r`, clamped so the
/// popup is never wider or taller than the parent. Standard ratatui idiom.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn render_item(app: &App, idx: usize) -> ListItem<'static> {
    if let Some(cat) = sentinel_to_category(idx) {
        let count = app
            .filtered
            .iter()
            .filter(|i| **i < SENTINEL_BASE && app.rows[**i].category == cat)
            .count();
        return ListItem::new(format!("⊟ {} ({count})", cat.display())).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
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
    body.as_str()
        .into_text()
        .unwrap_or_else(|_| Text::raw(body))
}

fn handle_key(app: &mut App, mods: KeyModifiers, code: KeyCode) {
    // Help overlay is modal: any key dismisses it, consuming that key so it
    // can't double as navigation, action, or query input. Must be the first
    // branch in handle_key for this to be airtight.
    if app.show_help {
        app.show_help = false;
        return;
    }

    // `?` and F1 open the help overlay. `?` is matched here (before the
    // generic ascii_graphic handler) so it doesn't type into the query.
    if matches!(code, KeyCode::Char('?') | KeyCode::F(1)) && !mods.contains(KeyModifiers::CONTROL) {
        app.show_help = true;
        return;
    }

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
                        | Category::Gist
                ) {
                    app.action = Some(Action::GhWeb(row));
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
