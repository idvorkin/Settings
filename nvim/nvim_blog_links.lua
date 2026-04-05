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

return M
