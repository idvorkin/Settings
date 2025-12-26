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

    # Get process info - use ps for PPID to avoid parsing issues with /proc/stat
    # (process names like "vitest 1" contain spaces which break awk field parsing)
    cwd=$(readlink "/proc/$pid/cwd" 2>/dev/null || echo "N/A")
    cmdline=$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || echo "N/A")
    ppid=$(/usr/bin/ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ' || echo "N/A")
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
    test_files_found=false
    while IFS= read -r link; do
        if [[ "$link" =~ \.(test|spec)\.(ts|js|tsx|jsx)$ ]]; then
            echo "    $link"
            test_files_found=true
        fi
    done < <(find "/proc/$pid/fd" -type l -exec readlink {} \; 2>/dev/null | head -20)
    if ! $test_files_found; then
        echo "    (none in open file descriptors)"
    fi

    # Look for test files in memory maps (loaded JS/TS files)
    echo "  Test files in memory maps:"
    maps_found=false
    if [[ -r "/proc/$pid/maps" ]]; then
        while IFS= read -r line; do
            echo "    $line"
            maps_found=true
        done < <(grep -oE '/[^ ]+\.(test|spec)\.(ts|js|tsx|jsx)' "/proc/$pid/maps" 2>/dev/null | sort -u | head -10)
    fi
    if ! $maps_found; then
        echo "    (none found)"
    fi

    # Check node environment for test context
    echo "  Environment hints:"
    env_found=false
    if [[ -r "/proc/$pid/environ" ]]; then
        while IFS= read -r -d '' env_var; do
            case "$env_var" in
                VITEST_*|TEST_*|NODE_OPTIONS*|VITE_*)
                    echo "    $env_var"
                    env_found=true
                    ;;
            esac
        done < "/proc/$pid/environ" 2>/dev/null
    fi
    if ! $env_found; then
        echo "    (no VITEST_/TEST_ env vars)"
    fi

    # Show what syscall it's stuck on
    if [[ -r "/proc/$pid/syscall" ]]; then
        echo "  Current syscall:"
        syscall=$(timeout 1 cat "/proc/$pid/syscall" 2>/dev/null | awk '{print $1}' || echo "N/A")
        echo "    $syscall"
    fi

    # Check if node inspector is available
    echo "  Node inspector:"
    if [[ -r "/proc/$pid/cmdline" ]] && grep -q "\-\-inspect" "/proc/$pid/cmdline" 2>/dev/null; then
        echo "    Inspector enabled in cmdline"
    else
        echo "    Not enabled (send SIGUSR1 to enable: kill -SIGUSR1 $pid)"
    fi

    # Show stack trace hint
    echo "  Debug tips:"
    echo "    strace -p $pid                    # See syscalls"
    echo "    kill -SIGUSR1 $pid                # Enable node inspector"
    echo "    node --inspect -p $pid 2>/dev/null  # Attach debugger"

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
