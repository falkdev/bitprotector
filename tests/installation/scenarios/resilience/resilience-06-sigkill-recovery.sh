#!/bin/bash
# tests/installation/scenarios/resilience/resilience-06-sigkill-recovery.sh
# Scenario #8 — SIGKILL during sync process, then rerun to completion.

resilience_06_sigkill_recovery() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/resilience-06.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/bulk

for i in $(seq 1 300); do
  printf "kill-%03d\n" "${i}" > "/mnt/primary/bulk/k-${i}.txt"
done

bitprotector --db "${DB}" drives add r06 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" folders add 1 bulk
bitprotector --db "${DB}" folders scan 1

bitprotector --db "${DB}" sync process &
pid=$!
sleep 1
kill -9 "${pid}" 2>/dev/null || true
wait "${pid}" || true

bitprotector --db "${DB}" sync process
bitprotector --db "${DB}" sync list --status pending | grep -q "Total: 0"
bitprotector --db "${DB}" sync list --status in_progress | grep -q "Total: 0"
'
}
