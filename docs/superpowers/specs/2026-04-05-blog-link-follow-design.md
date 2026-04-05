# Blog Link Following in Neovim

## Problem

When editing Jekyll blog markdown in `~/blog`, links like `[text](/ai-journ#boop)` can't be followed. Pressing `gf` fails because neovim doesn't know how to resolve permalink slugs to source files. Pressing `gx` doesn't open the correct blog URL.

## Desired Behavior

- `gf` on `/ai-journal#boop` → opens `~/blog/_d/ai-journal.md`, jumps to `#boop` heading
- `gx` on `/ai-journal#boop` → opens `https://idvork.in/ai-journal#boop` in browser

## Approach

Pure Lua implementation using `back-links.json` for permalink → file resolution. Cache the JSON on first use. Test pure functions with busted-style specs (matching existing `nvim/tests/` convention).

## Architecture

```
nvim/nvim_blog_links.lua             — pure module, no neovim side effects
nvim/ftplugin/markdown.lua           — thin glue: gf/gx mappings for markdown
nvim/tests/blog_links_spec.lua       — busted-style tests (matches existing convention)
```

## Plugin Conflicts

The following existing plugins also map `gx` and must be reconfigured:

- `sontungexpt/url-open` — remove `gx` from its `keys` table (blog_links handles blog URLs, url-open handles the rest as fallback)
- `tadmccorkle/markdown.nvim` — set `mappings = { link_follow = false }` so it doesn't compete

The `gf` mapping has no conflicts (no existing plugin overrides it for markdown).

## Module: `blog_links.lua`

Pure functions with no neovim UI side effects.

### `M.load(blog_root) → data | nil`

- Reads `<blog_root>/back-links.json`
- Caches result keyed by path (only reads file once per session)
- Returns parsed JSON with `redirects` and `url_info` tables

### `M.parse_link(line, col) → { slug, anchor } | nil`

- Parses markdown link at cursor position
- `col` is 0-indexed byte offset (matches `vim.api.nvim_win_get_cursor()` convention)
- Handles: `[text](/slug#anchor)`, `[text](/slug)`, bare `/slug#anchor`, bare `/slug`
- Returns table with `slug` (string, includes leading `/`, e.g. `"/ai-journal"`) and optional `anchor` (string without `#`, e.g. `"boop"`)
- Returns nil if cursor is not on a recognizable blog link

### `M.resolve(blog_root, slug) → absolute_path | nil`

- Loads back-links.json via `M.load()`
- Follows redirect chains: checks `data.redirects[slug]`, then follows up to 5 hops to handle chained redirects
- Looks up `data.url_info[target].markdown_path`
- Returns absolute path: `blog_root .. "/" .. markdown_path`
- Returns nil if slug not found

### `M.browser_url(slug, anchor) → url_string`

- Returns `"https://idvork.in" .. slug` with optional `"#" .. anchor`
- Domain is intentionally hardcoded to production (no localhost override)

## Glue: `ftplugin/markdown.lua`

Only activates when buffer path starts with `~/blog` (expanded via `vim.fn.expand("~/blog")` to absolute path before comparison). Sets buffer-local mappings:

### `gf` mapping

1. Get current line and cursor column
2. Call `parse_link()` to extract slug and anchor
3. If no link found, fall back to built-in `gf` (via `vim.cmd("normal! gf")`)
4. Call `resolve()` to get file path
5. If not found, `vim.notify()` warning and return
6. `:edit <path>`
7. If anchor exists, search for heading matching anchor: Jekyll generates anchors by lowercasing heading text, replacing spaces with `-`, and stripping punctuation. Search pattern: `^#+\s.*` case-insensitive match where `-` in anchor maps back to whitespace/punctuation in heading text

### `gx` mapping

1. Get current line and cursor column
2. Call `parse_link()` to extract slug and anchor
3. If no blog link found, fall back to default `gx` behavior
4. Call `browser_url()` and open with `vim.ui.open()`

## back-links.json Structure

```json
{
  "redirects": { "/ai-brain": "/ai-second-brain", ... },
  "url_info": {
    "/ai-journal": {
      "markdown_path": "_d/ai-journal.md",
      "title": "Igor's AI Journal",
      ...
    }
  }
}
```

- `redirects`: old slug → canonical slug (339 entries)
- `url_info`: canonical slug → metadata including `markdown_path` (334 entries)
- `markdown_path` is relative to blog root (e.g. `_d/ai-journal.md`, `_posts/2018-01-04-7-habits.md`)

## Testing

Tests follow existing convention in `nvim/tests/` — busted-style `describe`/`it` blocks using `require("nvim.nvim_blog_links")`.

Run: `nvim -l nvim/tests/blog_links_spec.lua`

Tests use a small fixture JSON (no real blog dependency):

- `parse_link`: markdown link, bare slug, with/without anchor, cursor at different positions, edge cases
- `resolve`: direct lookup, redirect following, redirect chains, missing slug
- `browser_url`: with/without anchor

## Edge Cases

| Case | Behavior |
|------|----------|
| Link not in back-links.json | Notify user, no action |
| Cursor not on a link | `gf`: fall back to built-in `gf`. `gx`: fall back to default |
| Anchor heading not found | Open file, cursor at top |
| Same-page anchor `#boop` | Search in current buffer |
| External URL `https://...` | `gx`: default browser open |
| Non-blog markdown file | Mappings don't activate |
| Stale back-links.json | Cache persists for session; restart nvim to refresh |
