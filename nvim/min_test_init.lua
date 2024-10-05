-- mini_init.lua

-- ~/.config/nvim/init.lua
-- Load Lazy.nvim directly
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

-- Setup Lazy.nvim
require("lazy").setup({
	-- List your plugins here
	{
		"nvim-lua/plenary.nvim", -- Plenary for testing and other utilities
		lazy = false,
	},
	-- Add more plugins here
})

-- Optional: Disable swapfile and other unnecessary options
vim.opt.swapfile = false
vim.opt.backup = false
vim.opt.writebackup = false
vim.opt.undofile = false

-- Hello world test to confirm setup
print("Last line in test_init.lua")

-- You can now use describe, it, etc., in your test files.
