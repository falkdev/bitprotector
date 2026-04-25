#!/bin/bash
# tests/installation/scenarios/resilience/resilience-02-readonly-mirror.sh
# Scenario #2 — Remount mirror read-only, fail cleanly, then recover.

resilience_02_readonly_mirror() {
    ssh_vm '
set -euo pipefail
DB=/tmp/resilience-02.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/data
printf "readonly-case\n" > /mnt/primary/data/ro.txt

bitprotector --db "${DB}" drives add r02 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" files track 1 data/ro.txt

sudo mount -o remount,ro /mnt/mirror
if bitprotector --db "${DB}" files mirror 1; then
  echo "mirror unexpectedly succeeded on read-only mirror" >&2
  exit 1
fi
sudo mount -o remount,rw /mnt/mirror

bitprotector --db "${DB}" files mirror 1
cmp /mnt/primary/data/ro.txt /mnt/mirror/data/ro.txt
'
}
