-- Snippets via mini.snippets (LSP snippet format)
-- Reload with \\s — see nvim_plugins.lua
--
-- Keymaps (set in nvim_init.lua mini.snippets setup):
--   i <C-k>  expand snippet under cursor
--   i,s <C-j>  jump to next placeholder
--   i,s <C-l>  jump to previous placeholder

local MiniSnippets = require("mini.snippets")

MiniSnippets.setup({
	snippets = {
		-- All filetypes
		{ prefix = "today", body = "$CURRENT_YEAR-$CURRENT_MONTH-$CURRENT_DATE", desc = "Today's date" },

		-- Lua
		function()
			if vim.bo.filetype ~= "lua" then
				return
			end
			return {
				{
					prefix = "lf",
					body = "local function $1($2)\n\t$3\nend",
					desc = "local function",
				},
			}
		end,

		-- Markdown
		function()
			if vim.bo.filetype ~= "markdown" then
				return
			end
			return {
				{
					prefix = "esummarize",
					body = '{%include summarize-page.html src="/$1"%}',
					desc = "Jekyll summarize include",
				},
			}
		end,
	},
	mappings = {
		expand = "<C-k>",
		jump_next = "<C-j>",
		jump_prev = "<C-l>",
		stop = "<C-c>",
	},
})
