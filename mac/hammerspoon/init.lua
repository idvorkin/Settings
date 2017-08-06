-- win is an alias for:  
-- https://github.com/S1ngS1ng/HammerSpoon/blob/master/window-management.lua

wm = require "win" 
hs.alert.show("Start Load")

hs.hotkey.bind({"cmd", "alt", "ctrl"}, "R", function()
  hs.notify.new({title="Hammerspoon", informativeText="Hello World"}):send()
  hs.alert.show("Hello World!")

end)
-- Lock the machine 
hs.hotkey.bind({"ctrl","cmd", "alt"}, "L", function()
    hs.caffeinate.lockScreen()
end)

-------------------------------
-- Start Default brower stuff.
-- Copied from: 
-- https://andrich.blog/2016/11/20/hammerspoon-an-awesome-tool-to-automate-your-mac/
-------------------------------
 
-- Step 1: Take care, that Hammerspoon is the default browser
if hs.urlevent.getDefaultHandler("http") ~= "org.hammerspoon.hammerspoon" then
    hs.urlevent.setDefaultHandler("http")
end
 
-- Step 2: Setup the browser menu
local active_browser     = hs.settings.get("active_browser") or "com.apple.safari"
local browser_menu       = hs.menubar.new()
local available_browsers = {
    ["com.apple.safari"] = {
        name = "Safari",
        icon = os.getenv("HOME") .. "/.hammerspoon/browsermenu/safari.png"
    },
    ["org.mozilla.firefox"] = {
        name = "Firefox",
        icon = os.getenv("HOME") .. "/.hammerspoon/browsermenu/firefox.png"
    },
    ["com.google.chrome"] = {
        name = "Google Chrome",
        icon = os.getenv("HOME") .. "/.hammerspoon/browsermenu/chrome.png"
    },
}
 
function init_browser_menu()
    local menu_items = {}
 
    for browser_id, browser_data in pairs(available_browsers) do
        local image = hs.image.imageFromPath(browser_data["icon"]):setSize({w=16, h=16})
 
        if browser_id == active_browser then
            browser_menu:setIcon(image)
        end
 
        table.insert(menu_items, {
            title   = browser_data["name"],
            image   = image,
            checked = browser_id == active_browser,
            fn      = function()
                active_browser = browser_id
                hs.settings.set("active_browser", browser_id)
                init_browser_menu()
            end
        })
    end
 
    browser_menu:setMenu(menu_items)
end
 
init_browser_menu()
 
-- Step 3: Register a handler for opening URLs
hs.urlevent.httpCallback = function(scheme, host, params, fullURL)
    hs.urlevent.openURLWithBundle(fullURL, active_browser)
end

-------------------------------
-- End Default brower stuff.
-------------------------------



-- Auto reload config
function reloadConfig(files)
    doReload = false
    for _,file in pairs(files) do
        if file:sub(-4) == ".lua" then
            doReload = true
        end
    end
    if doReload then
        hs.reload()
    end
end
local myWatcher = hs.pathwatcher.new(os.getenv("HOME") .. "/.hammerspoon/", reloadConfig):start()

local mouseCircle = nil
local mouseCircleTimer = nil

function mouseHighlight()
        -- Delete an existing highlight if it exists
        if mouseCircle then
            mouseCircle:delete()
            if mouseCircleTimer then
                mouseCircleTimer:stop()
            end
        end
        -- Get the current co-ordinates of the mouse pointer
        mousepoint = hs.mouse.getAbsolutePosition()
        -- Prepare a big red circle around the mouse pointer
        mouseCircle = hs.drawing.circle(hs.geometry.rect(mousepoint.x-40, mousepoint.y-40, 80, 80))
        mouseCircle:setStrokeColor({["red"]=1,["blue"]=0,["green"]=0,["alpha"]=1})
        mouseCircle:setFill(false)
        mouseCircle:setStrokeWidth(5)
        mouseCircle:show()

        -- Set a timer to delete the circle after 3 seconds
        mouseCircleTimer = hs.timer.doAfter(3, function() mouseCircle:delete() end)
end
hs.hotkey.bind({"cmd","alt","control"}, "D", mouseHighlight)

-- Play with a command mode!
---------------------------------------
k = hs.hotkey.modal.new('cmd', ';')
function k:entered() hs.alert'Entered mode' end
function k:exited() hs.alert'Exited mode' end
k:bind('', 'escape', function() k:exit() end)
k:bind('cmd', ';', function() k:exit() end)

k:bind('', 'J', 'throwLeft',function() 
        wm.throwLeft()
        k:exit()
end)
k:bind('', 'H', 'left half',function() 
        k:exit()
        wm.cycleLeft()
end)
k:bind('', 'L', 'right half',function() 
        k:exit()
        wm.cycleRight()
end)

k:bind('', 'K', 'throw right',function() 
        k:exit()
        wm.throwRight()
end)

k:bind('', 'R', 'reload',function() 
        k:exit()
        hs.reload()
end)

k:bind('', 'F', 'winHint',function() 
        k:exit()
        hs.hints.windowHints()
end)

k:bind('', 'C', 'center',function() 
        k:exit()
        wm.maximizeWindow()
end)

hs.alert.show("Config loaded")
