#!/bin/bash
# scripts/qemu_populate_primary.sh
# Populate /mnt/primary/test-files inside the local QEMU guest with generated files.
#
# Usage:
#   ./scripts/qemu_populate_primary.sh <file-count> <file-size>
#
# Examples:
#   ./scripts/qemu_populate_primary.sh 100 1M
#   QEMU_SSH_PORT=2222 ./scripts/qemu_populate_primary.sh 500 64K
#
# Environment overrides:
#   QEMU_SSH_HOST (default: localhost)
#   QEMU_SSH_PORT (default: 2222)
#   QEMU_SSH_USER (default: testuser)

set -euo pipefail

usage() {
    grep '^# ' "$0" | sed 's/^# //'
    exit 1
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
fi

if [[ "$#" -ne 2 ]]; then
    echo "ERROR: expected exactly 2 parameters."
    usage
fi

FILE_COUNT="$1"
FILE_SIZE="$2"

if ! [[ "${FILE_COUNT}" =~ ^[1-9][0-9]*$ ]]; then
    echo "ERROR: <file-count> must be a positive integer."
    exit 1
fi

if [[ -z "${FILE_SIZE}" ]]; then
    echo "ERROR: <file-size> cannot be empty."
    exit 1
fi

QEMU_SSH_HOST="${QEMU_SSH_HOST:-localhost}"
QEMU_SSH_PORT="${QEMU_SSH_PORT:-2222}"
QEMU_SSH_USER="${QEMU_SSH_USER:-testuser}"
TARGET_DIR="/mnt/primary/test-files"

if ! command -v ssh >/dev/null 2>&1; then
    echo "ERROR: ssh is required but was not found on PATH."
    exit 1
fi

SSH_OPTS=(
    -T
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o ConnectTimeout=5
    -p "${QEMU_SSH_PORT}"
)

echo "Connecting to ${QEMU_SSH_USER}@${QEMU_SSH_HOST}:${QEMU_SSH_PORT} ..."

ssh "${SSH_OPTS[@]}" "${QEMU_SSH_USER}@${QEMU_SSH_HOST}" \
    "bash -s -- '${FILE_COUNT}' '${FILE_SIZE}' '${TARGET_DIR}'" <<'EOF'
set -euo pipefail

file_count="$1"
file_size="$2"
target_dir="$3"

rm -rf "${target_dir}"
mkdir -p "${target_dir}"

for i in $(seq 1 "${file_count}"); do
    file_path=$(printf '%s/file-%06d.bin' "${target_dir}" "${i}")
    if command -v fallocate >/dev/null 2>&1; then
        fallocate -l "${file_size}" "${file_path}"
    elif command -v truncate >/dev/null 2>&1; then
        truncate -s "${file_size}" "${file_path}"
    else
        dd if=/dev/zero of="${file_path}" bs="${file_size}" count=1 status=none
    fi
done

echo "Created ${file_count} file(s) of size ${file_size} in ${target_dir}."
ls -1 "${target_dir}" | wc -l | awk '{print "File count:", $1}'
du -sh "${target_dir}" || true
EOF
