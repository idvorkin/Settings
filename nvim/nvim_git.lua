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

	-- Function to check if we're in a specific repo
	local function is_special_repo()
		local git_dir = vim.fn.system("git rev-parse --git-dir")
		local repo_path = vim.fn.fnamemodify(git_dir:gsub("\n", ""), ":h")
		return repo_path:match("igor2$") or repo_path:match("work%-notes$")
	end

	-- Function to generate commit message
	local function generate_commit_message()
		if is_special_repo() then
			return "Checkpoint " .. current_file
		end
		
		local msg = vim.fn.system("git diff --staged " .. current_file .. " | commit --oneline")
		local success = vim.v.shell_error == 0
		if not success or msg == "" then
			return "Checkpoint " .. current_file
		else
			return msg:gsub("\n$", "")
		end
	end

	-- Create a terminal buffer for colored diff output
	local preview_buf = vim.api.nvim_create_buf(false, true)
	vim.api.nvim_set_option_value("bufhidden", "wipe", { buf = preview_buf })
	
	-- Use the current window
	vim.api.nvim_win_set_buf(current_win, preview_buf)
	
	-- Open terminal with delta or git diff
	local term_cmd = "git diff --staged " .. vim.fn.shellescape(current_file)
	if vim.fn.executable("delta") == 1 then
		term_cmd = term_cmd .. " | delta"
	else
		term_cmd = term_cmd .. " --color"
	end
	
	local term_chan = vim.api.nvim_open_term(preview_buf, {})
	local first_non_empty_line_sent = false
	vim.fn.jobstart(term_cmd, {
		on_stdout = function(_, data)
			if data then
				-- Skip empty lines at the start
				if not first_non_empty_line_sent then
					-- Find first non-empty line
					local start_idx = 1
					while start_idx <= #data and data[start_idx]:match("^%s*$") do
						start_idx = start_idx + 1
					end
					
					if start_idx <= #data then
						first_non_empty_line_sent = true
						-- Send remaining lines
						local remaining = {}
						for i = start_idx, #data do
							table.insert(remaining, data[i])
						end
						vim.api.nvim_chan_send(term_chan, table.concat(remaining, "\n") .. "\n")
					end
				else
					-- After first non-empty line is sent, send all lines normally
					vim.api.nvim_chan_send(term_chan, table.concat(data, "\n") .. "\n")
				end
			end
		end,
		on_exit = function()
			vim.api.nvim_buf_set_option(preview_buf, "modifiable", false)
		end,
	})

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

	vim.api.nvim_buf_set_keymap(preview_buf, "n", "<ESC>", "", {
		callback = function()
			vim.cmd("e " .. current_file)
		end,
		noremap = true,
		silent = true,
	})

	-- Set buffer options
	vim.bo[preview_buf].buflisted = false
	vim.bo[preview_buf].buftype = "terminal"  -- Changed to terminal since we're using a terminal buffer
	vim.bo[preview_buf].swapfile = false

	-- Set window options
	vim.wo[current_win].number = true
	vim.wo[current_win].wrap = false
	vim.wo[current_win].cursorline = true
end

local neogit = require("neogit")
neogit.setup({})

require("octo").setup()
