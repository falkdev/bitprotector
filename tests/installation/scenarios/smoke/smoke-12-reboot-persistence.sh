#!/bin/bash
# tests/installation/scenarios/smoke/smoke-12-reboot-persistence.sh
# Scenario #10 — Reboot persistence: tracked state survives reboot and service auto-starts.
# Bundle: smoke. Assumes: package installed, service enabled, CLI available.

smoke_12_reboot_persistence() {
    local db="/mnt/bitprotector-db/db/smoke-reboot.db"

    ssh_vm '
set -euo pipefail
DB="/mnt/bitprotector-db/db/smoke-reboot.db"

mkdir -p /tmp/reboot-primary /tmp/reboot-secondary
printf "persist-me\n" > /tmp/reboot-primary/persist.txt

bitprotector --db "${DB}" drives add reboot-pair /tmp/reboot-primary /tmp/reboot-secondary --no-validate
bitprotector --db "${DB}" files track 1 persist.txt
bitprotector --db "${DB}" files show 1 | grep -q "persist.txt"
'

    # Trigger reboot and expect SSH disconnect during reboot transition.
    ssh_vm 'sudo reboot' || true

    wait_for_reboot_and_ssh 240

ssh_vm "
set -euo pipefail
DB='${db}'
systemctl is-active bitprotector | grep -q '^active$'
file_output=\$(bitprotector --db \"${db}\" files show 1)
echo \"\${file_output}\" | grep -q 'persist.txt'
drive_output=\$(bitprotector --db \"${db}\" drives show 1)
echo \"\${drive_output}\" | grep -q 'reboot-pair'
"
}
