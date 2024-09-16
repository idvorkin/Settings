-- Move Convo Control stuff here.
--

local function SetupPopup()
	-- https://github.com/MunifTanjim/nui.nvim/tree/main/lua/nui/popup
	local Popup = require("nui.popup")

	local popup = Popup({
		border = {
			padding = {
				top = 2,
				bottom = 2,
				left = 3,
				right = 3,
			},
			style = "rounded",
			text = {
				top = "GPTChat",
				top_align = "center",
				bottom = "I am bottom title",
				bottom_align = "left",
			},
		},
	})

	popup:update_layout({
		relative = "editor",
		size = {
			width = 0.9,
			height = 0.7,
		},
		position = {
			row = 5,
			col = 5,
		},
	})
	--popup:on("BufLeave", function()
	--popup:unmount()
	--end, { once = true })

	return popup
end

G_CONVO_PATH = nil
function OpenConvo()
	local win = SetupPopup()
	win:mount()
	vim.api.nvim_set_current_win(win.winid)
	if G_CONVO_PATH == nil then
		vim.cmd("e!  `vim_python make-convo` ")
		G_CONVO_PATH = vim.fn.expand("%")
		print("creating " .. G_CONVO_PATH)
	else
		vim.cmd("e! " .. G_CONVO_PATH)
		print("loading " .. G_CONVO_PATH)
	end
	-- jump to the bottom of the buffer
	vim.cmd("normal G")
end

function WINDOW_OPEN_CONVO()
	local convo_buffer = nil

	-- Check if a buffer with the filename ending in '.convo.md' exists
	for _, buf in ipairs(vim.api.nvim_list_bufs()) do
		local buf_name = vim.api.nvim_buf_get_name(buf)
		if buf_name:match(".*%.convo%.md$") then
			convo_buffer = buf
			break
		end
	end

	-- Check if current buffer is empty and close it if so
	local current_buf = vim.api.nvim_get_current_buf()
	if
		vim.api.nvim_buf_line_count(current_buf) == 1
		and vim.api.nvim_buf_get_lines(current_buf, 0, -1, false)[1] == ""
	then
		vim.api.nvim_buf_delete(current_buf, { force = true })
	end

	if convo_buffer then
		-- If the '.convo.md' buffer exists, find or create a window for it
		local win_id = vim.fn.bufwinid(convo_buffer)
		if win_id == -1 then
			vim.cmd("sbuffer " .. convo_buffer)
		else
			vim.api.nvim_set_current_win(win_id)
		end
	else
		-- If the '.convo.md' buffer doesn't exist, create a new one
		vim.cmd("new `vim_python make-convo`")
	end

	-- Jump to the bottom of the file
	vim.cmd("normal! G")
end

function AvanteEnglish()
	-- Set system prompt to fun
	local new_prompt = [[This file contains markdown of english. Keep the english simple, upbeat an fun
    ]]
	require("avante.config").override({ system_prompt = new_prompt })
end

function IgAvanteDebug()
	-- Set system prompt to fun
	print(require("avante.config").system_prompt)
end

-- Map to vim command Ig2 to open the Convo popup
vim.cmd("command! -nargs=0 IgOldConvo lua WINDOW_OPEN_CONVO()")
vim.cmd("command! -nargs=0 IgAvanteEnglish lua AvanteEnglish()")
vim.cmd("command! -nargs=0 IgAvantePrompt lua IgAvanteDebug()")
