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

local function wordcount()
    return tostring(vim.fn.wordcount().words) .. ' words'
end

local function is_markdown()
    return vim.bo.filetype == "markdown" or vim.bo.filetype == "asciidoc"
end

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
    -- Like LimeLight
    {
        "folke/twilight.nvim",
        opts = {
            context = 5, -- amount of lines we will try to show around the current line
        },
    },
    "ekalinin/Dockerfile.vim",
    "terrastruct/d2-vim",
    "voldikss/vim-floaterm",

    "HiPhish/rainbow-delimiters.nvim",

    -- Comment \cc
    -- Uncomment \cu
    "scrooloose/nerdcommenter",
    "dhruvasagar/vim-table-mode",
    "rking/ag.vim",
    "junegunn/limelight.vim",
    "dhruvasagar/vim-table-mode",
    "junegunn/goyo.vim",
    "reedes/vim-pencil",
    "catppuccin/nvim",

    "folke/zen-mode.nvim",
    -- :Rename a file
    "danro/rename.vim",
    -- Comment \cc
    -- Uncomment \cu
    "scrooloose/nerdcommenter",

    -- NVIM markdown stuff,  lets see if it works with tree sitter
    "ixru/nvim-markdown",

    -- Auto Update ToC
    "mzlogin/vim-markdown-toc",
    "panozzaj/vim-autocorrect",
    "nvim-lua/plenary.nvim",
    {
        "nvim-lualine/lualine.nvim",
        opts  = {
            sections = {
                lualine_z = {
                    "aerial" ,
                    -- By default, Z is line:column, I don't mind line, but column is too noisy
                    { wordcount,   cond = is_markdown },
                }
            },
        }
    },
    "nvim-tree/nvim-web-devicons",
    "Pocco81/true-zen.nvim",
    "dstein64/vim-startuptime",
    "folke/neodev.nvim",
    "folke/trouble.nvim",
    "nvim-telescope/telescope.nvim",
    "stevearc/dressing.nvim",
    {
        "max397574/better-escape.nvim",
        opts = {
            mapping = {"fj"}, -- a table with mappings to use
            timeout = vim.o.timeoutlen, -- the time in which the keys must be hit in ms. Use option timeoutlen by default
            clear_empty_lines = false, -- clear line after escaping if there is only whitespace
            keys = "<Esc>", -- keys used for escaping, if it is a function will use the result everytime
        }
    },
    "stevearc/aerial.nvim",
    {
        "nvim-neo-tree/neo-tree.nvim",
        opts = {
            window = {
                mappings = {
                    ["u"] = "navigate_up",
                }
            }
        }
    },
    "MunifTanjim/nui.nvim",
    "godlygeek/tabular",
    "AckslD/nvim-neoclip.lua",
    "preservim/vim-colors-pencil",
    'ttibsi/pre-commit.nvim',
}

local function readEulogyPrompts()
    local eulogy_prompts = vim.fn.systemlist("cat ~/gits/igor2/eulogy_prompts.md")
    if #eulogy_prompts == 0 then
        print("No prompts found.")
        return nil
    end
    math.randomseed(os.time()) -- Seed the random number generator
    local random_index = math.random(1, #eulogy_prompts)
    return eulogy_prompts[random_index]
end
-- Add dashboard
plugins = appendTables(plugins, {
     {
      'nvimdev/dashboard-nvim',
      event = 'VimEnter',
      config = function()
        require('dashboard').setup {
            theme="hyper",
            config={
                week_header = {
                    enable = true,
                },
            --footer = {"Igor Is here"}, -- footer
            footer = {readEulogyPrompts()},
            }
          -- config
          --project = { enable = true}

        }
      end,
      dependencies = { {'nvim-tree/nvim-web-devicons'}}
    },
}
)

-- Read the eulogy prompts, and insert 3 random ones
-- command! PromptEulogy  :r !shuf -n 3 ~/gits/igor2/eulogy_prompts.md
--


vim.g.dashboard_command_footer = readEulogyPrompts()

-- TSPlaygroundToggle
-- :TSHighlightCapturesUnderCursor
-- :TSNodeUnderCursor
--
plugins = appendTables(plugins, {
    "tree-sitter/tree-sitter-json",
    "nvim-treesitter/playground",
    "nvim-treesitter/nvim-treesitter",
    "MDeiml/tree-sitter-markdown",
})


plugins = appendTables(plugins, {"tpope/vim-surround"})
--[[
     Cool does wrapping
    help surround

    Wrap current line
    ys -> you surround, motion, element
    yss* <- Wrap 'Surround' line '*'
    ds" -> delete surround
    cs" -> change surround

    Setup surround for b (old)  and i(talics) for markdown.
    echo char2nr('b') -> 105
    "
    Cheat Sheat
    " - yssX - soround the current line with italics(i) or bold(b) or something
    " else.
    "
    - Once in visual mode, S will do the surround folowed by the b so like
    select text in visual mode, then Sb will make it bold.
]]

local git_plugins = {
    "tpope/vim-fugitive",
    "lewis6991/gitsigns.nvim",
    --  DiffViewOpen
    "sindrets/diffview.nvim",
    "NeogitOrg/neogit",
    "pwntester/octo.nvim"
}
plugins = appendTables(plugins, git_plugins)

-- plugins = appendTables(plugins, {"mhartington/formatter.nvim"})
-- plugins = appendTables(plugins, {"sbdchd/neoformat"})

-- Configure formatter



-- cmp and friends
plugins = appendTables(plugins, {
    "hrsh7th/cmp-nvim-lsp",
    "hrsh7th/cmp-buffer",
    "hrsh7th/cmp-path",
    "hrsh7th/nvim-cmp",
    "hrsh7th/cmp-cmdline",
    "zbirenbaum/copilot.lua",
    "zbirenbaum/copilot-cmp",
    "neovim/nvim-lspconfig",
    "onsails/lspkind.nvim",
})
-- lispy and racket
plugins = appendTables(plugins, {
    "wlangstroth/vim-racket",
    "Olical/conjure",
    "PaterJason/cmp-conjure",
})

--
-- require('lspconfig').racket_langserver.setup()
--  VIM LSP  for lua - I think I still need to configure
require("lazy").setup(plugins)

