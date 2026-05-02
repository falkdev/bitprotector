#!/bin/bash
# tests/installation/scenarios/failover/failover-09-two-pairs-one-disk.sh
# Scenario #19 — Two drive pairs on shared disks, failover one pair without affecting the other.

failover_09_two_pairs_one_disk() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-09.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/pairA /mnt/mirror/pairA /mnt/primary/pairB /mnt/mirror/pairB
printf "A\n" > /mnt/primary/pairA/a.txt
printf "B\n" > /mnt/primary/pairB/b.txt

bitprotector --db "${DB}" drives add pairA /mnt/primary/pairA /mnt/mirror/pairA --no-validate
bitprotector --db "${DB}" drives add pairB /mnt/primary/pairB /mnt/mirror/pairB --no-validate
bitprotector --db "${DB}" files track 1 a.txt
bitprotector --db "${DB}" files track 2 b.txt
bitprotector --db "${DB}" files mirror 1
bitprotector --db "${DB}" files mirror 2

bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary
bitprotector --db "${DB}" drives show 1 | grep -q "Active Role:     secondary"
bitprotector --db "${DB}" drives show 2 | grep -q "Active Role:     primary"
'
}
