#!/bin/bash
# LG display watchdog: the LG Ultra HD sometimes comes back from display
# sleep at 30Hz. On every display-on event this checks the current mode (what
# `lg-show` prints) and re-applies `lg-fix` only when it has drifted.
#
# Display-on events come from powerd's unified-log line that backs the
# "Display is turned on" entries in `pmset -g log` - no polling.
#
# Usage:
#   lg-watchdog.sh [check]    Check and fix if needed (default)
#   lg-watchdog.sh watch      Check now, then on every display-on event
#   lg-watchdog.sh install    Install/reload the launchd agent running `watch`
#   lg-watchdog.sh uninstall  Remove the launchd agent
#   lg-watchdog.sh status     Show agent state, current mode, recent log

set -uo pipefail

# launchd runs with a minimal PATH
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

DISPLAY_NAME="LG Ultra HD"
WANT_RES="3840x2160"
WANT_HZ="59.94Hz"
MIN_HZ=59 # anything below this counts as drifted (e.g. the 30Hz fallback)

WAKE_SETTLE_SECONDS=5 # let WindowServer finish picking a mode before checking
DISPLAY_ON_PREDICATE='subsystem == "com.apple.powerd" AND category == "displayState" AND eventMessage BEGINSWITH "Creating assertion to keep device awake while display is on"'

LABEL="com.idvorkin.lg-watchdog"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
LOG="$HOME/Library/Logs/lg-watchdog.log"
SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') $*" >>"$LOG"
}

bd_get() {
    betterdisplaycli get --name="$DISPLAY_NAME" "$@" 2>/dev/null
}

check() {
    local res hz
    # Display off/disconnected or BetterDisplay not running: nothing to fix
    if ! res=$(bd_get --resolution) || ! hz=$(bd_get --refreshRate); then
        log "skip: '$DISPLAY_NAME' not available"
        return 0
    fi

    if [[ "$res" == "$WANT_RES" ]] && awk -v hz="${hz%Hz}" -v min="$MIN_HZ" 'BEGIN { exit !(hz >= min) }'; then
        return 0
    fi

    log "drift: $res @ $hz -> fixing to $WANT_RES @ $WANT_HZ"
    if betterdisplaycli set --name="$DISPLAY_NAME" --resolution="$WANT_RES" --refreshRate="$WANT_HZ" >>"$LOG" 2>&1; then
        log "fixed: now $(bd_get --resolution) @ $(bd_get --refreshRate)"
    else
        log "error: betterdisplaycli set failed"
        return 1
    fi
}

watch() {
    log "watch: started (pid $$)"
    check
    # `log` is a zsh builtin, so always call the binary by path
    /usr/bin/log stream --style compact --predicate "$DISPLAY_ON_PREDICATE" |
        while read -r line; do
            # Only timestamped event lines; the header echoes the predicate text
            [[ "$line" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2} ]] || continue
            log "display on"
            sleep "$WAKE_SETTLE_SECONDS"
            check
        done
    log "watch: log stream exited"
}

install() {
    mkdir -p "$(dirname "$PLIST")" "$(dirname "$LOG")"
    uninstall
    cat >"$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>$LABEL</string>
    <key>ProgramArguments</key>
    <array><string>$SCRIPT</string><string>watch</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardErrorPath</key><string>$LOG</string>
</dict>
</plist>
EOF
    launchctl bootstrap "gui/$(id -u)" "$PLIST"
    echo "Installed $LABEL (log: $LOG)"
}

uninstall() {
    launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null
    rm -f "$PLIST"
}

status() {
    launchctl list | grep "$LABEL" || echo "agent not loaded"
    echo "current: $(bd_get --resolution) @ $(bd_get --refreshRate)"
    [[ -f "$LOG" ]] && tail -n 10 "$LOG"
}

case "${1:-check}" in
check) check ;;
watch) watch ;;
install) install ;;
uninstall) uninstall ;;
status) status ;;
*)
    sed -n '2,14p' "$SCRIPT"
    exit 1
    ;;
esac
