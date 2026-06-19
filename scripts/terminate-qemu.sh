#!/bin/bash
# scripts/terminate-qemu.sh
# Find all running QEMU processes and send SIGTERM to each of them.

set -euo pipefail

SELF_PID="$$"

mapfile -t QEMU_PIDS < <(
    ps -eo pid=,comm= | awk -v self_pid="${SELF_PID}" '
        $1 != self_pid && $2 ~ /^qemu/ { print $1 }
    '
)

if [[ ${#QEMU_PIDS[@]} -eq 0 ]]; then
    echo "No QEMU processes found."
    exit 0
fi

echo "Sending SIGTERM to ${#QEMU_PIDS[@]} QEMU process(es)..."
for pid in "${QEMU_PIDS[@]}"; do
    if kill -0 "${pid}" >/dev/null 2>&1; then
        kill -TERM "${pid}"
        echo "Sent SIGTERM to PID ${pid}"
    fi
done
