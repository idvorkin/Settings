local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not vim.loop.fs_stat(lazypath) then
	vim.fn.system({
		"git",
		"clone",
		"--filter=blob:none",
		"https://github.com/folke/lazy.nvim.git",
		"--branch=stable", -- latest stable release
		lazypath,
	})
end
vim.opt.rtp:prepend(lazypath)

local function wordcount()
	return tostring(vim.fn.wordcount().words) .. " words"
end

local function is_markdown()
	return vim.bo.filetype == "markdown" or vim.bo.filetype == "asciidoc"
end

local function appendTables(t1, t2)
	for i = 1, #t2 do
		t1[#t1 + 1] = t2[i]
	end
	return t1
end

local vscode_compatible_plugins = {
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
		config = function()
			local status_ok, url_open = pcall(require, "url-open")
			if not status_ok then
				return
			end
			url_open.setup({})
		end,

		opts = {},
	},
	-- Comment \cc
	-- Uncomment \cu
	"scrooloose/nerdcommenter",
	"dhruvasagar/vim-table-mode",
	-- :Rename a file
	"danro/rename.vim",
	-- Comment \cc
	-- Uncomment \cu
	"scrooloose/nerdcommenter",

	"panozzaj/vim-autocorrect",
	{
		"linux-cultist/venv-selector.nvim",
		branch = "regexp",
		opts = {},
	},
}
local plugins = {
	-- Highlight current line
	-- ConoLineEnable (Highlight current line)
	"miyakogi/conoline.vim",
	-- Like LimeLight
	{
		"folke/twilight.nvim",
		opts = {
			context = 5, -- amount of lines we will try to show around the current line
		},
	},
	"ekalinin/Dockerfile.vim",
	"terrastruct/d2-vim",
	"voldikss/vim-floaterm",
	"HiPhish/rainbow-delimiters.nvim",

	"rking/ag.vim",
	"junegunn/limelight.vim",
	"junegunn/goyo.vim",
	"reedes/vim-pencil",
	"catppuccin/nvim",

	"folke/zen-mode.nvim",

	"nvim-lua/plenary.nvim",
	-- :Lua <lua code>
	-- :LuaPad
	"rafcamlet/nvim-luapad",
	{ "echasnovski/mini.nvim", version = "*" },
	{
		"nvim-lualine/lualine.nvim",
		opts = {
			sections = {
				lualine_z = {
					"aerial",
					-- By default, Z is line:column, I don't mind line, but column is too noisy
					{ wordcount, cond = is_markdown },
				},
			},
		},
	},
	{
		"nvim-neotest/neotest",
		dependencies = {
			"nvim-neotest/nvim-nio",
			"nvim-neotest/neotest-plenary",
			"nvim-neotest/neotest-python",
			"nvim-lua/plenary.nvim",
			"antoinemadec/FixCursorHold.nvim",
			"nvim-treesitter/nvim-treesitter",
		},
	},
	{
		"folke/noice.nvim",
		event = "VeryLazy",
		opts = {
			-- add any options here
			messages = {
				-- NOTE: If you enable messages, then the cmdline is enabled automatically.
				-- This is a current Neovim limitation.
				enabled = false, -- enables the Noice messages UI
			},
		},
		dependencies = {
			-- if you lazy-load any plugin below, make sure to add proper `module="..."` entries
			"MunifTanjim/nui.nvim",
			-- OPTIONAL:
			--   `nvim-notify` is only needed, if you want to use the notification view.
			--   If not available, we use `mini` as the fallback
			"rcarriga/nvim-notify",
		},
	},
	{
		"cameron-wags/rainbow_csv.nvim",
		config = true,
		ft = {
			"csv",
			"tsv",
			"csv_semicolon",
			"csv_whitespace",
			"csv_pipe",
			"rfc_csv",
			"rfc_semicolon",
		},
		cmd = {
			"RainbowDelim",
			"RainbowDelimSimple",
			"RainbowDelimQuoted",
			"RainbowMultiDelim",
		},
	},
	"nvim-tree/nvim-web-devicons",
	"dstein64/vim-startuptime",
	"folke/lazydev.nvim",
	{
		"folke/trouble.nvim",
		opts = {}, -- for default options, refer to the configuration section for custom setup.
		cmd = "Trouble",
		keys = {
			{
				"<leader>xx",
				"<cmd>Trouble diagnostics toggle<cr>",
				desc = "Diagnostics (Trouble)",
			},
			{
				"<leader>xX",
				"<cmd>Trouble diagnostics toggle filter.buf=0<cr>",
				desc = "Buffer Diagnostics (Trouble)",
			},
			{
				"<leader>cs",
				"<cmd>Trouble symbols toggle focus=false<cr>",
				desc = "Symbols (Trouble)",
			},
			{
				"<leader>cl",
				"<cmd>Trouble lsp toggle focus=false win.position=right<cr>",
				desc = "LSP Definitions / references / ... (Trouble)",
			},
			{
				"<leader>xL",
				"<cmd>Trouble loclist toggle<cr>",
				desc = "Location List (Trouble)",
			},
			{
				"<leader>xQ",
				"<cmd>Trouble qflist toggle<cr>",
				desc = "Quickfix List (Trouble)",
			},
		},
	},
	"stevearc/dressing.nvim",
	{
		"max397574/better-escape.nvim",
		opts = {
			i = {
				f = { j = "<Esc>" },
			},
		},
	},
	"stevearc/aerial.nvim",
	{
		"nvim-neo-tree/neo-tree.nvim",
		opts = {
			window = {
				mappings = {
					["u"] = "navigate_up",
				},
			},
		},
	},
	"MunifTanjim/nui.nvim",
	"godlygeek/tabular",
	{
		"AckslD/nvim-neoclip.lua",
		config = function()
			require("neoclip").setup()
		end,
	},

	"preservim/vim-colors-pencil",
	"ttibsi/pre-commit.nvim",
}

-- DB Goop
--
local sqlite_path = "/Users/idvorkin/homebrew/opt/sqlite/lib/libsqlite3.dylib"
if vim.loop.fs_stat(sqlite_path) then
	vim.g.sqlite_clib_path = sqlite_path
end
plugins = appendTables(plugins, {
	"tpope/vim-dadbod",
	"kkharji/sqlite.lua",
	-- luarocks install sqlite luv
	{
		"kristijanhusak/vim-dadbod-ui",
		dependencies = {
			{ "tpope/vim-dadbod", lazy = true },
			{ "kristijanhusak/vim-dadbod-completion", ft = { "sql", "mysql", "plsql" }, lazy = true }, -- Optional
		},
		cmd = {
			"DBUI",
			"DBUIToggle",
			"DBUIAddConnection",
			"DBUIFindBuffer",
		},
		init = function()
			-- Your DBUI configuration
			vim.g.db_ui_use_nerd_fonts = 1
		end,
	},
})

-- configure completion
vim.api.nvim_create_autocmd("FileType", {
	pattern = { "sql", "mysql", "plsql" },
	callback = function()
		require("cmp").setup.buffer({
			sources = { { name = "vim-dadbod-completion" }, { name = "buffer" } },
		})
	end,
})

local function readEulogyPrompts()
	local eulogy_prompts = vim.fn.systemlist("cat ~/gits/igor2/eulogy_prompts.md")
	if #eulogy_prompts == 0 then
		print("No prompts found.")
		return nil
	end
	math.randomseed(os.time()) -- Seed the random number generator
	local random_index = math.random(1, #eulogy_prompts)
	return eulogy_prompts[random_index]
end
-- Add dashboard
plugins = appendTables(plugins, {
	{
		"nvimdev/dashboard-nvim",
		event = "VimEnter",
		opts = function()
			require("dashboard").setup({
				theme = "hyper",
				config = {
					week_header = {
						enable = true,
					},
					--footer = {"Igor Is here"}, -- footer
					footer = { readEulogyPrompts() },
				},
				-- config
				--project = { enable = true}
			})
		end,
		dependencies = { { "nvim-tree/nvim-web-devicons" } },
	},
})

-- Read the eulogy prompts, and insert 3 random ones
-- command! PromptEulogy  :r !shuf -n 3 ~/gits/igor2/eulogy_prompts.md
--

vim.g.dashboard_command_footer = readEulogyPrompts()

-- TSPlaygroundToggle
-- :TSHighlightCapturesUnderCursor
-- :TSNodeUnderCursor
--
plugins = appendTables(plugins, {
	"tree-sitter/tree-sitter-json",
	"nvim-treesitter/playground",
	"nvim-treesitter/nvim-treesitter",
	"nvim-treesitter/nvim-treesitter-textobjects",
	"MDeiml/tree-sitter-markdown",
})

function TelescopePlugins()
	return {
		"nvim-telescope/telescope-github.nvim",
		"nvim-telescope/telescope-file-browser.nvim",
		"jvgrootveld/telescope-zoxide",
		"nvim-telescope/telescope-frecency.nvim",
		"nvim-telescope/telescope-file-browser.nvim",
		{
			"nvim-telescope/telescope-fzf-native.nvim",
			build = "cmake -S. -Bbuild -DCMAKE_BUILD_TYPE=Release && cmake --build build --config Release",
		},
		{
			"nvim-telescope/telescope.nvim",
			requires = {
				{ "nvim-lua/plenary.nvim" },
			},
		},
	}
end
plugins = appendTables(plugins, TelescopePlugins())

function ConfigureTelescopePlugins()
	local extensions = {
		"zoxide",
		"file_browser",
		"frecency",
		"gh",
	}

	for _, extension in ipairs(extensions) do
		require("telescope").load_extension(extension)
	end

	vim.cmd([[
    cab ls :Telescope buffers<CR>
    command! Gitfiles Telescope git_files
    command! GitStatus Telescope git_status
    command! Registers Telescope registers
    command! History Telescope command_history
    command! LiveGrep Telescope live_grep
    command! NeoClip Telescope neoclip
    command! Cd Telescope zoxide list
    command! Marks Telescope marks
    command! Colorscheme Telescope colorscheme
    command! JumpList Telescope jumplist
    command! LiveSearch Telescope current_buffer_fuzzy_find
    command! Z Telescope zoxide list
    command! Yazi  Telescope file_browser path=%:p:h select_buffer=true
    command! FileBrowser  Telescope file_browser path=%:p:h select_buffer=true
    command! Gist Telescope gh gist limit=20
    command! Issues Telescope gh issues
    ]])
end

-- gh keybings
-- C-T open web - All
-- Gist: C-E : Edit in another tmux tab (weird)
-- C-L Insert markdown link to issue
-- <cr> Insert reference to issue number (probably want C-L instead)

plugins = appendTables(plugins, { "tpope/vim-surround" })
--[[
     Cool does wrapping
    help surround

    Wrap current line
    ys -> you surround, motion, element
    yss* <- Wrap 'Surround' line '*'
    ds" -> delete surround
    cs" -> change surround

    Setup surround for b (old)  and i(talics) for markdown.
    echo char2nr('b') -> 105
    "
    Cheat Sheat
    " - yssX - soround the current line with italics(i) or bold(b) or something
    " else.
    "
    - Once in visual mode, S will do the surround folowed by the b so like
    select text in visual mode, then Sb will make it bold.
]]

local git_plugins = {
	"tpope/vim-fugitive",
	"lewis6991/gitsigns.nvim",
	--  DiffViewOpen
	"sindrets/diffview.nvim",
	"NeogitOrg/neogit",
	{
		"pwntester/octo.nvim",
		opts = {
			suppress_missing_scope = {
				projects_v2 = true,
			},
		},
	},
	{
		-- Configure formatter
		"stevearc/conform.nvim",
		opts = {

			formatters_by_ft = {
				lua = { "stylua" },
				-- Conform will run multiple formatters sequentially
				python = { "ruff" },
				markdown = { "prettier" },
				html = { "prettier" },
				-- Conform will run the first available formatter
				javascript = { "prettierd", "prettier", stop_after_first = true },
			},
		},
	},
	{
		"yetone/avante.nvim",
		event = "VeryLazy",
		build = "make",
		opts = {
			-- add any opts here
			behaviour = {
				auto_suggestions = false, -- Experimental stage
				auto_set_highlight_group = true,
				auto_set_keymaps = true,
				auto_apply_diff_after_generation = false,
				support_paste_from_clipboard = false,
			},
		},
		dependencies = {
			"nvim-tree/nvim-web-devicons", -- or echasnovski/mini.icons
			"stevearc/dressing.nvim",
			"nvim-lua/plenary.nvim",
			"MunifTanjim/nui.nvim",
			--- The below is optional, make sure to setup it properly if you have lazy=true
			{
				"MeanderingProgrammer/render-markdown.nvim",
				opts = {
					file_types = { "markdown", "Avante" },
				},
				ft = { "markdown", "Avante" },
			},
		},
	},
}
plugins = appendTables(plugins, git_plugins)
-- cmp and friends
local cmp_and_friends = {
	"hrsh7th/nvim-cmp",
	"hrsh7th/cmp-nvim-lsp",
	{ "stevanmilic/nvim-lspimport" },
	"hrsh7th/cmp-buffer",
	"hrsh7th/cmp-path",
	"hrsh7th/cmp-cmdline",
	"petertriho/cmp-git",
	"zbirenbaum/copilot.lua",
	"zbirenbaum/copilot-cmp",
	"neovim/nvim-lspconfig",
	"onsails/lspkind.nvim",
}

plugins = appendTables(plugins, cmp_and_friends)

-- Take these out unless I'm going back to closure
plugins = appendTables(plugins, {
	-- "wlangstroth/vim-racket",
	-- "Olical/conjure",
	-- "PaterJason/cmp-conjure",
})

local function setupMarkdown()
	return {
		-- NVIM markdown stuff,  lets see if it works with tree sitter
		-- "ixru/nvim-markdown",
		"preservim/vim-markdown",

		{
			"MeanderingProgrammer/render-markdown.nvim",
			opts = {

				heading = {
					-- Turn on / off heading icon & ba󰲧ckground rendering
					enabled = true,
					width = "block",
					icons = { "# ", "## ", "### ", "#### ", "󰲩 ", "󰲫 " },
					backgrounds = {
						"RenderMarkdownH6Bg",
						"RenderMarkdownH5Bg",
						"RenderMarkdownH4Bg",
						"RenderMarkdownH3Bg",
						"RenderMarkdownH2Bg",
						"RenderMarkdownH1Bg",
					},
					-- The 'level' is used to index into the array using a clamp
					-- Highlight for the heading and sign icons
					foregrounds = {
						"RenderMarkdownH6",
						"RenderMarkdownH5",
						"RenderMarkdownH4",
						"RenderMarkdownH3",
						"RenderMarkdownH2",
						"RenderMarkdownH1",
					},
				},
			},
		},
		{
			"iamcco/markdown-preview.nvim",
			cmd = { "MarkdownPreviewToggle", "MarkdownPreview", "MarkdownPreviewStop" },
			build = "cd app && yarn install",
			init = function()
				vim.g.mkdp_filetypes = { "markdown" }
			end,
			ft = { "markdown" },
		},
		-- Auto Update ToC
		-- "mzlogin/vim-markdown-toc",
		{
			"hedyhli/markdown-toc.nvim",
			ft = "markdown", -- Lazy load on markdown filetype
			cmd = { "Mtoc" }, -- Or, lazy load on "Mtoc" command
			opts = {
				fences = {
					enabled = true,
					-- These fence texts are wrapped within "<!-- % -->", where the '%' is
					-- substituted with the text.
					start_text = "vim-markdown-toc-start",
					end_text = "vim-markdown-toc-end",
					-- An empty line is inserted on top and below the ToC list before the being
					-- wrapped with the fence texts, same as vim-markdown-toc.
				},
				toc_list = {
					-- string or list of strings (for cycling)
					-- If cycle_markers = false and markers is a list, only the first is used.
					-- You can set to '1.' to use a automatically numbered list for ToC (if
					-- your markdown render supports it).
					markers = "-",
					cycle_markers = false,
					-- Example config for cycling markers:
					----- markers = {'*', '+', '-'},
					----- cycle_markers = true,
				},
			},
		},
	}
end

plugins = appendTables(plugins, setupMarkdown())

plugins = appendTables(plugins, {
	{ "williamboman/mason.nvim" },
	{ "williamboman/mason-lspconfig.nvim" },
	{ "VonHeikemen/lsp-zero.nvim", branch = "v3.x" },
	{ "neovim/nvim-lspconfig" },
	{ "L3MON4D3/LuaSnip" },
	{
		"smjonas/inc-rename.nvim",
		config = function()
			require("inc_rename").setup({})
		end,
	},
})

local plugins_to_load = plugins
if vim.g.vscode then
	plugins_to_load = vscode_compatible_plugins
else
	plugins_to_load = appendTables(plugins, vscode_compatible_plugins)
end
--
-- require('lspconfig').racket_langserver.setup()
--  VIM LSP  for lua - I think I still need to configure
require("lazy").setup(plugins_to_load)

ConfigureTelescopePlugins()
