-- Setup Packe
local settings_dir = os.getenv("HOME") .. "/settings/nvim/"
dofile(settings_dir .. "nvim_shared.lua")
if vim.g.vscode then
	print("Running in vscode-neovim")
end
dofile(settings_dir .. "nvim_plugins.lua")

require("aerial").setup({
	placement = "edge",
	layout = {
		default_direction = "left",
	},
	-- optionally use on_attach to set keymaps when aerial has attached to a buffer
	on_attach = function(bufnr)
		-- Jump forwards/backwards with '{' and '}'
		vim.keymap.set("n", "{", "<cmd>AerialPrev<CR>", { buffer = bufnr })
		vim.keymap.set("n", "}", "<cmd>AerialNext<CR>", { buffer = bufnr })
		vim.keymap.set("n", "<C-y>", "<cmd>AerialToggle<CR>")
	end,
})

require("telescope").load_extension("aerial")

-- Lets keep exra lua stuff here
require("nvim-web-devicons").get_icons()

--[[ Setup Tree Sitter
Debug w/ :TSInstallInfo
See: https://github.com/MDeiml/tree-sitter-markdown/issues/121

Install Plugins
    :TSInstall markdown_inline
    :TSInstall yaml

]]

-- Bleh lets debug highlighting
-- For VIM hightlights
vim.cmd([[
    function! HighlightGroup()
        echo synIDattr(synID(line("."), col("."), 1), "name")
    endfunction
    function! HighlightLinks(group)
        redir => links
        silent highlight
        redir END
        return filter(split(links, '\n'), 'v:val =~ "^' . a:group . '\s\+xxx"')
    endfunction

    " Fix highlighting for markdown
    " highlight link @text.emphasis htmlItalic
    " highlight link @text.strong htmlBold

]])
-- For TS Highlights
--  :TSHighlightCapturesUnderCursor

require("nvim-treesitter.configs").setup({
	-- A list of parser names, or "all" (the five listed parsers should always be installed)
	ensure_installed = { "c", "lua", "vim", "vimdoc", "query", "markdown_inline", "python", "javascript", "regex" },

	-- Install parsers synchronously (only applied to `ensure_installed`)
	sync_install = false,

	-- Automatically install missing parsers when entering buffer
	-- Recommendation: set to false if you don't have `tree-sitter` CLI installed locally
	auto_install = true,

	highlight = {
		enable = true,
		-- Setting this to true will run `:h syntax` and tree-sitter at the same time.
		-- Set this to `true` if you depend on 'syntax' being enabled (like for indentation).
		-- Using this option may slow down your editor, and you may see some duplicate highlights.
		-- Instead of true it can also be a list of languages
		-- additional_vim_regex_highlighting = {'markdown','markdown_inline'},
		additional_vim_regex_highlighting = false,
	},
	textobjects = {
		select = {
			enable = true,
			lookahead = true,
			keymaps = {
				["af"] = "@function.outer",
				["if"] = "@function.inner",
			},
		},
	},
})

dofile(settings_dir .. "nvim_cmp_copilot.lua")
dofile(settings_dir .. "nvim_git.lua")
dofile(settings_dir .. "nvim_color.lua")

vim.cmd([[
command! -nargs=* ZMT lua ZenModeToggleFunction(<f-args>)
]])

function ZenModeToggleFunction(width)
	if width == nil or width == "" then
		width = "66"
	end
	local width_percentage = tonumber(width) / 100
	require("zen-mode").toggle({
		window = {
			width = width_percentage,
		},
	})
end

local nvim_logic = dofile(settings_dir .. "nvim_logic.lua")

function InsertYouTubeTemplate(url)
	if not url then
		url = vim.fn.getreg("+")
	end
	local template, err = nvim_logic.parse_youtube_url(url)
	if template then
		vim.api.nvim_put({ template }, "c", true, true)
	else
		vim.api.nvim_put({ err }, "c", true, true)
	end
end

print("nvim_init.lua loaded")
