#!/bin/bash
# tests/installation/scenarios/resilience/resilience-07-auto-restart-after-panic.sh
# Scenario #9 — Crash the service process and verify systemd restarts it.

resilience_07_auto_restart_after_panic() {
    local since
    since="$(date -Iseconds)"

    ssh_vm '
set -euo pipefail
sudo systemctl is-active bitprotector | grep -q "^active$"
pid=$(pidof bitprotector | awk "{print \$1}")
[[ -n "${pid}" ]]
sudo kill -SEGV "${pid}"
'

    ssh_vm '
set -euo pipefail
for _ in $(seq 1 20); do
  if systemctl is-active bitprotector | grep -q "^active$"; then
    exit 0
  fi
  sleep 1
done
echo "bitprotector did not return to active state after SIGSEGV" >&2
exit 1
'

    # Keep this expected crash out of the final journal error scrape.
    expect_journal_error "${since}" "code=dumped"
}
