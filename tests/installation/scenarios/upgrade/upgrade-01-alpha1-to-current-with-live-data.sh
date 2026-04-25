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
sudo systemctl restart bitprotector
sleep 3
systemctl is-active bitprotector | grep -q "^active$"

count=$(bitprotector --db "${DB}" files list | grep -c "file-" || true)
[[ "${count}" -ge 100 ]]
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
'
}
