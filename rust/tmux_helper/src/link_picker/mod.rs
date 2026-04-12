//! Scrollback link picker — top-level orchestration.

pub mod detect;
pub mod enrich;
pub mod tui;

use anyhow::Result;

/// Entry point for `rmux_helper pick-links`.
pub fn pick_links(_json: bool, _enrich_deadline_ms: u64) -> Result<()> {
    // Minimal smoke: print empty JSON array.
    println!("[]");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn pick_links_json_mode_returns_empty_array_when_scrollback_is_empty() {
        // Integration-level smoke; replaced in Task 13 with real pipeline.
        assert!(true, "placeholder — replaced when pipeline lands");
    }
}
