#!/bin/bash
# tests/installation/scenarios/failover/failover-04-both-corrupted.sh
# Scenario #-- — Corrupt both copies differently and assert BOTH_CORRUPTED is reported.

failover_04_both_corrupted() {
    ssh_vm '
set -euo pipefail
DB=/tmp/failover-04.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/docs /mnt/mirror/docs
printf "original\n" > /mnt/primary/docs/both.txt

bitprotector --db "${DB}" drives add f04 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 docs/both.txt
bitprotector --db "${DB}" files mirror 1

printf "primary-corrupted\n" > /mnt/primary/docs/both.txt
printf "mirror-corrupted\n" > /mnt/mirror/docs/both.txt

out=$(bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover 2>&1 || true)
echo "${out}" | grep -Eq "BOTH_CORRUPTED|both_corrupted"
'
}
