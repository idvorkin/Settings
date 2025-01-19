require("gitsigns").setup({
	on_attach = function(bufnr)
		local gs = package.loaded.gitsigns

		local function map(mode, l, r, opts)
			opts = opts or {}
			opts.buffer = bufnr
			vim.keymap.set(mode, l, r, opts)
		end

		-- Navigation
		map("n", "]c", function()
			if vim.wo.diff then
				return "]c"
			end
			vim.schedule(function()
				gs.next_hunk()
			end)
			return "<Ignore>"
		end, { expr = true })

		map("n", "[c", function()
			if vim.wo.diff then
				return "[c"
			end
			vim.schedule(function()
				gs.prev_hunk()
			end)
			return "<Ignore>"
		end, { expr = true })

		-- Actions
		map("n", "<leader>hs", gs.stage_hunk)
		map("n", "<leader>hr", gs.reset_hunk)
		map("v", "<leader>hs", function()
			gs.stage_hunk({ vim.fn.line("."), vim.fn.line("v") })
		end)
		map("v", "<leader>hr", function()
			gs.reset_hunk({ vim.fn.line("."), vim.fn.line("v") })
		end)
		map("n", "<leader>hS", gs.stage_buffer)
		map("n", "<leader>hu", gs.undo_stage_hunk)
		map("n", "<leader>hR", gs.reset_buffer)
		map("n", "<leader>hp", gs.preview_hunk_inline)
		map("n", "<leader>hb", function()
			gs.blame_line({ full = true })
		end)
		map("n", "<leader>tb", gs.toggle_current_line_blame)
		map("n", "<leader>hd", gs.diffthis)
		map("n", "<leader>hD", function()
			gs.diffthis("~")
		end)
		map("n", "<leader>td", gs.toggle_deleted)

		-- Text object
		map({ "o", "x" }, "ih", ":<C-U>Gitsigns select_hunk<CR>")
	end,
})

function GitCommitAndPush()
	-- Change directory to the directory of the current file
	vim.cmd("lcd %:p:h")

	-- Get the current file name
	local current_file = vim.fn.bufname()
	local current_win = vim.api.nvim_get_current_win()

	-- Stage the file
	vim.cmd("Gwrite")
	-- Run pre-commit hooks
	vim.cmd("!pre-commit run --files %")
	vim.cmd("e!")

	-- Function to generate commit message
	local function generate_commit_message()
		local msg = vim.fn.system("git diff --staged " .. current_file .. " | commit --oneline")
		local success = vim.v.shell_error == 0
		if not success or msg == "" then
			return "Checkpoint " .. current_file
		else
			return msg:gsub("\n$", "")
		end
	end

	-- Create preview buffer
	local preview_buf = vim.api.nvim_create_buf(false, true)
	vim.api.nvim_set_option_value("bufhidden", "wipe", { buf = preview_buf })

	-- Get diff content
	local diff_output = vim.fn.system("git diff --staged " .. current_file)
	local lines = vim.split(diff_output, "\n")

	-- Set buffer content
	vim.api.nvim_buf_set_lines(preview_buf, 0, -1, false, lines)
	vim.api.nvim_set_option_value("modifiable", false, { buf = preview_buf })
	vim.api.nvim_set_option_value("filetype", "diff", { buf = preview_buf })
	-- Use the current window
	vim.api.nvim_win_set_buf(current_win, preview_buf)

	-- Generate initial commit message
	local commit_message = generate_commit_message()

	-- Function to handle the commit process
	local function handle_commit()
		local result = vim.fn.input({
			prompt = "Commit message [" .. commit_message .. "] (y/n/+/new message): ",
			default = "",
		})

		if result == "n" then
			-- Restore the original buffer
			vim.cmd("e " .. current_file)
			return
		end

		if result == "+" then
			commit_message = generate_commit_message()
			return handle_commit()
		end

		local final_message = result
		if result == "" or result == "y" then
			final_message = commit_message
		elseif #result <= 3 then
			-- Ignore short messages that aren't y/n/+
			return handle_commit()
		end

		-- Perform the commit
		local commit_cmd = string.format("git commit %s -m '%s'", current_file, final_message)
		local commit_result = vim.fn.system(commit_cmd)

		if vim.v.shell_error == 0 then
			vim.fn.system("git push")
			-- Restore the original buffer
			vim.cmd("e " .. current_file)
			print("Changes committed and pushed successfully")
		else
			print("Error during commit: " .. commit_result)
			return handle_commit()
		end
	end

	-- Setup keymaps for the preview window
	local opts = { noremap = true, silent = true }
	vim.api.nvim_buf_set_keymap(preview_buf, "n", "q", "", {
		callback = handle_commit,
		noremap = true,
		silent = true,
	})

	-- Set buffer-local options
	vim.api.nvim_buf_set_keymap(preview_buf, "n", "<ESC>", "", {
		callback = function()
			vim.cmd("e " .. current_file)
		end,
		noremap = true,
		silent = true,
	})
	vim.bo[preview_buf].buflisted = false
	vim.bo[preview_buf].buftype = "nofile"
	vim.bo[preview_buf].swapfile = false

	-- Set window-local options
	vim.wo[current_win].number = true
	vim.wo[current_win].wrap = false
	vim.wo[current_win].cursorline = true
end

local neogit = require("neogit")
neogit.setup({})

require("octo").setup()
