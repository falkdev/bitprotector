#!/bin/bash
# tests/installation/scenarios/failover/failover-01-planned.sh
# Scenario E — Planned primary failover and replacement rebuild.
# Bundle: failover. Assumes: 4 virtio disks mounted at /mnt/primary, /mnt/mirror,
#   /mnt/replacement-primary, /mnt/replacement-secondary; service running.

failover_01_planned() {
    ssh_vm '
set -euo pipefail
DB=/tmp/failover.db
VIRTUAL_FILE=/tmp/bitprotector-virtual/docs/report.txt

mkdir -p /mnt/primary/docs
printf "before failover\n" > /mnt/primary/docs/report.txt

bitprotector --db "${DB}" drives add lab /mnt/primary /mnt/mirror
bitprotector --db "${DB}" files track 1 docs/report.txt
bitprotector --db "${DB}" files mirror 1
bitprotector --db "${DB}" folders add 1 docs
bitprotector --db "${DB}" virtual-paths set 1 "${VIRTUAL_FILE}"

readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/primary/"
cat "${VIRTUAL_FILE}" | grep -q "before failover"

bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary
bitprotector --db "${DB}" drives show 1 | grep -q "Active Role:     secondary"
readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/mirror/"

printf "after planned failover\n" >> "${VIRTUAL_FILE}"
bitprotector --db "${DB}" folders scan 1
bitprotector --db "${DB}" files show 1 | grep -q "Mirrored:      no"

bitprotector --db "${DB}" drives replace assign 1 --role primary /mnt/replacement-primary --no-validate
bitprotector --db "${DB}" sync process

test -f /mnt/replacement-primary/docs/report.txt
diff -u "${VIRTUAL_FILE}" /mnt/replacement-primary/docs/report.txt
readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/replacement-primary/"
'
}
