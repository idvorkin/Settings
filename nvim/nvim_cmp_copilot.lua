-- Setup Completers (CMP) including LSPs Source and Copilot source

local lspkind = require("lspkind")
--
-- [[
local _ = [[
npm i -g pyright
brew install lua-language-server
brew install marksman
npm install -g typescript typescript-language-server
-- ]]

-- Hymn, now I can use LSP install, which is cool.

require("mason").setup({})
require("mason-lspconfig").setup({
	handlers = {
		require("lsp-zero").default_setup,
	},
})

-- Setup LSP Config using new Neovim 0.11+ API
-- Configure LSPs
vim.lsp.config.racket_langserver = {}
vim.lsp.config.pyright = {}
vim.lsp.config.ts_ls = {}
vim.lsp.config.zls = {}

vim.lsp.config.typos_lsp = {
	-- Logging level of the language server. Logs appear in :LspLog. Defaults to error.
	cmd_env = { RUST_LOG = "error" },
	init_options = {
		-- Custom config. Used together with a config file found in the workspace or its parents,
		-- taking precedence for settings declared in both.
		-- Equivalent to the typos `--config` cli argument.
		config = "~/code/typos-lsp/crates/typos-lsp/tests/typos.toml",
		-- How typos are rendered in the editor, can be one of an Error, Warning, Info or Hint.
		-- Defaults to error.
		diagnosticSeverity = "Error",
	},
}

-- Enable debug logs for the LSP client. Recommended for debugging only.
vim.lsp.set_log_level("warn")

local ruff_on_attach = function(client, _)
	-- Disable hover in favor of Pyright
	client.server_capabilities.hoverProvider = false
end

vim.lsp.config.ruff = {
	on_attach = ruff_on_attach,
	init_options = {
		settings = {
			-- Any extra CLI arguments for `ruff` go here.
			args = {},
		},
	},
}

vim.lsp.config.lua_ls = {
	settings = {
		Lua = {
			runtime = {
				-- Tell the language server which version of Lua you're using
				-- (most likely LuaJIT in the case of Neovim)
				version = "LuaJIT",
			},
			diagnostics = {
				-- Get the language server to recognize the `vim` global
				globals = {
					"vim",
					"require",
				},
			},
			workspace = {
				-- Make the server aware of Neovim runtime files
				library = vim.api.nvim_get_runtime_file("", true),
				checkThirdParty = false,
			},
			-- Do not send telemetry data containing a randomized but unique identifier
			telemetry = {
				enable = false,
			},
		},
	},
}

vim.lsp.config.marksman = {}

-- Enable all configured LSPs
vim.lsp.enable("racket_langserver")
vim.lsp.enable("pyright")
vim.lsp.enable("ts_ls")
vim.lsp.enable("zls")
vim.lsp.enable("typos_lsp")
vim.lsp.enable("ruff")
vim.lsp.enable("lua_ls")
vim.lsp.enable("marksman")

-- Global mappings.
-- See `:help vim.diagnostic.*` for documentation on any of the below functions
vim.keymap.set("n", "<space>e", vim.diagnostic.open_float)
vim.keymap.set("n", "[d", vim.diagnostic.goto_prev)
vim.keymap.set("n", "]d", vim.diagnostic.goto_next)
vim.keymap.set("n", "<space>q", vim.diagnostic.setloclist)
-- K for hover

local telescope_builtin = require("telescope.builtin")

-- Use LspAttach autocommand to only map the following keys
-- after the language server attaches to the current buffer
vim.api.nvim_create_autocmd("LspAttach", {
	group = vim.api.nvim_create_augroup("UserLspConfig", {}),
	callback = function(ev)
		-- Enable completion triggered by <c-x><c-o>
		vim.bo[ev.buf].omnifunc = "v:lua.vim.lsp.omnifunc"

		-- Buffer local mappings.
		-- See `:help vim.lsp.*` for documentation on any of the below functions
		local opts = { buffer = ev.buf }
		vim.keymap.set("n", "gD", vim.lsp.buf.declaration, opts)
		vim.keymap.set("n", "gd", vim.lsp.buf.definition, opts)
		vim.keymap.set("n", "K", vim.lsp.buf.hover, opts)
		vim.keymap.set("n", "gi", vim.lsp.buf.implementation, opts)
		vim.keymap.set("n", "<C-k>", vim.lsp.buf.signature_help, opts)
		vim.keymap.set("n", "<space>wa", vim.lsp.buf.add_workspace_folder, opts)
		vim.keymap.set("n", "<space>wr", vim.lsp.buf.remove_workspace_folder, opts)
		vim.keymap.set("n", "<space>wl", function()
			print(vim.inspect(vim.lsp.buf.list_workspace_folders()))
		end, opts)
		vim.keymap.set("n", "<space>D", vim.lsp.buf.type_definition, opts)
		vim.keymap.set("n", "<space>gt", vim.lsp.buf.type_definition, opts)
		--vim.keymap.set('n', '<space>rn', vim.lsp.buf.rename, opts)
		-- vim.keymap.set('n', '<space>rn', ":IncRename")
		vim.keymap.set("n", "<space>rn", function()
			return ":IncRename " .. vim.fn.expand("<cword>")
		end, { expr = true })

		vim.keymap.set({ "n", "v" }, "<space>ca", vim.lsp.buf.code_action, opts)
		vim.keymap.set("n", "gr", vim.lsp.buf.references, opts)
		vim.keymap.set("n", "gR", telescope_builtin.lsp_references, opts)
		vim.keymap.set("n", "<space>ai", require("lspimport").import, { noremap = true }) -- OMG Such a PITA w/o this!
		vim.keymap.set("n", "<space>f", function()
			vim.lsp.buf.format({ async = true })
		end, opts)
	end,
})

-- Map :Errors to the following <cmd>Trouble diagnostics toggle filter.buf=0<cr>"
vim.cmd("command! Errors Trouble diagnostics toggle filter.buf=0")

local has_words_before = function()
	if vim.api.nvim_buf_get_option(0, "buftype") == "prompt" then
		return false
	end
	local line, col = unpack(vim.api.nvim_win_get_cursor(0))
	return col ~= 0 and vim.api.nvim_buf_get_text(0, line - 1, 0, line - 1, col, {})[1]:match("^%s*$") == nil
end

local cmp = require("cmp")
cmp.setup({
	window = {
		completion = cmp.config.window.bordered(),
		documentation = cmp.config.window.bordered(),
	},
	sources = cmp.config.sources({
		{ name = "copilot" },
		{ name = "nvim_lsp" },
		{ name = "luasnip" },
		snippet = {
			expand = function(args)
				require("luasnip").lsp_expand(args.body)
			end,
		},
		{ name = "conjure" },
		--       {name = 'buffer'},  Exclude buffer as it's pretty noisy
	}),
	mapping = cmp.mapping.preset.insert({
		["<C-b>"] = cmp.mapping.scroll_docs(-4),
		["<C-f>"] = cmp.mapping.scroll_docs(4),
		["<C-k>"] = cmp.mapping.complete(),
		["<C-e>"] = cmp.mapping.abort(),
		["<CR>"] = cmp.mapping.confirm({ select = true }), -- Accept currently selected item. Set `select` to `false` to only confirm explicitly selected items.
		["<Tab>"] = vim.schedule_wrap(function(fallback)
			if cmp.visible() and has_words_before() then
				cmp.select_next_item({ behavior = cmp.SelectBehavior.Select })
			else
				fallback()
			end
		end),
	}),

	formatting = {
		format = lspkind.cmp_format({
			mode = "symbol", -- show only symbol annotations
			maxwidth = 50, -- prevent the popup from showing more than provided characters (e.g 50 will not show more than 50 characters)
			ellipsis_char = "...", -- when popup menu exceed maxwidth, the truncated part would show ellipsis_char instead (must define maxwidth first)
		}),
	},
})

-- `/` cmdline setup.
cmp.setup.cmdline("/", {
	mapping = cmp.mapping.preset.cmdline(),
	sources = {
		{ name = "buffer" },
	},
})
-- `:` cmdline setup.
cmp.setup.cmdline(":", {
	mapping = cmp.mapping.preset.cmdline(),
	sources = cmp.config.sources({
		{ name = "path" },
	}, {
		{ name = "cmdline" },
	}),
})

-- https://github.com/petertriho/cmp-git
-- Lets us use # and  : in git commits
cmp.setup.filetype("gitcommit", {
	sources = cmp.config.sources({
		{ name = "git" },
	}, {
		{ name = "buffer" },
	}),
})

require("cmp_git").setup()

require("copilot").setup({
	auto_trigger = true,
	panel = {
		enabled = false,
		auto_refresh = true,
	},
	suggestion = {
		enabled = false,
		auto_trigger = true,
		keymap = {
			accept = "<tab>",
			next = "<C-j>",
			prev = "<C-k>",
		},
	},
	filetypes = {
		markdown = false,
	},
})

require("copilot_cmp").setup()
