#!/bin/zsh -f
# Append one timestamped line per yabai event to ~/.cache/yabai/trace.log, so a layout glitch can be
# replayed afterwards: which window moved or resized, when, and to what frame.
# yabai's own debug_output (/tmp/yabai_$USER.out.log) has the event stream but no timestamps.
# Called by the trace_* signals in yabairc as `trace.sh <event>`; yabai passes ids in YABAI_* env vars.
# Read it with the yabai-debug skill in the alfred repo.

zmodload zsh/datetime zsh/stat

log=~/.cache/yabai/trace.log
mkdir -p ${log:h}

# Keep it bounded: one previous generation of ~5 MB.
if [[ -f $log ]] && zstat -A size +size $log && (( size[1] > 5000000 )); then
  mv -f $log $log.1
fi

event=$1
now=$EPOCHREALTIME
ts="$(strftime '%Y-%m-%d %H:%M:%S' ${now%.*}).${${now#*.}[1,3]}"

ids=()
for name in ${(ok)parameters[(I)YABAI_*]}; do
  ids+=("${name#YABAI_}=${(P)name}")
done

# Clients spawned from the service print a MallocStackLogging warning on stderr; drop it.
case $event in
  window_destroyed)
    detail="" ;;
  window_*)
    detail=$(yabai -m query --windows --window $YABAI_WINDOW_ID 2>/dev/null \
      | jq -c '{id, app, title: .title[:40], frame, space, floating: ."is-floating", visible: ."is-visible"}') ;;
  *)
    # Space, display and system events: the tiled windows on the now-focused space.
    detail=$(yabai -m query --windows --space 2>/dev/null \
      | jq -c '[.[] | select(."is-visible" and (."is-floating" | not)) | {id, app, frame}]') ;;
esac

print -r -- "$ts $event ${ids[*]} $detail" >> $log
