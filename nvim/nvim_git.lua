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

	vim.cmd("Gwrite")
	-- pre-commit as it may do an edit
	vim.cmd("!pre-commit run --files %")
	-- reload the file
	vim.cmd("e!")

	-- Get the current file name, same as taking '%'
	local current_file = vim.fn.bufname()
	print(current_file)

	-- Create a new buffer and run git diff in a terminal in that buffer
	-- Use -1 to make it as big as possible, seems doesn't always work, use 999
	vim.cmd("999new")
	vim.bo.buftype = "nofile"
	vim.bo.bufhidden = "hide"
	vim.bo.swapfile = false
	vim.cmd("terminal export PAGER=don_t_use_me && git diff --staged " .. current_file)
	vim.api.nvim_buf_set_keymap(0, "n", "q", ":q<CR>", { noremap = true, silent = true })

	-- Defining a global function within GitCommitAndPush() to make a closure
	_G["ConfirmCommit"] = function()
		-- Ask for commit
		local commit_confirm = vim.fn.input("Do you want to commit " .. current_file .. "? (y/n) [y]: ")
		--  exit if n pressed
		if commit_confirm == "n" then
			return
		end

		-- default is blank
		local commit_message = "Checkpoint " .. current_file
		if commit_confirm == "" or commit_confirm == "y" then
			vim.cmd("!git commit " .. current_file .. " -m '" .. commit_message .. "' ")
			vim.cmd("!git push")
		end

		-- if more then 3 chars passed, make that the commit message [filename] - what is passed in
		if string.len(commit_confirm) > 3 then
			commit_message = commit_confirm
			vim.cmd("!git commit " .. current_file .. " -m '[" .. current_file .. "] " .. commit_message .. "' ")
			vim.cmd("!git push")
		end
	end

	-- Ask user to press Enter to continue after they close the buffer
	vim.api.nvim_exec([[ autocmd BufWinLeave <buffer> lua ConfirmCommit() ]], false)
end

local neogit = require("neogit")
neogit.setup({})

require("octo").setup()
