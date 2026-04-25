#!/bin/bash
# tests/installation/scenarios/failover/failover-03-bit-flip-auto-repair.sh
# Scenario #17 — Corrupt one byte on mirror and verify recovery succeeds.

failover_03_bit_flip_auto_repair() {
    ssh_vm '
set -euo pipefail
DB=/tmp/failover-03.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/docs /mnt/mirror/docs
printf "healthy before bit flip\n" > /mnt/primary/docs/flip.txt

bitprotector --db "${DB}" drives add f03 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 docs/flip.txt
bitprotector --db "${DB}" files mirror 1

python3 - <<'\''PY'\''
from pathlib import Path
p = Path('/mnt/mirror/docs/flip.txt')
with p.open('r+b') as f:
    f.seek(0)
    b = f.read(1)
    f.seek(0)
    f.write(bytes([b[0] ^ 0xFF]))
PY

bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
bitprotector --db "${DB}" sync process
cmp /mnt/primary/docs/flip.txt /mnt/mirror/docs/flip.txt
'
}
