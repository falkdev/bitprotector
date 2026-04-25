#!/bin/bash
# tests/installation/scenarios/smoke/smoke-07-journald-integration.sh
# Scenario #11 — journald integration: restarting the service yields journal
# entries for the unit.
# Bundle: smoke. Assumes: service running, journald active.

smoke_07_journald_integration() {
    ssh_vm '
set -euo pipefail
SINCE=$(date -Iseconds)
# Force a service lifecycle event that systemd should log to journald.
sudo systemctl restart bitprotector
sudo systemctl is-active bitprotector >/dev/null
sleep 2
# journalctl must show at least one line from the bitprotector unit
lines=$(sudo journalctl -u bitprotector --since "${SINCE}" --no-pager -q 2>/dev/null | wc -l)
[ "${lines}" -ge 1 ] || { echo "No journald output from bitprotector unit since ${SINCE}" >&2; exit 1; }
'
}
