#!/bin/bash
# tests/installation/scenarios/failover/failover-12-qmp-hot-remove-secondary.sh
# Scenario #-- — Hot-remove secondary disk and verify degraded integrity signal.

failover_12_qmp_hot_remove_secondary() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-12.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/docs /mnt/mirror/docs
printf "secondary-hot-remove\n" > /mnt/primary/docs/secondary.txt

bitprotector --db "${DB}" drives add f12 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 docs/secondary.txt
bitprotector --db "${DB}" files mirror 1
'

    qmp_device_del "dev-mirror"
    sleep 5

    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-12.db
out=$(bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover 2>&1 || true)
echo "${out}" | grep -Eq "SECONDARY_DRIVE_UNAVAILABLE|MIRROR_MISSING|need attention"
bitprotector --db "${DB}" drives show 1 | grep -q "Active Role:     primary"
'
}
