-- Lets keep exra lua stuff here
--
require'nvim-web-devicons'.get_icons()
require("true-zen").setup {
    -- your config goes here
    -- or just leave it empty :)
    plugins= {
        tmux = { enabled = false }, -- disables the tmux statusline
    }
}

-- require('lualine').setup()
print("Config Loaded")
