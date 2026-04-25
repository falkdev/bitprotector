#!/bin/bash
# tests/installation/scenarios/resilience/resilience-03-eacces-tracked-file.sh
# Scenario #3 — Permission-denied tracked file should surface and recover after chmod restore.

resilience_03_eacces_tracked_file() {
    ssh_vm '
set -euo pipefail
DB=/tmp/resilience-03.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/data
printf "perm-case\n" > /mnt/primary/data/perm.txt

bitprotector --db "${DB}" drives add r03 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 data/perm.txt
bitprotector --db "${DB}" files mirror 1

chmod 000 /mnt/primary/data/perm.txt
out=$(bitprotector --db "${DB}" integrity check 1 2>&1 || true)
echo "${out}" | grep -Eq "FAILED|MISSING|UNAVAILABLE|CORRUPTED|Permission denied|internal_error"

chmod 644 /mnt/primary/data/perm.txt
bitprotector --db "${DB}" integrity check 1 --recover
'
}
