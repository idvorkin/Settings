#!/usr/bin/env -S nvim -l
-- Bootstrap mini.nvim for testing
local mini_path = vim.fn.stdpath("data") .. "/site/pack/deps/start/mini.nvim"
if not vim.uv.fs_stat(mini_path) then
	vim.notify("Cloning mini.nvim...", vim.log.levels.INFO)
	vim.fn.system({ "git", "clone", "--filter=blob:none", "https://github.com/echasnovski/mini.nvim", mini_path })
	vim.cmd("packadd mini.nvim")
end

-- Add repo root to rtp so require("nvim.foo") works
vim.opt.rtp:prepend(vim.uv.cwd())

-- Disable swap/backup noise
vim.opt.swapfile = false
vim.opt.backup = false
vim.opt.writebackup = false

require("mini.test").setup()
MiniTest.run({
	collect = {
		find_files = function()
			local dir = vim.uv.cwd() .. "/nvim/tests"
			local files = {}
			for name, type in vim.fs.dir(dir) do
				if type == "file" and name:match("^test_.*%.lua$") then
					table.insert(files, dir .. "/" .. name)
				end
			end
			table.sort(files)
			return files
		end,
	},
})
