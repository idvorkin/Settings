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

local plugins = {
"folke/zen-mode.nvim",
"neovim/nvim-lspconfig",
"hrsh7th/cmp-nvim-lsp",
"hrsh7th/cmp-buffer",
"hrsh7th/cmp-path",
"hrsh7th/cmp-cmdline",
"hrsh7th/nvim-cmp",
"nvim-lua/plenary.nvim",
"nvim-lualine/lualine.nvim",
"nvim-tree/nvim-web-devicons",
"Pocco81/true-zen.nvim",
"dstein64/vim-startuptime",
"folke/neodev.nvim",
"folke/trouble.nvim",
"nvim-telescope/telescope.nvim",
"stevearc/dressing.nvim",
"max397574/better-escape.nvim",
"stevearc/aerial.nvim",
"zbirenbaum/copilot.lua",
"zbirenbaum/copilot-cmp",
"nvim-neo-tree/neo-tree.nvim",
"MunifTanjim/nui.nvim",
}

vim.cmd "Plugin 'nvim-treesitter/nvim-treesitter', {'do': ':TSUpdate'}"

--  VIM LSP  for lua - I think I still need to configure t
require("lazy").setup(plugins)

