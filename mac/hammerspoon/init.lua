require("hs.json")

-- By having IPC loaded can do:
-- /usr/local/bin hs -c "command"
require("hs.ipc")
--



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

function chatWith(name)
    hs.application.launchOrFocus("Workplace Chat")
    hs.timer.usleep(0.3 * 1000 * 1000)
    hs.eventtap.keyStroke({"cmd"}, "k")
    hs.timer.usleep(0.1 * 1000 * 1000)
    hs.eventtap.keyStrokes(name)
end

function reloadConfig()
      hs.reload()
      hs.alert.show("Config reloaded")
end

hs.hotkey.bind({"cmd", "alt", "ctrl"}, "r", reloadConfig)
