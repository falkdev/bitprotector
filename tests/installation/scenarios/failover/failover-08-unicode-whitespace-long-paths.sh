#!/bin/bash
# tests/installation/scenarios/failover/failover-08-unicode-whitespace-long-paths.sh
# Scenario #18 — Unicode, whitespace, and long file names mirror/integrity sanity.

failover_08_unicode_whitespace_long_paths() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-08.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/u /mnt/mirror/u

longbase=$(printf "l%.0s" {1..220})
printf "unicode\n" > "/mnt/primary/u/こんにちは.txt"
printf "spaces\n" > "/mnt/primary/u/double  space.txt"
printf "long\n" > "/mnt/primary/u/${longbase}.txt"

bitprotector --db "${DB}" drives add f08 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 "u/こんにちは.txt"
bitprotector --db "${DB}" files track 1 "u/double  space.txt"
bitprotector --db "${DB}" files track 1 "u/${longbase}.txt"
bitprotector --db "${DB}" files mirror 1
bitprotector --db "${DB}" files mirror 2
bitprotector --db "${DB}" files mirror 3
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
'
}
