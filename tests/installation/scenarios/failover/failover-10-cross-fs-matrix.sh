#!/bin/bash
# tests/installation/scenarios/failover/failover-10-cross-fs-matrix.sh
# Scenario #20 — Cross-filesystem pair (xfs/ext4) mirror/integrity/recovery path.

failover_10_cross_fs_matrix() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-10.db

rm -f "${DB}"
rm -rf /mnt/replacement-primary/* /mnt/mirror/*
mkdir -p /mnt/replacement-primary/matrix /mnt/mirror/matrix
printf "cross-fs\n" > /mnt/replacement-primary/matrix/file.txt

bitprotector --db "${DB}" drives add f10 /mnt/replacement-primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 matrix/file.txt
bitprotector --db "${DB}" files mirror 1

# Corrupt mirror and recover to exercise cross-FS recovery path.
printf "mirror-bad\n" > /mnt/mirror/matrix/file.txt
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
cmp /mnt/replacement-primary/matrix/file.txt /mnt/mirror/matrix/file.txt
'
}
