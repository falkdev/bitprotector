#!/bin/bash
# tests/installation/scenarios/smoke/smoke-13-database-backup-repair-restore.sh
# Scenario: database backup canonical files, repair, and staged restore.

smoke_13_database_backup_repair_restore() {
    ssh_vm '
set -euo pipefail
PRIMARY=/tmp/bitprotector-db-backup-primary
SECONDARY=/tmp/bitprotector-db-backup-secondary
sudo rm -rf "$PRIMARY" "$SECONDARY"
sudo mkdir -p "$PRIMARY" "$SECONDARY"

sudo bitprotector database add "$PRIMARY" --drive-label smoke-primary
sudo bitprotector database add "$SECONDARY" --drive-label smoke-secondary
sudo bitprotector database run

test -f "$PRIMARY/bitprotector.db"
test -f "$SECONDARY/bitprotector.db"
test "$(find "$PRIMARY" -maxdepth 1 -name bitprotector.db | wc -l)" -eq 1
test "$(find "$SECONDARY" -maxdepth 1 -name bitprotector.db | wc -l)" -eq 1

printf "not sqlite" | sudo tee "$PRIMARY/bitprotector.db" >/dev/null
sudo bitprotector database check-integrity | grep -qi repaired

sudo python3 - "$PRIMARY/bitprotector.db" <<'"'"'PY'"'"'
import sqlite3, sys
conn = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
assert conn.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
PY

sudo bitprotector database restore "$PRIMARY/bitprotector.db"
sudo systemctl restart bitprotector

for _ in $(seq 1 20); do
    if curl -sk https://127.0.0.1:8443/api/v1/health | jq -e ".status == \"ok\"" >/dev/null 2>&1; then
        break
    fi
    sleep 1
done
curl -sk https://127.0.0.1:8443/api/v1/health | jq -e ".status == \"ok\""
sudo bitprotector status >/dev/null
sudo bitprotector database list | grep -q "$PRIMARY"
'
}
