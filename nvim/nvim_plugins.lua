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

local function get_openai_api_key()
    -- Resolve the path to the user's home directory
    local home = os.getenv("HOME") or os.getenv("USERPROFILE")
    local filepath = home .. "/gits/igor2/secretBox.json"

    -- Attempt to open the file
    local file, err = io.open(filepath, "r")
    if not file then
        vim.api.nvim_echo({{"Error opening file: " .. err, "ErrorMsg"}}, false, {})
        return nil
    end

    -- Read the entire file content
    local content = file:read("*a")
    file:close()

    local data, parse_err = vim.json.decode(content)
    if not data then
        vim.api.nvim_echo({{"Error parsing JSON: " .. parse_err, "ErrorMsg"}}, false, {})
        return nil
    end

    local openai_api_key = data.openai
    if not openai_api_key then
        vim.api.nvim_echo({{"openai_api_key not found in JSON", "ErrorMsg"}}, false, {})
        return nil
    end

    -- Set the global variable and echo the key
    vim.g.openai_api_key = openai_api_key
    return openai_api_key
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
    -- Other gpT plugin
    {
        "jackMort/ChatGPT.nvim",
        event = "VeryLazy",
         -- model = "gpt-4-1106-preview",
        config = function()
            require("chatgpt").setup({
            -- openai_api_key = get_openai_api_key(),
            -- api_key_command = get_openai_api_key(),
            -- string with model name or table with model name and parameters
           --  model = "gpt-4-1106-preview",
            })
        end,
        dependencies = {
            "MunifTanjim/nui.nvim",
            "nvim-lua/plenary.nvim",
            "nvim-telescope/telescope.nvim"
        }
    },

    "HiPhish/rainbow-delimiters.nvim",

    -- Comment \cc
    -- Uncomment \cu
    "scrooloose/nerdcommenter",
    'tyru/open-browser.vim', -- gx to open URL
    "dhruvasagar/vim-table-mode",
    "rking/ag.vim",
    "junegunn/limelight.vim",
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
    -- "ixru/nvim-markdown",
    "preservim/vim-markdown",

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
    {
        "folke/noice.nvim",
        event = "VeryLazy",
        opts = {
            -- add any options here
            messages = {
                -- NOTE: If you enable messages, then the cmdline is enabled automatically.
                -- This is a current Neovim limitation.
                enabled = false, -- enables the Noice messages UI
            }
        },
        dependencies = {
            -- if you lazy-load any plugin below, make sure to add proper `module="..."` entries
            "MunifTanjim/nui.nvim",
            -- OPTIONAL:
            --   `nvim-notify` is only needed, if you want to use the notification view.
            --   If not available, we use `mini` as the fallback
            "rcarriga/nvim-notify",
        }
    },
    "nvim-tree/nvim-web-devicons",
    "dstein64/vim-startuptime",
    "folke/neodev.nvim",
    {
      "folke/trouble.nvim",
      opts = {}, -- for default options, refer to the configuration section for custom setup.
      cmd = "Trouble",
      keys = {
        {
          "<leader>xx",
          "<cmd>Trouble diagnostics toggle<cr>",
          desc = "Diagnostics (Trouble)",
        },
        {
          "<leader>xX",
          "<cmd>Trouble diagnostics toggle filter.buf=0<cr>",
          desc = "Buffer Diagnostics (Trouble)",
        },
        {
          "<leader>cs",
          "<cmd>Trouble symbols toggle focus=false<cr>",
          desc = "Symbols (Trouble)",
        },
        {
          "<leader>cl",
          "<cmd>Trouble lsp toggle focus=false win.position=right<cr>",
          desc = "LSP Definitions / references / ... (Trouble)",
        },
        {
          "<leader>xL",
          "<cmd>Trouble loclist toggle<cr>",
          desc = "Location List (Trouble)",
        },
        {
          "<leader>xQ",
          "<cmd>Trouble qflist toggle<cr>",
          desc = "Quickfix List (Trouble)",
        },
      },
    },
    { 'nvim-telescope/telescope-fzf-native.nvim', build = 'cmake -S. -Bbuild -DCMAKE_BUILD_TYPE=Release && cmake --build build --config Release' },
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
      opts = function()
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
    -- "pwntester/octo.nvim"
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
plugins = appendTables(plugins, {
        {
        "dustinblackman/oatmeal.nvim",
        cmd = { "Oatmeal" },
        keys = {
            { "<leader>om", mode = "n", desc = "Start Oatmeal session" },
        },
        opts = {
            backend = "ollama",
            model = "codellama:latest",
        },
    }
}
)
    -- gpt plugin
    local gpt2 =  {
        {
            "robitx/gp.nvim",
            opts = {
                chat_topic_gen_model = "gpt-4-1106-preview",
                openai_api_key = get_openai_api_key(),
                myagents = {
                    {
                        name = "ChatGPT4",
                        chat = true,
                        command = false,
                        -- string with model name or table with model name and parameters
                        model = "gpt-4-1106-preview",
                        -- system prompt (use this to specify the persona/role of the AI)
                        system_prompt = "You are a general AI assistant.\n\n"
                        .. "The user provided the additional info about how they would like you to respond:\n\n"
                        .. "- If you're unsure don't guess and say you don't know instead.\n"
                        .. "- Ask question if you need clarification to provide better answer.\n"
                        .. "- Think deeply and carefully from first principles step by step.\n"
                        .. "- Zoom out first to see the big picture and then zoom in to details.\n"
                        .. "- Use Socratic method to improve your thinking and coding skills.\n"
                        .. "- Don't elide any code from your output if the answer requires coding.\n"
                        .. "- Take a deep breath; You've got this!\n",
                    },
                }
            }
        },
    }

--
-- require('lspconfig').racket_langserver.setup()
--  VIM LSP  for lua - I think I still need to configure
require("lazy").setup(plugins)

