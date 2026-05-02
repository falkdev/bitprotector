#!/bin/bash
# tests/installation/scenarios/resilience/resilience-04-symlink-loop.sh
# Scenario #6 — Folder scan with symlink loop terminates instead of hanging.

resilience_04_symlink_loop() {
    ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/resilience-04.db

rm -f "${DB}"
rm -rf /mnt/primary/* /mnt/mirror/*
mkdir -p /mnt/primary/loopdir
ln -s . /mnt/primary/loopdir/loop
printf "loop-test\n" > /mnt/primary/loopdir/base.txt

bitprotector --db "${DB}" drives add r04 /mnt/primary /mnt/mirror --no-validate
bitprotector --db "${DB}" folders add 1 loopdir
if ! timeout 10 bitprotector --db "${DB}" folders scan 1; then
  code=$?
  if [[ "${code}" -eq 124 ]]; then
    echo "folders scan timed out under symlink loop" >&2
    exit 1
  fi
fi
'
}
