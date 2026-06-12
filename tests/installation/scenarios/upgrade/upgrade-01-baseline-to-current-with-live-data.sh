#!/bin/bash
# tests/installation/scenarios/upgrade/upgrade-01-baseline-to-current-with-live-data.sh
# Scenario #24 — Upgrade baseline install with live tracked data to current package.

upgrade_01_baseline_to_current_with_live_data() {
  local db="/mnt/bitprotector-db/db/upgrade-01.db"

  ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/upgrade-01.db

rm -f "${DB}"
mkdir -p /tmp/upg/p1 /tmp/upg/s1 /tmp/upg/p2 /tmp/upg/s2

for i in $(seq 1 100); do
  printf "up-%03d\n" "${i}" > "/tmp/upg/p1/file-${i}.txt"
done

bitprotector --db "${DB}" drives add upg-a /tmp/upg/p1 /tmp/upg/s1 --no-validate
bitprotector --db "${DB}" drives add upg-b /tmp/upg/p2 /tmp/upg/s2 --no-validate
for i in $(seq 1 100); do
  bitprotector --db "${DB}" files track 1 "file-${i}.txt" >/dev/null
done

bitprotector --db "${DB}" sync process
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
' || return 1

    verify_sqlite "${db}"

    ssh_vm '
set -euo pipefail
  DB=/mnt/bitprotector-db/db/upgrade-01.db
source /etc/bitprotector-upgrade.env

sudo dpkg -i "/mnt/debpkg/${CURRENT_DEB_NAME}"
if ! sudo systemctl restart bitprotector; then
  sudo systemctl status bitprotector --no-pager -l || true
  sudo journalctl -u bitprotector -n 80 --no-pager || true
  exit 1
fi
for _ in $(seq 1 30); do
  if systemctl is-active --quiet bitprotector; then
    break
  fi
  sleep 1
done
if ! systemctl is-active --quiet bitprotector; then
  sudo systemctl status bitprotector --no-pager -l || true
  sudo journalctl -u bitprotector -n 80 --no-pager || true
  exit 1
fi

sudo python3 - "${DB}" <<PY
import sqlite3
import sys

db_path = sys.argv[1]
conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)

required_columns = {
  "drive_pairs": {"id", "name", "primary_path", "secondary_path", "primary_media_type", "secondary_media_type"},
  "tracked_files": {"id", "drive_pair_id", "relative_path", "checksum", "file_size"},
  "sync_queue": {"id", "tracked_file_id", "action", "status"},
  "db_backup_config": {"id", "backup_path", "priority", "last_integrity_check", "last_integrity_status", "last_error"},
}

for table, required in required_columns.items():
  cols = {row[1] for row in conn.execute(f"PRAGMA table_info({table})")}
  missing = sorted(required - cols)
  if missing:
    sep = ", "
    raise SystemExit(f"missing columns for {table}: {sep.join(missing)}")

sync_queue_sql = conn.execute(
  "SELECT sql FROM sqlite_master WHERE type=? AND name=?",
  ("table", "sync_queue"),
).fetchone()
if not sync_queue_sql or "adopt_mirror" not in (sync_queue_sql[0] or ""):
  raise SystemExit("sync_queue schema missing adopt_mirror action support")

conn.close()
print("schema assertions passed for", db_path)
PY

files_output=$(bitprotector --db "${DB}" files list --per-page 100)
count=$(printf "%s\n" "${files_output}" | grep -c "file-" || true)
if [[ "${count}" -lt 100 ]]; then
  printf "%s\n" "${files_output}" >&2
  echo "Expected at least 100 tracked files after upgrade, got ${count}" >&2
  exit 1
fi

printf "post-upgrade\n" > /tmp/upg/p1/file-post-upgrade.txt
bitprotector --db "${DB}" files track 1 file-post-upgrade.txt >/dev/null
files_output=$(bitprotector --db "${DB}" files list --per-page 200)
post_count=$(printf "%s\n" "${files_output}" | grep -c "file-post-upgrade.txt" || true)
if [[ "${post_count}" -ne 1 ]]; then
  printf "%s\n" "${files_output}" >&2
  echo "Expected post-upgrade write-path track to persist" >&2
  exit 1
fi

bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
' || return 1

    verify_sqlite "${db}"
}
