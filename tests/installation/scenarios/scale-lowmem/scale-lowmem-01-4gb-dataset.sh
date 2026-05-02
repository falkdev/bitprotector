#!/bin/bash
# tests/installation/scenarios/scale-lowmem/scale-lowmem-01-4gb-dataset.sh
# Scenario #23 — 4GB dataset processing under 1GB RAM guest.

scale_lowmem_01_4gb_dataset() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/scale-lowmem-01.db

rm -f "${DB}"
rm -rf /mnt/primary/lowmem /mnt/mirror/lowmem
mkdir -p /mnt/primary/lowmem /mnt/mirror/lowmem

for i in $(seq 1 8); do
  fallocate -l 512M "/mnt/primary/lowmem/chunk-${i}.bin"
done

bitprotector --db "${DB}" drives add lowmem /mnt/primary /mnt/mirror --no-validate
for i in $(seq 1 8); do
  bitprotector --db "${DB}" files track 1 "lowmem/chunk-${i}.bin"
done

bitprotector --db "${DB}" sync process
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover

! sudo dmesg | grep -q "Killed process.*bitprotector"
pid=$(pidof bitprotector | awk "{print \$1}" || true)
if [[ -n "${pid}" ]]; then
  rss_kb=$(awk "/VmRSS/ { print \$2 }" "/proc/${pid}/status")
  [[ "${rss_kb}" -lt 300000 ]]
fi
'
}
