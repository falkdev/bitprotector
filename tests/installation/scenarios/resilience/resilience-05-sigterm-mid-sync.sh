#!/bin/bash
# tests/installation/scenarios/resilience/resilience-05-sigterm-mid-sync.sh
# Scenario #7 — SIGTERM during sync process should not corrupt queue state.

resilience_05_sigterm_mid_sync() {
    ssh_vm '
set -euo pipefail
DB=/tmp/resilience-05.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/bulk

for i in $(seq 1 300); do
  printf "file-%03d\n" "${i}" > "/mnt/primary/bulk/f-${i}.txt"
done

bitprotector --db "${DB}" drives add r05 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" folders add 1 bulk
bitprotector --db "${DB}" folders scan 1

bitprotector --db "${DB}" sync process &
pid=$!
sleep 1
kill -TERM "${pid}" 2>/dev/null || true
wait "${pid}" || true

# Queue remains processable and can finish.
bitprotector --db "${DB}" sync process
bitprotector --db "${DB}" sync list --status in_progress | grep -q "Total: 0"
'
}
