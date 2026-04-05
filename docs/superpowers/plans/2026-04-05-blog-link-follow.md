# Blog Link Following Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `gf` follow Jekyll blog permalink links to source markdown files, and `gx` open them in browser.

**Architecture:** Pure Lua module (`nvim_blog_links.lua`) with functions to parse links and resolve them via `back-links.json`. Thin ftplugin glue maps `gf`/`gx` for markdown buffers inside `~/blog`. Existing `gx` plugin conflicts resolved by disabling their mappings.

**Tech Stack:** Neovim Lua API, vim.json.decode, busted-style tests via plenary

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `nvim/nvim_blog_links.lua` | Create | Pure module: parse_link, resolve, browser_url, load |
| `nvim/tests/blog_links_spec.lua` | Create | Busted-style tests for all pure functions |
| `nvim/ftplugin/markdown.lua` | Modify | Add blog-specific gf/gx mappings (append to existing file) |
| `nvim/nvim_plugins.lua` | Modify | Disable gx mapping in url-open and markdown.nvim |

---

### Task 1: Pure Module — parse_link

**Files:**
- Create: `nvim/nvim_blog_links.lua`
- Create: `nvim/tests/blog_links_spec.lua`

- [ ] **Step 1: Write failing tests for parse_link**

Create `nvim/tests/blog_links_spec.lua`:

```lua
local blog_links = require("nvim.nvim_blog_links")

describe("parse_link", function()
	it("parses markdown link with slug and anchor", function()
		local line = "Check out [AI Journal](/ai-journal#diary) for details"
		-- cursor on the /ai-journal part (col is 0-indexed byte offset)
		local result = blog_links.parse_link(line, 25)
		assert.are.same({ slug = "/ai-journal", anchor = "diary" }, result)
	end)

	it("parses markdown link with slug only", function()
		local line = "See [AI Journal](/ai-journal) here"
		local result = blog_links.parse_link(line, 20)
		assert.are.same({ slug = "/ai-journal" }, result)
	end)

	it("parses bare slug with anchor", function()
		local line = "Visit /ai-journal#diary today"
		local result = blog_links.parse_link(line, 8)
		assert.are.same({ slug = "/ai-journal", anchor = "diary" }, result)
	end)

	it("parses bare slug without anchor", function()
		local line = "Visit /ai-journal today"
		local result = blog_links.parse_link(line, 8)
		assert.are.same({ slug = "/ai-journal" }, result)
	end)

	it("returns nil when cursor not on a link", function()
		local line = "No links here at all"
		local result = blog_links.parse_link(line, 5)
		assert.is_nil(result)
	end)

	it("parses same-page anchor", function()
		local line = "Jump to [section](#my-heading) above"
		local result = blog_links.parse_link(line, 20)
		assert.are.same({ slug = "", anchor = "my-heading" }, result)
	end)

	it("handles cursor at start of markdown link", function()
		local line = "[text](/foo#bar)"
		local result = blog_links.parse_link(line, 0)
		assert.are.same({ slug = "/foo", anchor = "bar" }, result)
	end)

	it("handles cursor at end of markdown link", function()
		local line = "[text](/foo#bar)"
		local result = blog_links.parse_link(line, 14)
		assert.are.same({ slug = "/foo", anchor = "bar" }, result)
	end)

	it("ignores external URLs", function()
		local line = "See [Google](https://google.com) here"
		local result = blog_links.parse_link(line, 18)
		assert.is_nil(result)
	end)
end)
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: FAIL — module `nvim.nvim_blog_links` not found.

- [ ] **Step 3: Write parse_link implementation**

Create `nvim/nvim_blog_links.lua`:

```lua
local M = {}

--- Parse a blog link from a line of text at the given cursor column.
--- @param line string The full line of text
--- @param col number 0-indexed byte offset (matches nvim_win_get_cursor)
--- @return table|nil {slug=string, anchor=string|nil} or nil if no link found
function M.parse_link(line, col)
	-- Strategy: find all markdown links [text](url) and bare /slugs in the line,
	-- then check if cursor falls within any of them.

	-- Try markdown link [text](/slug#anchor) first
	-- We need to find if cursor is anywhere inside [...](...) 
	local start = 1
	while start <= #line do
		-- Find next markdown link pattern
		local ls, le, url = line:find("%[.-%]%((.-)%)", start)
		if not ls then
			break
		end
		-- Check if cursor (0-indexed) falls within [ls-1, le-1] (convert to 0-indexed)
		if col >= ls - 1 and col <= le - 1 then
			return M._parse_url_part(url)
		end
		start = le + 1
	end

	-- Try bare /slug or /slug#anchor
	start = 1
	while start <= #line do
		local ls, le, slug_part = line:find("(/[%w_-]+#?[%w_-]*)", start)
		if not ls then
			break
		end
		if col >= ls - 1 and col <= le - 1 then
			return M._parse_url_part(slug_part)
		end
		start = le + 1
	end

	return nil
end

--- Parse a URL/path part into slug and anchor components.
--- @param url string e.g. "/ai-journal#diary" or "/ai-journal" or "#heading"
--- @return table|nil {slug=string, anchor=string|nil} or nil for external URLs
function M._parse_url_part(url)
	-- Skip external URLs
	if url:match("^https?://") then
		return nil
	end

	local slug, anchor = url:match("^(.-)#(.+)$")
	if slug then
		return { slug = slug, anchor = anchor }
	end

	-- No anchor
	if url:match("^/[%w_-]+") or url == "" then
		return { slug = url }
	end

	return nil
end

return M
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: All 9 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add nvim/nvim_blog_links.lua nvim/tests/blog_links_spec.lua
git commit -m "feat(nvim): add blog link parser with tests"
```

---

### Task 2: Pure Module — load and resolve

**Files:**
- Modify: `nvim/nvim_blog_links.lua`
- Modify: `nvim/tests/blog_links_spec.lua`

- [ ] **Step 1: Create test fixture and write failing tests**

Create fixture file `nvim/tests/fixtures/test_back_links.json`:

```json
{
  "redirects": {
    "/ai-brain": "/ai-second-brain",
    "/old-page": "/middle-redirect",
    "/middle-redirect": "/final-page"
  },
  "url_info": {
    "/ai-journal": {
      "markdown_path": "_d/ai-journal.md",
      "title": "AI Journal"
    },
    "/ai-second-brain": {
      "markdown_path": "_d/ai-second-brain.md",
      "title": "AI Second Brain"
    },
    "/final-page": {
      "markdown_path": "_d/final-page.md",
      "title": "Final Page"
    }
  }
}
```

Append to `nvim/tests/blog_links_spec.lua`:

```lua
describe("load", function()
	it("loads and parses back-links.json from blog root", function()
		-- Use the fixture directory as blog root
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		-- Rename fixture for this test
		local data = blog_links.load(fixture_root, "test_back_links.json")
		assert.is_not_nil(data)
		assert.is_not_nil(data.redirects)
		assert.is_not_nil(data.url_info)
		assert.equals("/ai-second-brain", data.redirects["/ai-brain"])
	end)

	it("returns nil for missing file", function()
		local data = blog_links.load("/nonexistent/path")
		assert.is_nil(data)
	end)

	it("caches on repeated calls", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local data1 = blog_links.load(fixture_root, "test_back_links.json")
		local data2 = blog_links.load(fixture_root, "test_back_links.json")
		assert.equals(data1, data2) -- same table reference
	end)
end)

describe("resolve", function()
	before_each(function()
		blog_links._clear_cache()
	end)

	it("resolves a direct slug to absolute path", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/ai-journal", "test_back_links.json")
		assert.equals(fixture_root .. "/_d/ai-journal.md", path)
	end)

	it("follows a single redirect", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/ai-brain", "test_back_links.json")
		assert.equals(fixture_root .. "/_d/ai-second-brain.md", path)
	end)

	it("follows a redirect chain", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/old-page", "test_back_links.json")
		assert.equals(fixture_root .. "/_d/final-page.md", path)
	end)

	it("returns nil for unknown slug", function()
		local fixture_root = vim.fn.getcwd() .. "/nvim/tests/fixtures"
		local path = blog_links.resolve(fixture_root, "/nonexistent", "test_back_links.json")
		assert.is_nil(path)
	end)
end)
```

- [ ] **Step 2: Run tests to verify new tests fail**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: New `load` and `resolve` tests FAIL, parse_link tests still PASS.

- [ ] **Step 3: Implement load, resolve, and _clear_cache**

Add to `nvim/nvim_blog_links.lua` (before `return M`):

```lua
local _cache = nil
local _cache_path = nil

--- Load and cache back-links.json from a blog root directory.
--- @param blog_root string Absolute path to blog root
--- @param filename string|nil JSON filename (default: "back-links.json")
--- @return table|nil Parsed JSON with redirects and url_info
function M.load(blog_root, filename)
	filename = filename or "back-links.json"
	local path = blog_root .. "/" .. filename
	if _cache and _cache_path == path then
		return _cache
	end
	local f = io.open(path, "r")
	if not f then
		return nil
	end
	local content = f:read("*a")
	f:close()
	local data = vim.json.decode(content)
	_cache = data
	_cache_path = path
	return data
end

--- Resolve a permalink slug to an absolute markdown file path.
--- Follows redirect chains up to 5 hops.
--- @param blog_root string Absolute path to blog root
--- @param slug string Permalink slug (e.g. "/ai-journal")
--- @param filename string|nil JSON filename for testing
--- @return string|nil Absolute path to markdown file
function M.resolve(blog_root, slug, filename)
	local data = M.load(blog_root, filename)
	if not data then
		return nil
	end

	-- Follow redirect chain (max 5 hops)
	local target = slug
	for _ = 1, 5 do
		local redirect = data.redirects[target]
		if not redirect then
			break
		end
		target = redirect
	end

	local info = data.url_info[target]
	if info and info.markdown_path then
		return blog_root .. "/" .. info.markdown_path
	end
	return nil
end

--- Clear the cache (for testing).
function M._clear_cache()
	_cache = nil
	_cache_path = nil
end
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add nvim/nvim_blog_links.lua nvim/tests/blog_links_spec.lua nvim/tests/fixtures/test_back_links.json
git commit -m "feat(nvim): add back-links.json loader and slug resolver with tests"
```

---

### Task 3: Pure Module — browser_url

**Files:**
- Modify: `nvim/nvim_blog_links.lua`
- Modify: `nvim/tests/blog_links_spec.lua`

- [ ] **Step 1: Write failing tests for browser_url**

Append to `nvim/tests/blog_links_spec.lua`:

```lua
describe("browser_url", function()
	it("builds URL with slug only", function()
		local url = blog_links.browser_url("/ai-journal")
		assert.equals("https://idvork.in/ai-journal", url)
	end)

	it("builds URL with slug and anchor", function()
		local url = blog_links.browser_url("/ai-journal", "diary")
		assert.equals("https://idvork.in/ai-journal#diary", url)
	end)

	it("handles nil anchor", function()
		local url = blog_links.browser_url("/ai-journal", nil)
		assert.equals("https://idvork.in/ai-journal", url)
	end)
end)
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: browser_url tests FAIL.

- [ ] **Step 3: Implement browser_url**

Add to `nvim/nvim_blog_links.lua` (before `return M`):

```lua
--- Build a browser URL for a blog permalink.
--- @param slug string Permalink slug (e.g. "/ai-journal")
--- @param anchor string|nil Optional anchor (without #)
--- @return string Full URL
function M.browser_url(slug, anchor)
	local url = "https://idvork.in" .. slug
	if anchor then
		url = url .. "#" .. anchor
	end
	return url
end
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add nvim/nvim_blog_links.lua nvim/tests/blog_links_spec.lua
git commit -m "feat(nvim): add browser_url builder with tests"
```

---

### Task 4: Disable conflicting gx plugin mappings

**Files:**
- Modify: `nvim/nvim_plugins.lua:31-38` (url-open keys)
- Modify: `nvim/nvim_plugins.lua:850-851` (markdown.nvim link_follow)

- [ ] **Step 1: Remove gx from url-open keys table**

In `nvim/nvim_plugins.lua`, change the url-open plugin entry. Remove the `keys` table so the plugin loads on command only (our ftplugin handles `gx`):

Before (lines 31-38):
```lua
	{
		"sontungexpt/url-open",
		cmd = "URLOpenUnderCursor",
		keys = {
			{
				"gx",
				"<cmd>URLOpenUnderCursor<cr>",
				desc = "Open URL",
			},
		},
```

After:
```lua
	{
		"sontungexpt/url-open",
		cmd = "URLOpenUnderCursor",
```

- [ ] **Step 2: Disable link_follow in markdown.nvim**

In `nvim/nvim_plugins.lua`, change the markdown.nvim opts. Change `link_follow = "gx"` to `link_follow = false`:

Before (lines 850-851):
```lua
			mappings = {
				link_follow = "gx", -- Use gx to follow markdown links
			},
```

After:
```lua
			mappings = {
				link_follow = false, -- Handled by blog_links ftplugin
			},
```

- [ ] **Step 3: Verify nvim starts cleanly**

```bash
nvim --headless -c "qa" 2>&1
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add nvim/nvim_plugins.lua
git commit -m "fix(nvim): disable gx mappings from url-open and markdown.nvim

blog_links ftplugin will handle gx for blog files, falling back to
URLOpenUnderCursor for non-blog links."
```

---

### Task 5: ftplugin glue — gf and gx mappings

**Files:**
- Modify: `nvim/ftplugin/markdown.lua`

- [ ] **Step 1: Read existing ftplugin/markdown.lua**

```bash
cat nvim/ftplugin/markdown.lua
```

Understand what's already there before appending.

- [ ] **Step 2: Add blog link mappings to ftplugin/markdown.lua**

Append to existing `nvim/ftplugin/markdown.lua`:

```lua
-- Blog link following: gf opens source file, gx opens in browser
local blog_root = vim.fn.expand("~/blog")
local buf_path = vim.api.nvim_buf_get_name(0)

if vim.startswith(buf_path, blog_root) then
	local blog_links = require("nvim_blog_links")

	vim.keymap.set("n", "gf", function()
		local line = vim.api.nvim_get_current_line()
		local col = vim.api.nvim_win_get_cursor(0)[2]
		local link = blog_links.parse_link(line, col)
		if not link then
			-- Fall back to built-in gf
			vim.cmd("normal! gf")
			return
		end
		if link.slug == "" and link.anchor then
			-- Same-page anchor: search in current buffer
			vim.fn.search("^#+\\s.*" .. link.anchor:gsub("%-", "[- ]"), "w")
			return
		end
		local path = blog_links.resolve(blog_root, link.slug)
		if not path then
			vim.notify("blog_links: no file for " .. link.slug, vim.log.levels.WARN)
			return
		end
		vim.cmd("edit " .. vim.fn.fnameescape(path))
		if link.anchor then
			-- Search for heading matching anchor (Jekyll: lowercase, spaces→dashes)
			vim.fn.search("^#+\\s.*" .. link.anchor:gsub("%-", "[- ]"), "wi")
		end
	end, { buffer = true, desc = "Follow blog link to source file" })

	vim.keymap.set("n", "gx", function()
		local line = vim.api.nvim_get_current_line()
		local col = vim.api.nvim_win_get_cursor(0)[2]
		local link = blog_links.parse_link(line, col)
		if not link then
			-- Fall back to url-open for non-blog links
			vim.cmd("URLOpenUnderCursor")
			return
		end
		local url = blog_links.browser_url(link.slug, link.anchor)
		vim.ui.open(url)
	end, { buffer = true, desc = "Open blog link in browser" })
end
```

- [ ] **Step 3: Verify nvim loads markdown file without errors**

```bash
nvim --headless -c "edit /tmp/test.md" -c "lua print('ftplugin loaded ok')" -c "qa" 2>&1
```

Expected: "ftplugin loaded ok", no errors.

- [ ] **Step 4: Commit**

```bash
git add nvim/ftplugin/markdown.lua
git commit -m "feat(nvim): add gf/gx blog link following in markdown ftplugin

gf resolves Jekyll permalink slugs via back-links.json and opens the
source markdown file. gx opens the link in the browser. Only activates
for files inside ~/blog."
```

---

### Task 6: Manual integration test

**Files:** None (verification only)

- [ ] **Step 1: Run all automated tests**

```bash
cd ~/settings && nvim --headless -u nvim/min_test_init.lua -c "PlenaryBustedFile nvim/tests/blog_links_spec.lua" 2>&1
```

Expected: All tests PASS.

- [ ] **Step 2: Manual test gf in blog**

Open a blog file with a known link and test `gf`:

```bash
nvim ~/blog/_d/ai-journal.md
```

1. Find a line with a link like `[text](/ai-coder)`
2. Place cursor on the link
3. Press `gf`
4. Verify it opens `_d/ai-coder.md` (or whichever file the slug maps to)

- [ ] **Step 3: Manual test gx in blog**

1. In the same blog file, place cursor on a link
2. Press `gx`
3. Verify browser opens `https://idvork.in/<slug>`

- [ ] **Step 4: Manual test fallback outside blog**

```bash
nvim /tmp/test.md
```

1. Add a line with a URL like `https://google.com`
2. Press `gx` — should open in browser (url-open fallback)
3. Press `gf` on a filename — should use built-in gf

- [ ] **Step 5: Run pre-commit checks**

```bash
pre-commit run --all-files
```

Expected: All checks pass.
