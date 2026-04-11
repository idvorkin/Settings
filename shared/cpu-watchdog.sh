#!/bin/bash
# cpu-watchdog.sh — reactive in-VM CPU runaway catcher.
#
# Scans every INTERVAL seconds. When a user process exceeds THRESHOLD%
# instantaneous CPU (from `top -bn2 -d1`), attaches a cpulimit instance
# capped at LIMIT_PCT%. cpulimit uses -z so it auto-exits when the target
# dies. Pairs with the OrbStack Mac-side VM CPU cap for two-layer protection:
# the hypervisor sets the ceiling, this script catches individual runaways
# early so the whole VM never approaches the ceiling.
#
# Tunables via env:
#   CPU_WATCHDOG_LIMIT      per-process cap once fired (default 200 = 2 cores)
#   CPU_WATCHDOG_THRESHOLD  fire threshold (default 400 = 4 cores sustained)
#   CPU_WATCHDOG_INTERVAL   scan interval in seconds (default 30)
#   CPU_WATCHDOG_LOG        log file (default /tmp/cpu-watchdog.log)

set -u

LIMIT_PCT=${CPU_WATCHDOG_LIMIT:-200}
THRESHOLD=${CPU_WATCHDOG_THRESHOLD:-400}
INTERVAL=${CPU_WATCHDOG_INTERVAL:-30}
LOG=${CPU_WATCHDOG_LOG:-/tmp/cpu-watchdog.log}

log() {
    printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >> "$LOG"
}

# Do not throttle anything in this list — stopping them would wedge the VM
is_excluded() {
    case "$1" in
        cpulimit|tailscaled|etserver|sh|init|systemd|dolt|\
        "tmux:"*|kthreadd|kworker*|ksoftirqd*|migration*|rcu_*) return 0 ;;
    esac
    return 1
}

trap 'log "cpu-watchdog stopping (pid=$$)"; exit 0' TERM INT

log "cpu-watchdog starting (limit=${LIMIT_PCT}% threshold=${THRESHOLD}% interval=${INTERVAL}s pid=$$)"

while true; do
    # top -bn2 -d1 → second iteration has instantaneous CPU over 1s
    # -w512 prevents COMMAND truncation. awk concatenates cols 12..NF as COMMAND.
    /usr/bin/top -bn2 -d1 -w512 2>/dev/null \
        | awk '
            /^top - /{iter++}
            iter==2 && $1 ~ /^[0-9]+$/ {
                comm=""
                for (i=12; i<=NF; i++) comm = comm (i>12 ? " " : "") $i
                print $1, $9, comm
            }
          ' \
        | while read -r pid pcpu comm; do
            # bash can't compare floats; let awk do it
            over=$(awk -v a="$pcpu" -v t="$THRESHOLD" 'BEGIN{print (a+0 > t+0)}')
            [ "$over" = "1" ] || continue
            is_excluded "$comm" && continue
            # Already being throttled? (our cpulimit has `-p <pid>` in its args)
            if pgrep -f "cpulimit.*-p $pid " >/dev/null 2>&1; then continue; fi
            log "throttle pid=$pid comm=\"$comm\" cpu=${pcpu}% → cap ${LIMIT_PCT}%"
            cpulimit -l "$LIMIT_PCT" -p "$pid" -z -q >/dev/null 2>&1 &
        done
    sleep "$INTERVAL"
done
