#!/usr/bin/osascript

on run argv
    if (count of argv) is 0 then
        set searchTerm to "Pass a chat name"
    else
        set searchTerm to item 1 of argv
    end if

    -- Check if Workplace Chat is open
    set isAppRunning to false
    tell application "System Events"
        set runningApps to (name of every process)
        if "Workplace Chat" is in runningApps then
            set isAppRunning to true
        end if
    end tell

    -- Open Workplace Chat app if it's not open
    if isAppRunning is true then
        tell application "Workplace Chat"
            activate
        end tell
    end if

    -- Wait for Workplace Chat to become the frontmost app
    tell application "System Events"
        repeat while not (frontmost of process "Workplace Chat")
            delay 0.5
        end repeat
    end tell

    -- Send Command-K
    tell application "System Events"
        tell process "Workplace Chat"
            keystroke "k" using {command down}
        end tell
    end tell

    -- Sleep for a bit
    delay 0.5

    -- Send Command-A
    tell application "System Events"
        tell process "Workplace Chat"
            keystroke "a" using {command down}
        end tell
    end tell

    -- Sleep for 1 second
    delay 1

    -- Type searchTerm
    tell application "System Events"
        tell process "Workplace Chat"
            keystroke searchTerm
        end tell
    end tell
end run
