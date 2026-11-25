#!/bin/sh
# Toggle Wispr Flow hands-free dictation. Wispr has start and stop URLs but no
# toggle, so a state file remembers which comes next. Karabiner runs this when
# the D06 Pro mouse is clicked.
#
# If Wispr stops on its own (silence timeout, Esc), the state goes stale and
# the next click sends a harmless stop; the click after that starts again.
#
# On the home laptop, a start with a text field focused just passes through to
# Wispr. With nothing to type into, the click captures what is in front (Edge's
# page, or which window to screenshot) and opens mac/wispr_dialog.js for the
# dictation, set to send it to Larry with that. Sending goes through the Alfred
# Agent Send workflow's agent.py, as `w l p` (page) or `w l ssa` (window) does.
# With an Agent Send popup or any app's dialog on top, Wispr just starts.
# With the Wispr dialog already open, the click brings that one back to the
# front and dictates into it: there is only ever one.
#
# Clicks are serialized by a lock, and a click within DEBOUNCE_MS of the last
# one is dropped, so a double click or a bouncing switch can't start two
# dictations or flip the state twice.
#
# Every decision goes to ~/Library/Logs/wispr-hands-free.log; Agent Send keeps
# its own in ~/Library/Logs/agent-send.log.

clicked_ms=$(perl -MTime::HiRes=time -e 'printf "%d", time * 1000')
here=$(cd "$(dirname "$0")" && pwd)
state="$HOME/.cache/wispr-hands-free-on" # holds "dialog" or "passthrough": what this dictation is for
dialog_lock="$HOME/.cache/wispr-hands-free-dialog.pid" # "<osascript pid> <yabai space>" of the open dialog
log_file="$HOME/Library/Logs/wispr-hands-free.log"
click_lock="$HOME/.cache/wispr-hands-free.lock"  # a directory: mkdir is the atomic test-and-set
last_click="$HOME/.cache/wispr-hands-free-last-click" # ms of the last click handled
DEBOUNCE_MS=400
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.local/bin:$PATH"

now_ms() {
	perl -MTime::HiRes=time -e 'printf "%d", time * 1000'
}

# Each line carries the ms since the click, to see where time goes.
log() {
	printf '%s [%s] +%sms %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$$" "$(($(now_ms) - clicked_ms))" "$*" >>"$log_file"
}

focus_helper="$HOME/.cache/wispr_focus"

# "<bundle id>|<focused element role>|<focused window subrole>" of the front app.
# The compiled mac/wispr_focus.swift takes ~30 ms; System Events, the fallback
# when the helper can't build or has no Accessibility access, ~250 ms.
front_state() {
	if [ ! -x "$focus_helper" ] || [ "$here/wispr_focus.swift" -nt "$focus_helper" ]; then
		swiftc -O "$here/wispr_focus.swift" -o "$focus_helper" 2>>"$log_file" && log "built $focus_helper"
	fi
	"$focus_helper" 2>>"$log_file" && return
	log "focus helper failed (exit $?, reason above if it gave one); asking System Events"
	osascript -e 'tell application "System Events"
		set p to first process whose frontmost is true
		set b to bundle identifier of p
		set r to "none"
		set s to "none"
		try
			set r to role of (value of attribute "AXFocusedUIElement" of p)
		end try
		try
			set s to subrole of (value of attribute "AXFocusedWindow" of p)
		end try
		return b & "|" & r & "|" & s
	end tell' 2>>"$log_file"
}

dialog_open() {
	[ -e "$dialog_lock" ] && kill -0 "$(cut -d' ' -f1 "$dialog_lock")" 2>/dev/null
}

# Bring the open dialog to the front, so Wispr pastes into it.
raise_dialog() {
	read -r dialog_pid dialog_space <"$dialog_lock"
	osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $dialog_pid) to true" 2>>"$log_file"
}

# Take the click lock, or say why not. A lock older than a few seconds is
# left over from a click that died and is taken over.
take_click_lock() {
	mkdir "$click_lock" 2>/dev/null && return 0
	age=$(($(date +%s) - $(stat -f %m "$click_lock" 2>/dev/null || date +%s)))
	if [ "$age" -ge 5 ]; then
		log "click: taking over a lock left ${age}s ago"
		rm -rf "$click_lock"
		mkdir "$click_lock" 2>/dev/null && return 0
	fi
	return 1
}

release_click_lock() {
	rm -rf "$click_lock"
}

current_space() {
	yabai -m query --spaces --space 2>>"$log_file" | jq -r '.index // empty'
}

# Why to pass straight through to Wispr, or nothing when the dialog should open.
skip_reason() {
	if [ "$(scutil --get LocalHostName 2>/dev/null)" != "ig-mbp-home" ]; then
		echo "not the home laptop"
		return
	fi
	if pgrep -f 'ux.py center' >/dev/null; then
		echo "an Agent Send popup is up"
		return
	fi
	case "$window_subrole" in
	AXDialog | AXSystemDialog | AXFloatingWindow)
		echo "a dialog is on top ($window_subrole)"
		return
		;;
	esac
	case "$focus_role" in
	AXTextField | AXTextArea | AXComboBox)
		echo "a text field has focus ($focus_role)"
		return
		;;
	esac
}

agent_send_dir() {
	dirname "$(grep -l "com.idvorkin.agent-send" \
		"$HOME/Library/Application Support/Alfred/Alfred.alfredpreferences/workflows"/*/info.plist | head -1)"
}

# Edge's page as Agent Send's `l p` item arg, written to $1 at click time.
capture_page() {
	(cd "$(agent_send_dir)" && ./agent.py filter "l p -" 2>>"$log_file") |
		jq -c '.items[0].arg // empty | fromjson' >"$1"
	log "capture: page arg=$(cat "$1")"
}

# Send to Larry with the captured page (p) or a screenshot of the captured
# window (ssa). The ssa arg is built here as tree.py's _send_arg builds it,
# since the filter would ask yabai for the focused window after the dialog.
send_to_larry() {
	message=$1 kind=$2 window_id=$3 page_file=$4
	arg=""
	if [ "$kind" = p ]; then
		[ -s "$page_file" ] && arg=$(jq -c --arg m "$message" '.message = $m' "$page_file")
	elif [ -n "$window_id" ]; then
		arg=$(jq -cn --arg m "$message" --argjson w "$window_id" \
			'{do: "send", key: "l", kind: "ssa", message: $m, window: $w}')
	fi
	log "send: kind=$kind window=$window_id arg=$arg"
	if [ -z "$arg" ]; then
		printf '%s' "$message" | pbcopy
		log "send: nothing captured to send with; copied to the clipboard instead"
		osascript -e 'display notification "Could not send to Larry; copied to the clipboard" with title "Wispr"'
		return 1
	fi
	# The Cockpit files a send as Igor only when its client starts with "Alfred".
	(cd "$(agent_send_dir)" && LARRY_CLIENT="Alfred on Mac" ./agent.py run "$arg") >>"$log_file" 2>&1
	log "send: agent.py run exited $?"
}

dictate_into_dialog() {
	kind=$1 window_id=$2 attached=$3
	page_file=""
	capture_pid=""
	if [ "$kind" = p ]; then
		page_file=$(mktemp -t wispr-page)
		capture_page "$page_file" &
		capture_pid=$!
	fi
	reply_file=$(mktemp -t wispr-reply)
	# The dialog gets the state file: a key pressed while Wispr still listens
	# stops Wispr and waits for its text, rather than leaving it listening.
	osascript -l JavaScript "$here/wispr_dialog.js" "$attached" "$clicked_ms" "$state" >"$reply_file" 2>>"$log_file" &
	echo "$! $(current_space)" >"$dialog_lock"
	wait "$!"
	rm -f "$dialog_lock"
	[ -n "$capture_pid" ] && wait "$capture_pid"
	reply=$(cat "$reply_file")
	rm -f "$reply_file"
	button=$(printf '%s\n' "$reply" | head -1)
	text=$(printf '%s\n' "$reply" | tail -n +2)
	log "dialog: button=${button:-Cancel} chars=$(printf '%s' "$text" | wc -c | tr -d ' ')"
	if [ -n "$text" ]; then
		case "$button" in
		"Send to Larry") send_to_larry "$text" "$kind" "$window_id" "$page_file" ;;
		"Copy to Clipboard") printf '%s' "$text" | pbcopy ;;
		esac
	fi
	[ -n "$page_file" ] && rm -f "$page_file"
}

mkdir -p "$(dirname "$state")"
if ! take_click_lock; then
	log "click: ignored, another click is still being handled"
	exit 0
fi
trap release_click_lock EXIT
since_last=$((clicked_ms - $(cat "$last_click" 2>/dev/null || echo 0)))
if [ "$since_last" -lt "$DEBOUNCE_MS" ]; then
	log "click: ignored, ${since_last} ms after the last one"
	exit 0
fi
echo "$clicked_ms" >"$last_click"
log "click: state=$(cat "$state" 2>/dev/null || echo off)"

if [ -e "$state" ]; then
	mode=$(cat "$state" 2>/dev/null)
	rm -f "$state"
	if [ "$mode" = dialog ] && dialog_open; then
		# Wispr pastes where focus is when it stops. Make that the dialog, but
		# only on the space it opened on: elsewhere you are talking to something else.
		read -r dialog_pid dialog_space <"$dialog_lock"
		space=$(current_space)
		if [ "$space" = "$dialog_space" ]; then
			raise_dialog
			log "stop: brought the dialog to the front (space $space)"
		else
			log "stop: dialog is on space $dialog_space, you are on $space: focus left alone"
		fi
	fi
	log "stop"
	open -g "wispr-flow://stop-hands-free"
	exit 0
fi

# Wispr first, so dictation never waits on the checks below. The dialog, if
# one opens, comes up after Wispr's own window and so ends up in front.
open -g "wispr-flow://start-hands-free" &
touch "$state"
if dialog_open; then
	echo dialog >"$state"
	raise_dialog
	log "start: the Wispr dialog is already open: brought it to the front to dictate into"
	exit 0
fi
IFS='|' read -r front_app focus_role window_subrole <<EOF
$(front_state)
EOF
reason=$(skip_reason)
if [ -n "$reason" ]; then echo passthrough; else echo dialog; fi >"$state"
log "start: app=$front_app focus=$focus_role window=$window_subrole -> ${reason:-dialog}"
[ -n "$reason" ] && exit 0

window=$(yabai -m query --windows --window 2>>"$log_file")
window_id=$(printf '%s' "$window" | jq -r '.id // empty')
if [ "$front_app" = "com.microsoft.edgemac" ]; then
	kind=p
	attached="Edge page: $(printf '%s' "$window" | jq -r '.title // empty')"
else
	kind=ssa
	attached="screenshot of $(printf '%s' "$window" | jq -r '"\(.app): \(.title)"')"
	[ -n "$window_id" ] || attached="nothing: no front window to screenshot"
fi
log "capture: kind=$kind window=$window_id ($attached)"
# Backgrounded so the dialog waiting on you never holds up Karabiner, and
# after letting go of the click lock, so the next click is handled meanwhile.
release_click_lock
dictate_into_dialog "$kind" "$window_id" "$attached" </dev/null >/dev/null 2>&1 &
