# Scrollback Link Picker Design — Changelog

Spec: /home/developer/settings/docs/superpowers/specs/2026-04-12-scrollback-link-picker-design.md

## Architect Review — 2026-04-12 20:30

### Convergence Tracking

| Pass | Changes |
| ---- | ------- |
| 1    | 11      |
| 2    | 5       |
| 3    | 0       |

### Pass 1 — 2026-04-12

**Architectural edits**

1. **Tightened the "works from anywhere" claim** in Goals — it's actually "works from any tmux pane"; non-tmux terminals are explicitly out in v1.
2. **Added explicit pipeline phasing narrative** at the top of Enrichment → Pipeline: capture/parse sync, one `Runtime::new()?.block_on(enrich)` boundary, TUI never re-enters tokio. Pins the tokio runtime placement.
3. **Added concurrent-writers contract** to cache handling: last-writer-wins, no flock in v1, with rationale.
4. **Added SIGINT-during-enrichment behavior**: SIGINT works normally pre-TUI, process exits 130, `gh` children are reaped by init. No custom signal handler.
5. **Fixed filter-semantics conflict with `pick-tui`**: called out that multi-digit tokens do _not_ split per-digit here (would cause spurious PR-number matches), and marked this as a deliberate Divergence 1.
6. **Fixed category short-name tag leaking into content match**: introduced a `\x1f` unit-separator between tag and content so typing `pr` doesn't accidentally match "prose" in a context line. Marked as Divergence 2.
7. **Made F2 exec sequence concrete**: enumerated the 5-step teardown (`disable_raw_mode` → `LeaveAlternateScreen` → drop terminal → flush stdout/stderr → `exec`), added explicit "never yank on F2" rule, noted there's no runtime-shutdown step in v1 because the tokio runtime has already returned before the TUI starts.
8. **Added Unicode width invariant** for dynamic columns (CJK / emoji in enriched PR titles is real and would break naive `str::len` column math). Commits to `unicode-width` crate.
9. **Added detection-performance note**: precompiled `regex::RegexSet` single-pass over each scrollback line, not 9 independent regex walks. Makes the "works on megabyte scrollbacks" claim defensible.
10. **Added New Cargo Dependencies subsection** listing tokio, regex, serde(\_json), unicode-width, base64, dirs — none currently in the crate — with the single-thread tokio runtime choice pinned. Reconciled with File Layout's earlier feature-gate language, which was misleading.
11. **Added missing testable invariants**: filter-tag leak, digit-token substring, unicode column widths, context truncation idempotency, F2-never-yanks, SIGINT-before-TUI. Also tightened the `$TMUX_PANE` unset fallback (use `client_active_pane`, handle the pane-killed-mid-flight race) and noted the `C-a L` default-binding override.

**Reviewed but deliberately left unchanged**

- **`--json` mode in v1** — spec references it under Invariants (idempotent detection over fixture strings is cleanest to assert via JSON round-trip), so it's earning its keep as a test-harness hook even if no user would invoke it directly.
- **Glyph system for PR state** — five glyphs is small, and the color mapping lines up 1:1 with GitHub's state model. Not over-engineered.
- **Override keys `y` / `o` / `g` / `s`** — each does a semantically different thing (clipboard vs browser vs gh CLI vs force-ssh). No consolidation would improve things.
- **Single-level drill-down (no repo subgrouping)** — matches the v1 non-goals block and keeps navigation minimal. Right call.
- **Dedicated `LINK_PICKER_SPEC.md`** — mirroring `PICKER_SPEC.md` is the right pattern for this repo.
- **No frecency / no multi-select / no streaming enrichment** — all correctly parked under v2 roadmap with justification.
- **OSC 52 as the only clipboard path** — avoiding `pbcopy`/`xclip`/`wl-copy` shelling is the right call for cross-boundary (devvm → Mac host) operation. Write-ordering nuance is already captured in the body and now in invariants.

**Assessment**

After pass 1, the spec is close to implementation-ready. The pipeline seams are clean, the filter divergences from `pick-tui` are now named and justified, the F2 exec path is mechanically specified, the dep surface is called out, and the concurrency/signal edges are covered.

**Highest-priority thing still missing going into pass 2:** a concrete sketch of the top-level `pick-links` error flow (ordering of: capture → detect → enrich → TUI exit → OSC 52 flush → stdout print → process exit), shown as a single pseudocode sequence. Right now the pieces are distributed across Enrichment, Clipboard Bridge, Actions, and Error Handling, and the reader has to reconstruct the happy path and error paths mentally. A 30-line sequence would make the whole thing click and would catch any remaining ordering bugs before implementation.

### Pass 2 — 2026-04-12

**Architectural edits**

1. **Added `## Execution Flow` section** near the top of the spec (after Non-goals, before Invocation) with a ~50-line pseudocode sequence for `pick_links()` covering pane resolve → capture → detect → enrich (single `block_on`) → TUI → teardown → dispatch, plus four ordering invariants called out explicitly (OSC 52 timing, F2-no-yank, no runtime-at-F2, SIGINT-before-TUI). This is pass 1's primary ask — the ordering was previously distributed across Enrichment, Clipboard Bridge, Actions, and Error Handling, and now lives in one place as the canonical reference.
2. **Fixed stale "no live enrichment" non-goal reference** on the `g` key action. Pass 1 inverted enrichment from non-goal to goal but left the `g`-key blurb claiming GitHub-web is "consistent with the v1 non-goal of 'no live enrichment'" — that non-goal no longer exists. Rewrote the note to contrast `g` (shell out, no data parsing) with the blocking pre-TUI enrichment fan-out, which is the real distinction.
3. **Added malformed-`gh`-JSON degradation clause** to the Degradation list: schema drift, truncation, and non-UTF-8 are treated as row-level errors (fall back to context, no panic, no global abort). Previously this error class was hand-waved under "individual call errors" but JSON parse failure is distinct from HTTP failure and deserves explicit mention because `serde_json::from_slice` is the touchpoint.
4. **Added corresponding malformed-JSON invariant** under Enrichment invariants so the behavior has a test hook, not just prose.
5. **Added four UX/navigation invariants** with no prior automated coverage: empty-category-hides-header, fixed category display order, query-preserved-across-drill, and Esc-double-press-to-quit-from-drill. All four are claimed in the body of the spec but had no invariant entry, so implementations could quietly regress them.

**Reviewed but deliberately left unchanged**

- **`\x1f` unit-separator for filter tag isolation** — I considered challenging this as over-engineering (you could instead carry `tag: &str` as a side-channel next to the row's search string and match it against non-digit tokens explicitly). Rejected the simplification because the whole point of the existing `pick-tui` filter abstraction is a single flat search string per row, and introducing a side-channel for one field would fork the filter code. `\x1f` is a 1-byte insertion that preserves the substring-match invariant. Pass 1 got it right.
- **Divergence 1 / Divergence 2 wording in Filtering semantics body** — the body already introduces them by name (lines 581+). The Invariants section references them semantically ("documented divergence from `pick-tui`") without restating the number; I considered tightening that but decided it's fine — the reader who gets to Invariants has read the body.
- **File Layout vs New Cargo dependencies** — looked for a seam here (pass 1 said it had "reconciled with File Layout's earlier feature-gate language, which was misleading") and confirmed the current text is consistent: File Layout describes module structure, the Cargo deps subsection explicitly says "not gated behind a Cargo feature". No fix needed.
- **`bind-key L` override justification** — pass 1 added "is intentional — the new link picker is more useful day-to-day than 'switch to last client,' which is already reachable via other means." That's sufficient. Not adding more.
- **Concurrent-writers cache contract vs "no frecency" non-goal** — checked for contradiction per pass 2 instructions. No conflict: the cache is an enrichment-latency optimization (gh titles), not a usage history. Frecency would track user-opened links, which is a separate datastore.
- **F2 invariants vs exec teardown sequence** — checked; they mirror each other. The 5-step teardown in Cross-picker shortcut matches the three F2 invariants in the Action layer (no-yank, exec cleanliness, no runtime shutdown).
- **`--enrich-deadline-ms` default** — already explicit ("default 3000, `0` disables enrichment entirely"). No fix needed.
- **Cache format** — JSON structure is fully specified with a concrete example. No hand-wave.
- **OSC 52 byte-level shape** — fully specified in Clipboard Bridge with the exact escape sequence. No hand-wave.

**Assessment**

The spec is implementation-ready. The Execution Flow pseudocode eliminates the last major reader-reassembly burden and makes the OSC 52 / F2 ordering mechanically obvious. Invariants now have near-complete coverage of body claims. The only remaining soft edges are (a) the concurrent-writers cache contract has no direct test invariant — it's inherently hard to test without racing subprocesses — and (b) the "50ms detection budget" is explicitly soft (target, not hard deadline), which is fine but means there's no CI regression gate on detection performance.

**Suggested focus for pass 3 (if one happens):** this may be the pass to stop. Pass 1 was structural, pass 2 was consistency+flow, and the remaining gaps are either inherently hard to automate (concurrent cache write) or already explicitly soft-budgeted. Pass 3 should either (a) bless the spec as done, or (b) focus narrowly on the `cargo install --path . --force` smoke test plan — verifying the new `pick-links` subcommand integrates cleanly with `pick-tui`'s existing dispatch in `main.rs` and the F2 bidirectional binding exists in `picker.rs`. That's integration-level and might surface a real seam the design didn't anticipate.

### Pass 3 — 2026-04-12

**Mode:** A (declare done). Read the spec end-to-end cold, then verified the four Mode B integration seams against the actual `main.rs` and `picker.rs` source. All four check out.

**Architectural edits: 0 (zero).**

**Integration seams verified against source:**

1. **`main.rs` dispatch** — the clap `Subcommand` enum (L41) is a flat list; adding `PickLinks { json, enrich_deadline_ms }` and a dispatch arm at L1819 is a two-line, mechanical extension. No seam.
2. **F2 teardown at `picker.rs:686`** — the existing teardown (`disable_raw_mode` + `LeaveAlternateScreen` + return) is a direct subset of the spec's 5-step F2 sequence. F2 in the key handler can call the full teardown then `exec`, which never returns, so the existing `Ok(app.selected_target)` return path is untouched. No `Option<String>` return-shape change required, so the spec's silence on that field is correct — not a gap.
3. **Module hierarchy** — `main.rs` L1 is `mod picker;` (flat). The spec's `mod link_picker { mod.rs, detect.rs, enrich.rs, tui.rs }` is standard Rust subdirectory module form and coexists trivially with the flat `picker.rs`.
4. **Test harness** — `picker.rs:990` already has a `#[cfg(test)] mod tests` block with in-tree fixture tests. The spec's detection invariants slot into an identical block in `link_picker/detect.rs`, runnable via `cargo test -p rmux_helper detect::` (which the spec explicitly names). Not hand-wavy.

**Also verified:** `F(1)` is the only function key currently bound in `picker.rs` (L650), so `F(2)` is free. The spec correctly flags that `PICKER_SPEC.md` must be updated in the same changeset.

**End-to-end coherence check:** Execution Flow → Enrichment → Actions → Clipboard Bridge → Cross-picker shortcut → Invariants all tell the same ordering story. OSC 52 timing, F2-no-yank, no-runtime-at-F2, and SIGINT-before-TUI are stated in prose, cross-referenced in Execution Flow, and asserted as invariants — three-way coverage with no contradiction. The `\x1f` tag-separator trick, unicode-width column math, last-writer-wins cache contract, malformed-gh-JSON row-level fallback, and the four navigation invariants all have matching entries. Pipeline is coherent end-to-end.

**Assessment**

**Converged, implementation-ready.** Convergence trend 11 → 5 → 0 is clean. The pull toward making edits was weak — nothing rises to "blocking implementation." The remaining minor items (naming the future `Option<String>` → enum shift, etc.) are implementation mechanics a senior engineer will make in the first 5 minutes of coding. Recommend starting implementation rather than running pass 4.
