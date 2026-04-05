-- Blog link following: gf opens source file, gx opens in browser
local blog_root = vim.fn.expand("~/blog")
local buf_path = vim.api.nvim_buf_get_name(0)

if vim.startswith(buf_path, blog_root) then
	local settings_dir = os.getenv("HOME") .. "/settings/nvim/"
	local blog_links = dofile(settings_dir .. "nvim_blog_links.lua")

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
