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

local function appendTables(t1, t2)
    for i=1, #t2 do
        t1[#t1+1] = t2[i]
    end
    return t1
end

local plugins = {
    -- Highlight current line
    -- ConoLineEnable (Highlight current line)
    "miyakogi/conoline.vim",
    "ekalinin/Dockerfile.vim",
    "terrastruct/d2-vim",
    "voldikss/vim-floaterm",
    "tree-sitter/tree-sitter-json",

    "HiPhish/rainbow-delimiters.nvim",

    -- Comment \cc
    -- Uncomment \cu
    "scrooloose/nerdcommenter",
    "dhruvasagar/vim-table-mode",
    "MDeiml/tree-sitter-markdown",
    "rking/ag.vim",
    "junegunn/limelight.vim",
    "dhruvasagar/vim-table-mode",
    "junegunn/goyo.vim",
    "reedes/vim-pencil",
    "folke/zen-mode.nvim",
    "neovim/nvim-lspconfig",
    -- :Rename a file
    "danro/rename.vim",
    -- Comment \cc
    -- Uncomment \cu
    "scrooloose/nerdcommenter",

    -- Auto Update ToC
    "mzlogin/vim-markdown-toc",
    "hrsh7th/cmp-nvim-lsp",
    "panozzaj/vim-autocorrect",
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
    "nvim-treesitter/nvim-treesitter",
    "glepnir/dashboard-nvim",
    "onsails/lspkind.nvim",
    "godlygeek/tabular",
}

local git_plugins = {
    "tpope/vim-fugitive",
    "lewis6991/gitsigns.nvim",
    --  DiffViewOpen
    "sindrets/diffview.nvim",
    "NeogitOrg/neogit",
    "pwntester/octo.nvim"
}

plugins = appendTables(plugins, git_plugins)



--  VIM LSP  for lua - I think I still need to configure t
require("lazy").setup(plugins)

