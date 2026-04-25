#!/bin/bash
# tests/installation/scenarios/upgrade/upgrade-01-alpha1-to-current-with-live-data.sh
# Scenario #24 — Upgrade alpha1 install with live tracked data to current package.

upgrade_01_alpha1_to_current_with_live_data() {
    ssh_vm '
set -euo pipefail
DB=/tmp/upgrade-01.db
source /etc/bitprotector-upgrade.env

rm -f "${DB}"
mkdir -p /tmp/upg/p1 /tmp/upg/s1 /tmp/upg/p2 /tmp/upg/s2

for i in $(seq 1 100); do
  printf "up-%03d\n" "${i}" > "/tmp/upg/p1/file-${i}.txt"
done

bitprotector --db "${DB}" drives add upg-a /tmp/upg/p1 /tmp/upg/s1 --no-validate
bitprotector --db "${DB}" drives add upg-b /tmp/upg/p2 /tmp/upg/s2 --no-validate
for i in $(seq 1 100); do
  bitprotector --db "${DB}" files track 1 "file-${i}.txt" >/dev/null
done

bitprotector --db "${DB}" sync process
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover

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

files_output=$(bitprotector --db "${DB}" files list --per-page 100)
count=$(printf "%s\n" "${files_output}" | grep -c "file-" || true)
if [[ "${count}" -lt 100 ]]; then
  printf "%s\n" "${files_output}" >&2
  echo "Expected at least 100 tracked files after upgrade, got ${count}" >&2
  exit 1
fi
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
'
}
