#!/bin/bash
# tests/installation/scenarios/uninstall/uninstall-04-purge-preserves-user-drive-data.sh
# Scenario #26 — Package purge must not remove user-managed drive content.
# Bundle: uninstall. Assumes uninstall-02 created /mnt/primary/docs/keeper.txt.

uninstall_04_purge_preserves_user_drive_data() {
    ssh_vm '
set -euo pipefail

test -f /mnt/primary/docs/keeper.txt
grep -q "keep-me-after-purge" /mnt/primary/docs/keeper.txt
'
}
