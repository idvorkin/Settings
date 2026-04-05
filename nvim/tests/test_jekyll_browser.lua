local expect = MiniTest.expect
local new_set = MiniTest.new_set

local T = new_set()

-- Save originals to restore after each test
local orig_expand, orig_finddir, orig_fnamemodify, orig_readfile, orig_jobstart, orig_line, orig_buf_get_lines

T["OpenJekyllInBrowser"] = new_set({
	hooks = {
		pre_case = function()
			-- Save originals
			orig_expand = vim.fn.expand
			orig_finddir = vim.fn.finddir
			orig_fnamemodify = vim.fn.fnamemodify
			orig_readfile = vim.fn.readfile
			orig_jobstart = vim.fn.jobstart
			orig_line = vim.fn.line
			orig_buf_get_lines = vim.api.nvim_buf_get_lines
		end,
		post_case = function()
			-- Restore originals
			vim.fn.expand = orig_expand
			vim.fn.finddir = orig_finddir
			vim.fn.fnamemodify = orig_fnamemodify
			vim.fn.readfile = orig_readfile
			vim.fn.jobstart = orig_jobstart
			vim.fn.line = orig_line
			vim.api.nvim_buf_get_lines = orig_buf_get_lines
		end,
	},
})

-- Helper: stub vim.fn/api, capture jobstart calls
local function setup_stubs(opts)
	local jobstart_calls = {}

	vim.fn.expand = function()
		return opts.current_file
	end
	vim.fn.finddir = function()
		return opts.posts_dir or ""
	end
	vim.fn.fnamemodify = function(path, mod)
		if mod == ":h" then
			return opts.jekyll_root or ""
		elseif mod == ":~:." then
			return opts.relative_path or ""
		end
		return path
	end
	vim.fn.readfile = function()
		return opts.file_lines or {}
	end
	vim.fn.jobstart = function(cmd)
		table.insert(jobstart_calls, cmd)
		return 0
	end
	vim.fn.line = function()
		return opts.cursor_line or 1
	end
	vim.api.nvim_buf_get_lines = function()
		return opts.buf_lines or {}
	end

	return jobstart_calls
end

T["OpenJekyllInBrowser"]["opens correct URL for a Jekyll post"] = function()
	local nvim_logic = require("nvim.nvim_logic")
	local calls = setup_stubs({
		current_file = "/path/to/jekyll/_posts/2023-01-01-test-post.md",
		posts_dir = "/path/to/jekyll/_posts",
		jekyll_root = "/path/to/jekyll",
		relative_path = "_posts/2023-01-01-test-post.md",
		file_lines = { "---", "title: Test", "---" },
		buf_lines = {},
	})

	nvim_logic.OpenJekyllInBrowser()
	expect.equality(#calls, 1)
	expect.equality(calls[1], { "open", "http://localhost:4000/2023-01-01-test-post.html" })
end

T["OpenJekyllInBrowser"]["handles Jekyll posts with permalinks"] = function()
	local nvim_logic = require("nvim.nvim_logic")
	local calls = setup_stubs({
		current_file = "/path/to/jekyll/_posts/2023-01-01-test-post.md",
		posts_dir = "/path/to/jekyll/_posts",
		jekyll_root = "/path/to/jekyll",
		relative_path = "_posts/2023-01-01-test-post.md",
		file_lines = { "---", "permalink: /custom-url/", "---" },
		buf_lines = {},
	})

	nvim_logic.OpenJekyllInBrowser()
	expect.equality(#calls, 1)
	expect.equality(calls[1], { "open", "http://localhost:4000/custom-url" })
end

T["OpenJekyllInBrowser"]["handles files in special directories"] = function()
	local nvim_logic = require("nvim.nvim_logic")
	local calls = setup_stubs({
		current_file = "/path/to/jekyll/_ig66/test-file.md",
		posts_dir = "/path/to/jekyll/_posts",
		jekyll_root = "/path/to/jekyll",
		relative_path = "_ig66/test-file.md",
		file_lines = { "---", "title: Test", "---" },
		buf_lines = {},
	})

	nvim_logic.OpenJekyllInBrowser()
	expect.equality(#calls, 1)
	expect.equality(calls[1], { "open", "http://localhost:4000/ig66/test-file" })
end

T["OpenJekyllInBrowser"]["appends section anchor to URL"] = function()
	local nvim_logic = require("nvim.nvim_logic")
	local calls = setup_stubs({
		current_file = "/path/to/jekyll/_posts/2023-01-01-test-post.md",
		posts_dir = "/path/to/jekyll/_posts",
		jekyll_root = "/path/to/jekyll",
		relative_path = "_posts/2023-01-01-test-post.md",
		file_lines = { "---", "title: Test", "---" },
		cursor_line = 4,
		buf_lines = { "# Introduction", "Content", "## Section 1", "More" },
	})

	nvim_logic.OpenJekyllInBrowser()
	expect.equality(#calls, 1)
	expect.equality(calls[1], { "open", "http://localhost:4000/2023-01-01-test-post.html#section-1" })
end

T["OpenJekyllInBrowser"]["prints error when not in Jekyll project"] = function()
	local nvim_logic = require("nvim.nvim_logic")
	local printed = {}
	local orig_print = nvim_logic.print
	nvim_logic.print = function(msg)
		table.insert(printed, msg)
	end

	setup_stubs({
		current_file = "/path/to/non_jekyll/file.md",
		posts_dir = "",
	})

	nvim_logic.OpenJekyllInBrowser()
	expect.equality(printed, { "Not in a Jekyll project" })

	nvim_logic.print = orig_print
end

return T
