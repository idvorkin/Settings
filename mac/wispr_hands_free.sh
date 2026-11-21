#!/bin/sh
# Toggle Wispr Flow hands-free dictation. Wispr has start and stop URLs but no
# toggle, so a state file remembers which comes next. Karabiner runs this when
# the D06 Pro mouse is clicked.
#
# If Wispr stops on its own (silence timeout, Esc), the state goes stale and
# the next click sends a harmless stop; the click after that starts again.

state="$HOME/.cache/wispr-hands-free-on"

if [ -e "$state" ]; then
	rm -f "$state"
	open -g "wispr-flow://stop-hands-free"
else
	mkdir -p "$(dirname "$state")"
	touch "$state"
	open -g "wispr-flow://start-hands-free"
fi
