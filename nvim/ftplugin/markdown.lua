-- Blog link following: gf opens source file, gx opens in browser
-- Detect blog root by walking up from current file looking for back-links.json
local buf_dir = vim.fn.expand("%:p:h")
local blog_root = vim.fs.root(buf_dir, "back-links.json")

if blog_root then
	local settings_dir = os.getenv("HOME") .. "/settings/nvim/"
	local blog_links = dofile(settings_dir .. "nvim_blog_links.lua")

	local function follow_blog_link()
		local line = vim.api.nvim_get_current_line()
		local col = vim.api.nvim_win_get_cursor(0)[2]
		local link = blog_links.parse_link(line, col)
		if not link then
			return false
		end
		if link.slug == "" and link.anchor then
			-- Same-page anchor: search in current buffer
			vim.fn.search("^#+\\s.*" .. link.anchor:gsub("%-", "[- ]"), "w")
			return true
		end
		local path = blog_links.resolve(blog_root, link.slug)
		if not path then
			vim.notify("blog_links: no file for " .. link.slug, vim.log.levels.WARN)
			return true
		end
		-- Push to tag stack so Ctrl-t jumps back
		local from = { vim.fn.bufnr("%"), vim.fn.line("."), vim.fn.col("."), 0 }
		local tagname = link.slug:gsub("^/", "")
		vim.fn.settagstack(vim.fn.win_getid(), { items = { { tagname = tagname, from = from } } }, "t")
		vim.cmd("edit " .. vim.fn.fnameescape(path))
		if link.anchor then
			vim.fn.search("\\c^#+\\s.*" .. link.anchor:gsub("%-", "[- ]"), "w")
		end
		return true
	end

	vim.keymap.set("n", "gf", function()
		if not follow_blog_link() then
			pcall(vim.cmd, "normal! gf")
		end
	end, { buffer = true, desc = "Follow blog link to source file" })

	vim.keymap.set("n", "<C-]>", function()
		if not follow_blog_link() then
			vim.lsp.buf.definition()
		end
	end, { buffer = true, desc = "Follow blog link to source file" })

	vim.keymap.set("n", "gx", function()
		local line = vim.api.nvim_get_current_line()
		local col = vim.api.nvim_win_get_cursor(0)[2]
		local link = blog_links.parse_link(line, col)
		if not link then
			pcall(vim.cmd, "URLOpenUnderCursor")
			return
		end
		if link.slug == "" then
			-- Same-page anchor: jump to heading in current buffer
			if link.anchor then
				vim.fn.search("^#+\\s.*" .. link.anchor:gsub("%-", "[- ]"), "w")
			end
			return
		end
		local url = blog_links.browser_url(link.slug, link.anchor)
		vim.ui.open(url)
	end, { buffer = true, desc = "Open blog link in browser" })
end
