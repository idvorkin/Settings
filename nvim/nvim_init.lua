-- Setup Packer
settings_dir =  os.getenv("HOME").."/settings/nvim/"
dofile(settings_dir.."nvim_plugins.lua")


require("dashboard").setup()
require("trouble").setup{
}


require('aerial').setup({
    placement= "edge",
    layout = {
        default_direction = "left",
    },
    -- optionally use on_attach to set keymaps when aerial has attached to a buffer
    on_attach = function(bufnr)
        -- Jump forwards/backwards with '{' and '}'
        vim.keymap.set('n', '{', '<cmd>AerialPrev<CR>', {buffer = bufnr})
        vim.keymap.set('n', '}', '<cmd>AerialNext<CR>', {buffer = bufnr})
        vim.keymap.set('n', '<C-y>', '<cmd>AerialToggle<CR>')
    end
})
require('telescope').load_extension('aerial')



-- Lets keep exra lua stuff here
require'nvim-web-devicons'.get_icons()
require("true-zen").setup {
    -- your config goes here
    -- or just leave it empty :)
    plugins= {
        tmux = { enabled = false }, -- disables the tmux statusline
    }
}


--[[ Setup Tree Sitter
Debug w/ :TSInstallInfo
See: https://github.com/MDeiml/tree-sitter-markdown/issues/121

Install Plugins
    :TSInstall markdown_inline
    :TSInstall yaml



]]

-- Bleh lets debug highlighting
-- For VIM hightlights
vim.cmd([[
    function! HighlightGroup()
        echo synIDattr(synID(line("."), col("."), 1), "name")
    endfunction
    function! HighlightLinks(group)
        redir => links
        silent highlight
        redir END
        return filter(split(links, '\n'), 'v:val =~ "^' . a:group . '\s\+xxx"')
    endfunction

    " Fix highlighting for markdown
    " highlight link @text.emphasis htmlItalic
    " highlight link @text.strong htmlBold

]])
-- For TS Highlights
--  :TSHighlightCapturesUnderCursor


require'nvim-treesitter.configs'.setup {
  -- A list of parser names, or "all" (the five listed parsers should always be installed)
  ensure_installed = { "c", "lua", "vim", "vimdoc", "query" , "markdown_inline", "python", "javascript"},

  -- Install parsers synchronously (only applied to `ensure_installed`)
  sync_install = false,

  -- Automatically install missing parsers when entering buffer
  -- Recommendation: set to false if you don't have `tree-sitter` CLI installed locally
  auto_install = true,


  highlight = {
    enable = true,
    -- Setting this to true will run `:h syntax` and tree-sitter at the same time.
    -- Set this to `true` if you depend on 'syntax' being enabled (like for indentation).
    -- Using this option may slow down your editor, and you may see some duplicate highlights.
    -- Instead of true it can also be a list of languages
    -- additional_vim_regex_highlighting = {'markdown','markdown_inline'},
    additional_vim_regex_highlighting = False
  },
}




-- Example using a list of specs with the default options
-- vim.g.mapleader = " " -- Make sure to set `mapleader` before lazy so your mappings are correct
local dashboard_nvim = {
  'glepnir/dashboard-nvim',
  event = 'VimEnter',
  config = function()
    require('dashboard').setup {
      -- config
    }
  end,
  dependencies = { {'nvim-tree/nvim-web-devicons'}}
}



local junk =  {
  'glepnir/dashboard-nvim',
  event = 'VimEnter',
  config = function()
    require('dashboard').setup {
      -- config
    }
  end,
  requires = {'nvim-tree/nvim-web-devicons'}
}




require("better_escape").setup {
    mapping = {"fj"}, -- a table with mappings to use
    timeout = vim.o.timeoutlen, -- the time in which the keys must be hit in ms. Use option timeoutlen by default
    clear_empty_lines = false, -- clear line after escaping if there is only whitespace
    keys = "<Esc>", -- keys used for escaping, if it is a function will use the result everytime
    -- example(recommended)
    -- keys = function()
    --   return vim.api.nvim_win_get_cursor(0)[2] > 1 and '<esc>l' or '<esc>'
    -- end,
}

require("neo-tree").setup(
{
      window = {
            mappings = {
              ["u"] = "navigate_up",
          }
      }
})

-- Provides the Format, FormatWrite, FormatLock, and FormatWriteLock commands
require("formatter").setup {
    -- Enable or disable logging
    logging = true,
    -- Set the log level
    log_level = vim.log.levels.WARN,
    -- All formatter configurations are opt-in
    filetype = {
        -- Formatter configurations for filetype "lua" go here
        -- and will be executed in order
        lua = {
            require("formatter.filetypes.lua").stylua,
        },
        python = {
            require("formatter.filetypes.python").black,
        },
        markdown = {
            require("formatter.filetypes.markdown"),
        },
    }
}


dofile(settings_dir.."nvim_lualine.lua")
dofile(settings_dir.."nvim_cmp_copilot.lua")
dofile(settings_dir.."nvim_git.lua")

vim.cmd(":colorscheme catppuccin")

print("Config Loaded")
