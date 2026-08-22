#!/bin/bash
# tests/installation/scenarios/upgrade/upgrade-04-paused-queue-persists.sh
# Scenario #30 — Paused sync queue state survives a package upgrade.

upgrade_04_paused_queue_persists() {
  local db="/mnt/bitprotector-db/db/upgrade-04.db"

  ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/upgrade-04.db

rm -f "${DB}"
mkdir -p /tmp/upg4/p1 /tmp/upg4/s1

for i in $(seq 1 3); do
  printf "upg4-%03d\n" "${i}" > "/tmp/upg4/p1/file-${i}.txt"
done

bitprotector --db "${DB}" drives add upg4-a /tmp/upg4/p1 /tmp/upg4/s1 --no-validate
for i in $(seq 1 3); do
  bitprotector --db "${DB}" files track 1 "file-${i}.txt" >/dev/null
done

bitprotector --db "${DB}" sync pause
if ! bitprotector --db "${DB}" sync list | grep -q "\[PAUSED\]"; then
  echo "Expected sync queue to report paused before upgrade" >&2
  exit 1
fi
' || return 1

    verify_sqlite "${db}"

    ssh_vm '
set -euo pipefail
  DB=/mnt/bitprotector-db/db/upgrade-04.db
source /etc/bitprotector-upgrade.env

sudo dpkg -i "/mnt/debpkg/${CURRENT_DEB_NAME}"
if ! sudo systemctl restart bitprotector; then
  sudo systemctl status bitprotector --no-pager -l || true
  sudo journalctl -u bitprotector -n 80 --no-pager || true
  exit 1
fi
for _ in $(seq 1 30); do
  if systemctl is-active --quiet bitprotector; then
    break
  fi
  sleep 1
done
if ! systemctl is-active --quiet bitprotector; then
  sudo systemctl status bitprotector --no-pager -l || true
  sudo journalctl -u bitprotector -n 80 --no-pager || true
  exit 1
fi

if ! bitprotector --db "${DB}" sync list | grep -q "\[PAUSED\]"; then
  echo "Expected sync queue to still report paused after upgrade" >&2
  exit 1
fi

process_output=$(bitprotector --db "${DB}" sync process)
if [[ "${process_output}" != "Processed 0 pending sync queue item(s)" ]]; then
  printf "%s\n" "${process_output}" >&2
  echo "Expected paused queue to remain a no-op for sync process after upgrade" >&2
  exit 1
fi

bitprotector --db "${DB}" sync resume
if bitprotector --db "${DB}" sync list | grep -q "\[PAUSED\]"; then
  echo "Expected sync queue to no longer report paused after resume" >&2
  exit 1
fi

process_output=$(bitprotector --db "${DB}" sync process)
if [[ "${process_output}" == "Processed 0 pending sync queue item(s)" ]]; then
  printf "%s\n" "${process_output}" >&2
  echo "Expected resumed queue to process the previously queued items" >&2
  exit 1
fi
' || return 1

    verify_sqlite "${db}"
}
