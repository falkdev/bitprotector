#!/bin/bash
# tests/installation/scenarios/resilience/resilience-01-enospc.sh
# Scenario #1 — Fill mirror disk, verify mirror action fails, then succeeds after cleanup.

resilience_01_enospc() {
    ssh_vm '
set -euo pipefail
DB=/tmp/resilience-01.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/data

fallocate -l 120M /mnt/primary/data/payload.bin
bitprotector --db "${DB}" drives add r01 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 data/payload.bin

fallocate -l 2800M /mnt/mirror/filler.bin
if bitprotector --db "${DB}" files mirror 1; then
  echo "mirror unexpectedly succeeded with ENOSPC setup" >&2
  exit 1
fi

rm -f /mnt/mirror/filler.bin
bitprotector --db "${DB}" files mirror 1
cmp /mnt/primary/data/payload.bin /mnt/mirror/data/payload.bin
'
}
