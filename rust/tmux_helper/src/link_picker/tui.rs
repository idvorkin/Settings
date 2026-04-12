//! Ratatui TUI for the link picker. Replaced in Task 14.

use crate::link_picker::detect::Row;
use anyhow::Result;

/// Placeholder action enum — extended in Task 16.
pub enum Action {
    Quit,
}

pub fn run(_rows: Vec<Row>) -> Result<Action> {
    // Temporary: immediately return Quit so `pick-links` without `--json`
    // is still callable from tests without a TTY.
    Ok(Action::Quit)
}
