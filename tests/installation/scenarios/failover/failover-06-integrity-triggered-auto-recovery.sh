#!/bin/bash
# tests/installation/scenarios/failover/failover-06-integrity-triggered-auto-recovery.sh
# Scenario #-- — Corrupt primary copy and recover from mirror via integrity check.

failover_06_integrity_triggered_auto_recovery() {
    ssh_vm '
set -euo pipefail
DB=/tmp/failover-06.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/docs /mnt/mirror/docs
printf "recover-me\n" > /mnt/primary/docs/recover.txt

bitprotector --db "${DB}" drives add f06 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 docs/recover.txt
bitprotector --db "${DB}" files mirror 1

printf "primary-bad\n" > /mnt/primary/docs/recover.txt
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
cmp /mnt/primary/docs/recover.txt /mnt/mirror/docs/recover.txt
'
}
