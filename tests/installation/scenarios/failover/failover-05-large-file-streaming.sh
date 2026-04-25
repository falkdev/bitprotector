#!/bin/bash
# tests/installation/scenarios/failover/failover-05-large-file-streaming.sh
# Scenario #-- — Large-file mirror/integrity path with RSS sanity check.

failover_05_large_file_streaming() {
    ssh_vm '
set -euo pipefail
DB=/tmp/failover-05.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/large

# Keep runtime practical in CI while still exercising large-file code paths.
fallocate -l 200M /mnt/primary/large/blob.bin

bitprotector --db "${DB}" drives add f05 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 large/blob.bin
bitprotector --db "${DB}" files mirror 1
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover

pid=$(pidof bitprotector | awk "{print \$1}")
if [[ -n "${pid}" ]]; then
  rss_kb=$(awk "/VmRSS/ { print \$2 }" "/proc/${pid}/status")
  test -n "${rss_kb}"
  # Bound is intentionally lenient in VM CI environments.
  [[ "${rss_kb}" -lt 350000 ]]
fi
'
}
