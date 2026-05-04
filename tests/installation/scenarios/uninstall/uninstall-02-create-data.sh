#!/bin/bash
# tests/installation/scenarios/uninstall/uninstall-02-create-data.sh
# Scenario E — Create package-owned DB and backup data before purge.
# Bundle: uninstall. Assumes: package installed, service may be running.

uninstall_02_create_data() {
    ssh_vm '
set -euo pipefail
DB_PATH=/var/lib/bitprotector/bitprotector.db
BACKUP_DIR=/var/lib/bitprotector/backups/uninstall-test
NO_CFG=/tmp/bitprotector-missing-config.toml

sudo systemctl stop bitprotector || true
sudo install -d -m 0750 -o bitprotector -g bitprotector "${BACKUP_DIR}"

sudo bitprotector --config "${NO_CFG}" --db "${DB_PATH}" status >/dev/null

add_output=$(sudo bitprotector --config "${NO_CFG}" --db "${DB_PATH}" database add "${BACKUP_DIR}" 2>&1)
printf "%s\n" "${add_output}" | grep -q "Backup destination #"

run_output=$(sudo bitprotector --config "${NO_CFG}" --db "${DB_PATH}" database run 2>&1)
printf "%s\n" "${run_output}" | grep -Fq "[OK] Destination #"
printf "%s\n" "${run_output}" | grep -Eq "[0-9]+/[0-9]+ backups succeeded\."

sudo test -f "${DB_PATH}"
backup_count=$(sudo find "${BACKUP_DIR}" -maxdepth 1 -type f -name "bitprotector.db" | wc -l)
test "${backup_count}" -eq 1

# User-owned drive-like data (must survive purge).
sudo install -d -m 0755 -o testuser -g testuser /mnt/primary/docs
printf "keep-me-after-purge\n" > /mnt/primary/docs/keeper.txt
test -f /mnt/primary/docs/keeper.txt
'
}
