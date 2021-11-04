require("hs.json")

-- By having IPC loaded can do:
-- /usr/local/bin hs -c "command"
require("hs.ipc")
require("hs.caffeinate")
--

function laptopScreen()
    return hs.screen.allScreens()[1]
end

function monitorScreen()
    return hs.screen.allScreens()[2]
end

inspect = hs.inspect.inspect

function getSecretFromSecretBox(secret)
    --  local vpnKey = hs.execute("~/gits/igor2/secretBox.json | jq '.VPNKey'")
    local pf = "/Users/idvorkin/gits/igor2/secretBox.json"
    local secretsAsText = io.open(pf):read("*a")
    secrets = hs.json.decode(secretsAsText)
    return secrets[secret]
end

function screenCapture()
 hs.execute('screencapture -isc >/dev/null 1>&2')
end

function vpn()
    hs.application.launchOrFocus("Cisco AnyConnect Secure Mobility Client")
    hs.timer.usleep(1 * 1000 * 1000)
    hs.eventtap.keyStroke({}, "return")
    hs.timer.usleep(5 * 1000 * 1000)
    hs.eventtap.keyStrokes(getSecretFromSecretBox("VPNKey"))
end

-- To call from alfred do like:
-- Not sure why install is broken to /usr/local/bin/hs
-- /Users/idvorkin/bin/hs -c "chatWith('{query}')"
function chatWith(name)
    hs.application.launchOrFocus("Workplace Chat")
    hs.timer.usleep(0.3 * 1000 * 1000)
    hs.eventtap.keyStroke({"cmd"}, "k")
    hs.eventtap.keyStroke({"cmd"}, "a")
    hs.timer.usleep(0.1 * 1000 * 1000)
    print ("Trying to chat with")
    print (name)
    hs.eventtap.keyStrokes(name)
end

function reloadConfig()
      hs.reload()
      hs.alert.show("Config reloaded")
end

-- Screen co-ordinates are a global, with main monitor top left = 0,0
--

-- use integer division to make equality work for swapping.
--

function toggle(partial)
  print ("Toggling")

  local win = hs.window.focusedWindow()
  local original_screen = win:screen()
  if original_screen == laptopScreen  then
     move_to(true,laptopScreen, 1)
     return
  end

  -- 4 cases:
  -- On Left exactly -> go right
  -- On Right exactly -> go left
  -- On Left hand side -> go left
  -- On Right hand side -> go right

  local origFrame = win:frame()
  local max = monitorScreen():frame()


  print ("Original")
  print (inspect(origFrame))
  print ("Screen")
  print (inspect(max))

  local newWidth =  max.w/ partial
  local changeWidth =  math.abs(origFrame.w - newWidth) > 10 -- rounding errors
  local onLeft = origFrame.x  == max.x

  print ("New Width:", newWidth)
  print ("changeWidth-",changeWidth)
  print ("onLeft-",onLeft)

  local left=true

  if onLeft then
      left = changeWidth
  else
      left = not changeWidth
  end

  print("Move to left:", left)
  return move_to(left, monitorScreen(), partial)
end

function move_to(left,target_screen, partial)

  local win = hs.window.focusedWindow()
  local original_screen = win:screen()
  if original_screen ~= target_screen then
      win.moveToScreen(target_screen)
  end

  local max = target_screen:frame()
  local new_frame = win:frame()
  new_frame.y = max.y
  new_frame.w = max.w / partial
  new_frame.h = max.h
  if left then
    new_frame.x = max.x
  else
    -- new_frame.x =  max.x - new_frame.w
    new_frame.x =  max.x + (max.w - new_frame.w)
  end
  win:setFrame(new_frame)
end

function toleft(target_screen, partial)
    move_to(true, target_screen, partial)
end

function toright(target_screen, partial)
    move_to(false, target_screen, partial)
end

function reload()
    hs.reload()
    hs.alert.show("Config loaded")
end

-- Fancy Reloading
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
function lock()
    hs.caffeinate.lockScreen()
end

-- Sigh this is getting complicated
-- Inspiration
-- https://bezhermoso.github.io/2016/01/20/making-perfect-ramen-lua-os-x-automation-with-hammerspoon/
-- Testing: hs.urlevent.openURL("hammerspoon://left?q=right")

myWatcher = hs.pathwatcher.new(os.getenv("HOME") .. "/.hammerspoon/", reloadConfig):start()
hs.alert.show("Config loaded")

hs.hotkey.bind({"cmd"}, "l",lock)
hs.hotkey.bind({"ctrl"}, "l",lock)
hs.hotkey.bind({"cmd", "alt", "ctrl"}, "r", reloadConfig)

function urlMove(event, params)
    -- inspect (params)
    -- print (event)
    -- print (params)
    -- print (inspect(params))
    left = params["left"] ~= nil
    if params["right"] ~= nil then
        left = false
    end

    partial = 0.5
    if params["partial"] ~= nil then
        partial = params["partial"]
    end

    target_screen = monitorScreen()
    if params["laptop"] ~= nil then
        target_screen = laptopScreen()
    end

    move_to(left, target_screen, partial)
end

hs.urlevent.bind("move", urlMove)

-- To connecto to stream deck, use applescript plugin and do this:
-- do shell script "/Users/idvorkin/bin/hs -c 'toggle(3/2)' "
