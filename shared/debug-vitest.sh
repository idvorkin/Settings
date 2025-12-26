#!/bin/bash
# Debug and optionally kill orphaned vitest processes

set -euo pipefail

KILL_MODE=false
if [[ "${1:-}" == "--kill" ]]; then
    KILL_MODE=true
fi

PIDS=$(pgrep -f "node.*vitest" 2>/dev/null || true)

if [[ -z "$PIDS" ]]; then
    echo "No vitest processes found."
    exit 0
fi

echo "========================================"
echo "VITEST PROCESS DEBUG REPORT"
echo "Timestamp: $(date -Iseconds)"
echo "========================================"
echo

ORPHANS=()

for pid in $PIDS; do
    # Skip if process disappeared
    [[ -d "/proc/$pid" ]] || continue

    # Get process info
    cwd=$(readlink "/proc/$pid/cwd" 2>/dev/null || echo "N/A")
    cmdline=$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || echo "N/A")
    ppid=$(awk '{print $4}' "/proc/$pid/stat" 2>/dev/null || echo "N/A")
    cpu=$(/usr/bin/ps -o %cpu= -p "$pid" 2>/dev/null || echo "N/A")
    mem=$(/usr/bin/ps -o %mem= -p "$pid" 2>/dev/null || echo "N/A")
    runtime=$(/usr/bin/ps -o etime= -p "$pid" 2>/dev/null || echo "N/A")

    # Check if orphaned (ppid=1)
    is_orphan="no"
    if [[ "$ppid" == "1" ]]; then
        is_orphan="YES"
        ORPHANS+=("$pid")
    fi

    echo "=== PID $pid ==="
    echo "  Orphaned:  $is_orphan"
    echo "  PPID:      $ppid"
    echo "  CPU:       ${cpu}%"
    echo "  Memory:    ${mem}%"
    echo "  Runtime:   $runtime"
    echo "  CWD:       $cwd"
    echo "  Command:   $cmdline"

    # Show open files (test files being processed)
    echo "  Open test files:"
    ls -l "/proc/$pid/fd" 2>/dev/null | grep -E '\.(test|spec)\.(ts|js|tsx|jsx)' | head -5 | while read -r line; do
        echo "    $line"
    done || echo "    (none found)"

    # Show what syscall it's stuck on
    if command -v timeout &>/dev/null; then
        echo "  Current syscall:"
        syscall=$(timeout 1 cat "/proc/$pid/syscall" 2>/dev/null | awk '{print $1}' || echo "N/A")
        echo "    $syscall"
    fi

    echo
done

echo "========================================"
echo "SUMMARY"
echo "========================================"
echo "Total vitest processes: $(echo "$PIDS" | wc -w)"
echo "Orphaned processes:     ${#ORPHANS[@]}"

if [[ ${#ORPHANS[@]} -gt 0 ]]; then
    echo "Orphan PIDs:            ${ORPHANS[*]}"

    if $KILL_MODE; then
        echo
        echo "Killing orphaned processes..."
        kill "${ORPHANS[@]}" 2>/dev/null && echo "Done." || echo "Some processes already exited."
    else
        echo
        echo "Run with --kill to terminate orphaned processes:"
        echo "  $0 --kill"
    fi
fi
