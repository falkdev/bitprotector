#!/bin/bash
# tests/installation/scenarios/upgrade/upgrade-03-restore-path-compatibility.sh
# Scenario #26 — Restore path remains compatible after package upgrade.

upgrade_03_restore_path_compatibility() {
    local db="/mnt/bitprotector-db/db/upgrade-03.db"
    local restore_source="/tmp/upg3/restore-source.db"

  ssh_vm '
set -euo pipefail
DB=/mnt/bitprotector-db/db/upgrade-03.db
RESTORE_SOURCE=/tmp/upg3/restore-source.db

rm -f "${DB}" "${RESTORE_SOURCE}" "${DB}.restore-pending"
mkdir -p /tmp/upg3/p1 /tmp/upg3/s1

for i in $(seq 1 5); do
  printf "seed-%03d\n" "${i}" > "/tmp/upg3/p1/file-${i}.txt"
done

bitprotector --db "${DB}" drives add upg3-a /tmp/upg3/p1 /tmp/upg3/s1 --no-validate
for i in $(seq 1 5); do
  bitprotector --db "${DB}" files track 1 "file-${i}.txt" >/dev/null
done
bitprotector --db "${DB}" sync process
bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover

cp "${DB}" "${RESTORE_SOURCE}"
printf "new-after-snapshot\n" > /tmp/upg3/p1/file-after-snapshot.txt
bitprotector --db "${DB}" files track 1 file-after-snapshot.txt >/dev/null
' || return 1

    verify_sqlite "${db}"
    verify_sqlite "${restore_source}"

    ssh_vm '
set -euo pipefail
  DB=/mnt/bitprotector-db/db/upgrade-03.db
  RESTORE_SOURCE=/tmp/upg3/restore-source.db
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

bitprotector --db "${DB}" database restore "${RESTORE_SOURCE}"
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

files_output=$(bitprotector --db "${DB}" files list --per-page 200)
seed_count=$(printf "%s\n" "${files_output}" | grep -c "file-" || true)
if [[ "${seed_count}" -lt 5 ]]; then
  printf "%s\n" "${files_output}" >&2
  echo "Expected at least five baseline tracked files after restore apply" >&2
  exit 1
fi
if printf "%s\n" "${files_output}" | grep -q "file-after-snapshot.txt"; then
  printf "%s\n" "${files_output}" >&2
  echo "Restore apply failed: post-snapshot file still present" >&2
  exit 1
fi

printf "post-restore-track\n" > /tmp/upg3/p1/file-post-restore.txt
bitprotector --db "${DB}" files track 1 file-post-restore.txt >/dev/null
files_output=$(bitprotector --db "${DB}" files list --per-page 200)
post_count=$(printf "%s\n" "${files_output}" | grep -c "file-post-restore.txt" || true)
if [[ "${post_count}" -ne 1 ]]; then
  printf "%s\n" "${files_output}" >&2
  echo "Expected restore-path write assertion to succeed" >&2
  exit 1
fi

bitprotector --db "${DB}" integrity check-all --drive-id 1 --recover
' || return 1

    verify_sqlite "${db}"
}
