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
# With a dialog already on top (this one, an Agent Send popup, any app's
# dialog), Wispr just starts.
#
# Every decision goes to ~/Library/Logs/wispr-hands-free.log; Agent Send keeps
# its own in ~/Library/Logs/agent-send.log.

clicked_ms=$(perl -MTime::HiRes=time -e 'printf "%d", time * 1000')
here=$(cd "$(dirname "$0")" && pwd)
state="$HOME/.cache/wispr-hands-free-on" # holds "dialog" or "passthrough": what this dictation is for
dialog_lock="$HOME/.cache/wispr-hands-free-dialog.pid" # "<osascript pid> <yabai space>" of the open dialog
log_file="$HOME/Library/Logs/wispr-hands-free.log"
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
	log "focus helper failed; asking System Events"
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

current_space() {
	yabai -m query --spaces --space 2>>"$log_file" | jq -r '.index // empty'
}

# Why to pass straight through to Wispr, or nothing when the dialog should open.
skip_reason() {
	if [ "$(scutil --get LocalHostName 2>/dev/null)" != "ig-mbp-home" ]; then
		echo "not the home laptop"
		return
	fi
	if dialog_open; then
		echo "the Wispr dialog is already open"
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
	osascript -l JavaScript "$here/wispr_dialog.js" "$attached" "$clicked_ms" >"$reply_file" 2>>"$log_file" &
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

if [ -e "$state" ]; then
	mode=$(cat "$state" 2>/dev/null)
	rm -f "$state"
	if [ "$mode" = dialog ] && dialog_open; then
		# Wispr pastes where focus is when it stops. Make that the dialog, but
		# only on the space it opened on: elsewhere you are talking to something else.
		read -r dialog_pid dialog_space <"$dialog_lock"
		space=$(current_space)
		if [ "$space" = "$dialog_space" ]; then
			osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $dialog_pid) to true" 2>>"$log_file"
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
mkdir -p "$(dirname "$state")"
touch "$state"
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
# Backgrounded so the dialog waiting on you never holds up Karabiner.
dictate_into_dialog "$kind" "$window_id" "$attached" </dev/null >/dev/null 2>&1 &
