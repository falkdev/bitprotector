#!/bin/bash
# tests/installation/scenarios/degraded-boot/degraded-boot-02-device-absent-at-boot.sh
# Scenario #5 — Boot with nofail absent-primary entry and ensure service + CLI still operate.

degraded_boot_02_device_absent_at_boot() {
    ssh_vm '
set -euo pipefail
DB=/tmp/degraded-02.db

rm -f "${DB}"
# nofail mountpoint exists but is not actually backed by a mounted device
! findmnt /mnt/absent-primary >/dev/null 2>&1 || true
mkdir -p /tmp/degraded-secondary2

bitprotector --db "${DB}" drives add d02 /mnt/absent-primary /tmp/degraded-secondary2 --no-validate
bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary

systemctl is-active bitprotector | grep -q "^active$"
bitprotector --db "${DB}" status | grep -qi "degraded"
'
}
