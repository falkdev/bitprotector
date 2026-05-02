#!/bin/bash
# tests/installation/scenarios/degraded-boot/degraded-boot-01-fake-mount-point.sh
# Scenario #4 — Use a plain directory as primary path and verify degraded status can be surfaced.

degraded_boot_01_fake_mount_point() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/degraded-01.db

rm -f "${DB}"
sudo rm -rf /mnt/fake-primary
sudo install -d -m 0755 -o testuser -g testuser /mnt/fake-primary /mnt/fake-primary/docs
mkdir -p /tmp/degraded-secondary
printf "fake mount content\n" > /mnt/fake-primary/docs/fake.txt

bitprotector --db "${DB}" drives add d01 /mnt/fake-primary /tmp/degraded-secondary --no-validate
bitprotector --db "${DB}" files track 1 docs/fake.txt
bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary

bitprotector --db "${DB}" drives show 1 | grep -q "Primary State:   failed"
bitprotector --db "${DB}" status | grep -qi "degraded"
'
}
