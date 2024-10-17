-- Snippets
-- OK, no clue how to make this work... project for another day.
local ls = require("luasnip")
local s = ls.snippet
local sn = ls.snippet_node
local isn = ls.indent_snippet_node
local t = ls.text_node
local i = ls.insert_node
local f = ls.function_node
local c = ls.choice_node
local d = ls.dynamic_node
local r = ls.restore_node
local events = require("luasnip.util.events")
local ai = require("luasnip.nodes.absolute_indexer")
local extras = require("luasnip.extras")
local l = extras.lambda
local rep = extras.rep
local p = extras.partial
local m = extras.match
local n = extras.nonempty
local dl = extras.dynamic_lambda
local fmt = require("luasnip.extras.fmt").fmt
local fmta = require("luasnip.extras.fmt").fmta
local conds = require("luasnip.extras.expand_conditions")
local postfix = require("luasnip.extras.postfix").postfix
local types = require("luasnip.util.types")
local parse = require("luasnip.util.parser").parse_snippet
local ms = ls.multi_snippet
local k = require("luasnip.nodes.key_indexer").new_key

ls.config.set_config({
	history = true,
	updateevents = "TextChanged,TextChangedI",
	enable_autosnippets = true,
	ext_opts = {
		[".lua"] = {
			-- Whether to use treesitter to determine the end of a snippet
			-- (used for moving out of a snippet)
			treesitter = true,
		},
	},
})

local ls = require("luasnip")

-- I don't need expand, I get it for free from comp
vim.keymap.set({ "i" }, "<C-k>", function()
	ls.expand()
end, { silent = true })
vim.keymap.set({ "i", "s" }, "<C-j>", function()
	ls.jump(1)
end, { silent = true })
vim.keymap.set({ "i", "s" }, "<C-l>", function()
	ls.jump(-1)
end, { silent = true })

vim.keymap.set({ "i", "s" }, "<C-E>", function()
	if ls.choice_active() then
		ls.change_choice(1)
	end
end, { silent = true })

local function lua_snippets()
	-- Lua-specific snippets can be added here
	-- lf = local function
	return {
		s("lf", {
			t("local function "),
			i(1),
			t("("),
			i(2),
			t(")"),
			t({ "", "\t" }),
			i(3),
			t({ "", "end" }),
		}),
	}
end

local function all_snippets()
	return {
		s("today", {
			f(function()
				return os.date("%Y-%m-%d")
			end, {}),
		}),
	}
end

ls.add_snippets("lua", lua_snippets(), { overwrite = true })
ls.add_snippets("all", all_snippets(), { overwrite = true })
