local M = {}

--- Parse a blog link from a line of text at the given cursor column.
--- @param line string The full line of text
--- @param col number 0-indexed byte offset (matches nvim_win_get_cursor)
--- @return table|nil {slug=string, anchor=string|nil} or nil if no link found
function M.parse_link(line, col)
	-- Strategy: find all markdown links [text](url) and bare /slugs in the line,
	-- then check if cursor falls within any of them.

	-- Try markdown link [text](/slug#anchor) first
	local start = 1
	while start <= #line do
		local ls, le, url = line:find("%[.-%]%((.-)%)", start)
		if not ls then
			break
		end
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

return M
