#!/bin/bash
# tests/installation/scenarios/scale/scale-01-100k-real-files.sh
# Scenario #21 — 100k file scan/mirror/integrity timing capture.

scale_01_100k_real_files() {
    ssh_vm '
set -euo pipefail
DB=/tmp/scale-01.db
RESULTS=/tmp/scale-results.txt

rm -f "${DB}" "${RESULTS}"
rm -rf /mnt/scale/docs /mnt/scale-mirror/docs
mkdir -p /mnt/scale/docs /mnt/scale-mirror/docs

echo "=== guest diagnostics ==="
grep -E "^(NAME|VERSION)=" /etc/os-release
uname -r
nproc
free -h
df -h /mnt/scale
findmnt /mnt/scale || true
echo "=== end diagnostics ==="

echo "=== phase: generation start ==="
start_gen=$(date +%s)
for i in $(seq 1 100000); do
  printf "x\n" > "/mnt/scale/docs/file-${i}.txt"
done
end_gen=$(date +%s)
echo "gen_seconds=$((end_gen-start_gen))" >> "${RESULTS}"
echo "=== phase: generation done $((end_gen-start_gen))s ==="

bitprotector --db "${DB}" drives add scale /mnt/scale /mnt/scale-mirror --no-validate
bitprotector --db "${DB}" folders add 1 docs

echo "=== phase: scan start ==="
start_scan=$(date +%s)
bitprotector --db "${DB}" folders scan 1
end_scan=$(date +%s)
echo "scan_seconds=$((end_scan-start_scan))" >> "${RESULTS}"
echo "=== phase: scan done $((end_scan-start_scan))s ==="

echo "=== phase: sync start ==="
start_sync=$(date +%s)
bitprotector --db "${DB}" sync process
end_sync=$(date +%s)
echo "sync_seconds=$((end_sync-start_sync))" >> "${RESULTS}"
echo "=== phase: sync done $((end_sync-start_sync))s ==="

echo "=== phase: integrity start ==="
start_integrity=$(date +%s)
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
end_integrity=$(date +%s)
echo "integrity_seconds=$((end_integrity-start_integrity))" >> "${RESULTS}"
echo "=== phase: integrity done $((end_integrity-start_integrity))s ==="
'
}
