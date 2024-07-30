
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
        }
    })

    popup: update_layout({
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
    win :mount()
    vim.api.nvim_set_current_win(win.winid)
    if G_CONVO_PATH == nil then
        vim.cmd('e!  `vim_python make-convo` ')
        G_CONVO_PATH = vim.fn.expand('%')
        print('creating '.. G_CONVO_PATH)
    else
        vim.cmd('e! '.. G_CONVO_PATH )
        print('loading '.. G_CONVO_PATH)
    end
    -- jump to the bottom of the buffer
    vim.cmd('normal G')
end

-- Map to vim command Ig2 to open the Convo popup
vim.cmd('command! -nargs=0 IgConvo lua OpenConvo()')
