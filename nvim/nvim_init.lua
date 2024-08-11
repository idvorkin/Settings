-- Setup Packer
settings_dir =  os.getenv("HOME").."/settings/nvim/"
dofile(settings_dir.."nvim_plugins.lua")


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
  ensure_installed = { "c", "lua", "vim", "vimdoc", "query" , "markdown_inline", "python", "javascript", "regex"},

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
    additional_vim_regex_highlighting = false
  },
  textobjects = {
      select = {
          enable = true,
          lookahead = true,
          keymaps = {
              ["af"] = "@function.outer",
              ["if"] = "@function.inner",
          },
      },
  }
}

dofile(settings_dir.."nvim_cmp_copilot.lua")
dofile(settings_dir.."nvim_git.lua")
dofile(settings_dir.."nvim_color.lua")


vim.cmd([[
command! -nargs=* ZMT lua ZenModeToggleFunction(<f-args>)
]])

function ZenModeToggleFunction(width)
  if width == nil or width == "" then
    width = "66"
  end
  local width_percentage = tonumber(width) / 100
  require("zen-mode").toggle({
    window = {
      width = width_percentage
    }
  })
end


function InsertYouTubeTemplate()
  -- Get the clipboard content
  local clipboard = vim.fn.getreg('+')

  -- Pattern to match a YouTube URL and extract the video ID
  local pattern = 'https://www%.youtube%.com/watch%?v=([%w-_]+)'

  -- Try to match the clipboard content against the pattern
  local video_id = clipboard:match(pattern)

  if video_id then
    -- If a match is found, create the template string
    local template = string.format('{%%include youtube.html src="%s" %%}', video_id)

    -- Insert the template string into the current buffer
    vim.api.nvim_put({template}, 'c', true, true)
  else
    -- If no match is found, print an error message
    print('Clipboard does not contain a valid YouTube URL.')
  end
end

print("nvim_intit.lua loaded")
