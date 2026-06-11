#!/bin/bash
# tests/installation/scenarios/resilience/resilience-07-auto-restart-after-panic.sh
# Scenario #9 — Crash the service process and verify systemd restarts it.

resilience_07_auto_restart_after_panic() {
    local since
    since="$(ssh_vm 'date -d "10 seconds ago" "+%Y-%m-%d %H:%M:%S"')"

    ssh_vm '
set -euo pipefail
sudo systemctl is-active bitprotector | grep -q "^active$"
pid=$(systemctl show -p MainPID --value bitprotector)
[[ "${pid}" =~ ^[1-9][0-9]*$ ]]
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

    # Probe for crash evidence in unit logs across distros/journald variants.
    if ! ssh_vm "sudo journalctl -u bitprotector --since '${since}' --no-pager -q 2>/dev/null \
      | grep -Eq 'status=11|SEGV|code=(dumped|killed)'"; then
        echo "WARN: SIGSEGV crash token not found in journal; restart check already passed" >&2
    fi

    # Keep expected crash records out of the final journal error scrape.
    mkdir -p "${WORKDIR:-.}"
    printf '%s\n' "SEGV" >> "${WORKDIR}/expected-journal-patterns.txt"
}
