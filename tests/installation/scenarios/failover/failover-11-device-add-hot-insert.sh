#!/bin/bash
# tests/installation/scenarios/failover/failover-11-device-add-hot-insert.sh
# Scenario #27 — Hot-remove then hot-add replacement-primary device and complete assignment.

failover_11_device_add_hot_insert() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-11.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/* /mnt/replacement-primary/*
mkdir -p /mnt/primary/docs
printf "hot-insert\n" > /mnt/primary/docs/hot.txt

bitprotector --db "${DB}" drives add f11 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 docs/hot.txt
bitprotector --db "${DB}" files mirror 1
bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary
'

    qmp_device_del "dev-replacement-primary"
    sleep 3

    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-11.db
if bitprotector --db "${DB}" drives replace assign 1 --role primary /does-not-exist 2>/tmp/f11-assign.err; then
  echo "assign unexpectedly succeeded on missing path" >&2
  exit 1
fi
'

    qmp_device_add '{"driver":"virtio-blk-pci","drive":"drive-replacement-primary","id":"dev-replacement-primary","serial":"bpreplprimary"}'
    sleep 3

    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-11.db
sudo mount -a
bitprotector --db "${DB}" drives replace assign 1 --role primary /mnt/replacement-primary --no-validate
bitprotector --db "${DB}" sync process
test -f /mnt/replacement-primary/docs/hot.txt
'
}
