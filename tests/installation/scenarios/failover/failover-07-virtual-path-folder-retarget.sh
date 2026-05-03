#!/bin/bash
# tests/installation/scenarios/failover/failover-07-virtual-path-folder-retarget.sh
# Scenario #-- — Folder virtual paths retarget to active drive after planned failover.

failover_07_virtual_path_folder_retarget() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/failover-07.db
VROOT=/tmp/bitprotector-virtual/f07

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/* /mnt/replacement-primary/* /tmp/bitprotector-virtual/f07
mkdir -p /mnt/primary/docs /tmp/bitprotector-virtual

for i in $(seq 1 10); do
  printf "doc-%s\n" "${i}" > "/mnt/primary/docs/file-${i}.txt"
done

bitprotector --db "${DB}" drives add f07 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" folders add 1 docs --virtual-path "${VROOT}"
bitprotector --db "${DB}" folders scan 1
bitprotector --db "${DB}" folders mirror 1

for i in $(seq 1 10); do
  readlink -f "${VROOT}/file-${i}.txt" | grep -q "^/mnt/primary/"
done

bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary
for i in $(seq 1 10); do
  readlink -f "${VROOT}/file-${i}.txt" | grep -q "^/mnt/mirror/"
done

bitprotector --db "${DB}" drives replace assign 1 --role primary /mnt/replacement-primary --no-validate
bitprotector --db "${DB}" sync process
for i in $(seq 1 10); do
  readlink -f "${VROOT}/file-${i}.txt" | grep -q "^/mnt/replacement-primary/"
done
'
}
