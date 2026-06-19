#!/bin/bash
# scripts/qemu_seed_pm_and_track.sh
# Ensure a "pm" drive pair exists in the local QEMU guest API, populate data under
# /mnt/primary/<folder-name>, then add that folder to tracking with virtual paths
# under /mnt/spare1/<folder-name>.
#
# Usage:
#   ./scripts/qemu_seed_pm_and_track.sh <folder-name> <file-count> <file-size>
#   ./scripts/qemu_seed_pm_and_track.sh --batch-file <path>
#
# Examples:
#   ./scripts/qemu_seed_pm_and_track.sh test-files 100 1M
#   QEMU_SSH_PORT=2222 QEMU_API_PORT=18443 ./scripts/qemu_seed_pm_and_track.sh loadset-a 500 64K
#   ./scripts/qemu_seed_pm_and_track.sh --batch-file ./jobs.txt
#
# Batch file format (one row per folder, whitespace-delimited):
#   <folder-name> <file-count> <file-size>
# Blank lines and lines starting with # are ignored.
#
# Environment overrides:
#   QEMU_SSH_HOST (default: localhost)
#   QEMU_SSH_PORT (default: 2222)
#   QEMU_SSH_USER (default: testuser)
#   QEMU_API_HOST (default: localhost)
#   QEMU_API_PORT (default: 18443)
#   QEMU_API_USER (default: testuser)
#   QEMU_API_PASSWORD (default: bitprotector)
#   QEMU_VIRTUAL_BASE (default: /mnt/spare1)

set -euo pipefail

usage() {
    grep '^# ' "$0" | sed 's/^# //'
    exit 1
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
fi

BATCH_FILE=""
FOLDER_NAME=""
FILE_COUNT=""
FILE_SIZE=""

if [[ "$#" -eq 2 && "${1}" == "--batch-file" ]]; then
    BATCH_FILE="$2"
elif [[ "$#" -eq 3 ]]; then
    FOLDER_NAME="$1"
    FILE_COUNT="$2"
    FILE_SIZE="$3"
else
    echo "ERROR: expected either 3 parameters or --batch-file <path>."
    usage
fi

QEMU_SSH_HOST="${QEMU_SSH_HOST:-localhost}"
QEMU_SSH_PORT="${QEMU_SSH_PORT:-2222}"
QEMU_SSH_USER="${QEMU_SSH_USER:-testuser}"
QEMU_API_HOST="${QEMU_API_HOST:-localhost}"
QEMU_API_PORT="${QEMU_API_PORT:-18443}"
QEMU_API_USER="${QEMU_API_USER:-testuser}"
QEMU_API_PASSWORD="${QEMU_API_PASSWORD:-bitprotector}"
QEMU_VIRTUAL_BASE="${QEMU_VIRTUAL_BASE:-/mnt/spare1}"

API_BASE="https://${QEMU_API_HOST}:${QEMU_API_PORT}/api/v1"

require_commands() {
    local missing=()
    local cmd
    for cmd in ssh curl jq; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            missing+=("${cmd}")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "ERROR: missing required commands: ${missing[*]}" >&2
        exit 1
    fi
}

SSH_OPTS=(
    -T
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o ConnectTimeout=5
    -p "${QEMU_SSH_PORT}"
)

api_request() {
    local method="$1"
    local path="$2"
    local token="$3"
    local body="${4:-}"
    local raw status response
    local url="${API_BASE}${path}"

    if [[ -n "${body}" ]]; then
        raw="$(
            curl -skS --connect-timeout 5 --max-time 30 \
                -X "${method}" \
                "${url}" \
                -H "Authorization: Bearer ${token}" \
                -H "Content-Type: application/json" \
                --data "${body}" \
                -w $'\nHTTP_STATUS:%{http_code}\n'
        )"
    else
        raw="$(
            curl -skS --connect-timeout 5 --max-time 30 \
                -X "${method}" \
                "${url}" \
                -H "Authorization: Bearer ${token}" \
                -w $'\nHTTP_STATUS:%{http_code}\n'
        )"
    fi

    status="$(printf '%s\n' "${raw}" | sed -n 's/^HTTP_STATUS://p' | tail -1)"
    response="$(printf '%s\n' "${raw}" | sed '/^HTTP_STATUS:/d')"

    if [[ "${status}" =~ ^[0-9]{3}$ ]] && (( status >= 200 && status < 300 )); then
        printf '%s\n' "${response}"
        return 0
    fi

    echo "ERROR: API ${method} ${path} failed (status=${status})." >&2
    echo "${response}" >&2
    exit 1
}

api_login() {
    local raw token
    raw="$(
        curl -skS --connect-timeout 5 --max-time 30 \
            -X POST \
            "${API_BASE}/auth/login" \
            -H "Content-Type: application/json" \
            --data "{\"username\":\"${QEMU_API_USER}\",\"password\":\"${QEMU_API_PASSWORD}\"}"
    )"
    token="$(printf '%s' "${raw}" | jq -r '.token // empty')"
    if [[ -z "${token}" ]]; then
        echo "ERROR: failed API login at ${API_BASE}/auth/login" >&2
        echo "${raw}" >&2
        exit 1
    fi
    printf '%s\n' "${token}"
}

ensure_pm_drive() {
    local token="$1"
    local drives_json drive_id by_name_id by_path_id
    drives_json="$(api_request GET "/drives" "${token}")"

    by_name_id="$(
        printf '%s' "${drives_json}" \
            | jq -r '.[] | select(.name=="pm") | .id' \
            | head -n1
    )"
    by_path_id="$(
        printf '%s' "${drives_json}" \
            | jq -r '.[] | select(.primary_path=="/mnt/primary" and .secondary_path=="/mnt/mirror") | .id' \
            | head -n1
    )"

    if [[ -n "${by_name_id}" ]]; then
        drive_id="${by_name_id}"
        api_request PUT "/drives/${drive_id}" "${token}" \
            '{"name":"pm","primary_path":"/mnt/primary","secondary_path":"/mnt/mirror"}' >/dev/null
    elif [[ -n "${by_path_id}" ]]; then
        drive_id="${by_path_id}"
        api_request PUT "/drives/${drive_id}" "${token}" \
            '{"name":"pm","primary_path":"/mnt/primary","secondary_path":"/mnt/mirror"}' >/dev/null
    else
        drive_id="$(
            api_request POST "/drives" "${token}" \
                '{"name":"pm","primary_path":"/mnt/primary","secondary_path":"/mnt/mirror"}' \
                | jq -r '.id'
        )"
    fi

    if [[ -z "${drive_id}" || "${drive_id}" == "null" ]]; then
        echo "ERROR: unable to resolve drive id for pm." >&2
        exit 1
    fi

    printf '%s\n' "${drive_id}"
}

validate_job() {
    local folder_name="$1"
    local file_count="$2"
    local file_size="$3"

    if [[ -z "${folder_name}" ]]; then
        echo "ERROR: <folder-name> cannot be empty." >&2
        exit 1
    fi

    if [[ "${folder_name}" == *"/"* || "${folder_name}" == "." || "${folder_name}" == ".." ]]; then
        echo "ERROR: <folder-name> must be a single directory name (no slashes)." >&2
        exit 1
    fi

    if ! [[ "${file_count}" =~ ^[1-9][0-9]*$ ]]; then
        echo "ERROR: <file-count> must be a positive integer." >&2
        exit 1
    fi

    if [[ -z "${file_size}" ]]; then
        echo "ERROR: <file-size> cannot be empty." >&2
        exit 1
    fi
}

populate_primary_folder() {
    local folder_name="$1"
    local file_count="$2"
    local file_size="$3"
    local target_dir="/mnt/primary/${folder_name}"

    echo "Connecting to ${QEMU_SSH_USER}@${QEMU_SSH_HOST}:${QEMU_SSH_PORT} ..."

    ssh "${SSH_OPTS[@]}" "${QEMU_SSH_USER}@${QEMU_SSH_HOST}" \
        "bash -s -- '${file_count}' '${file_size}' '${target_dir}'" <<'EOF'
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
}

ensure_folder_tracking() {
    local token="$1"
    local drive_id="$2"
    local folder_name="$3"
    local folders_json folder_id current_virtual_path target_virtual_path

    target_virtual_path="${QEMU_VIRTUAL_BASE%/}/${folder_name}"
    folders_json="$(api_request GET "/folders" "${token}")"
    folder_id="$(
        printf '%s' "${folders_json}" | jq -r \
            --argjson drive_id "${drive_id}" \
            --arg folder "${folder_name}" \
            '.[] | select(.drive_pair_id == $drive_id and (.folder_path | rtrimstr("/")) == $folder) | .id' \
            | head -n1
    )"
    current_virtual_path="$(
        printf '%s' "${folders_json}" | jq -r \
            --argjson drive_id "${drive_id}" \
            --arg folder "${folder_name}" \
            '.[] | select(.drive_pair_id == $drive_id and (.folder_path | rtrimstr("/")) == $folder) | .virtual_path // empty' \
            | head -n1
    )"

    if [[ -z "${folder_id}" ]]; then
        folder_id="$(
            api_request POST "/folders" "${token}" \
                "{\"drive_pair_id\":${drive_id},\"folder_path\":\"${folder_name}\",\"virtual_path\":\"${target_virtual_path}\"}" \
                | jq -r '.id'
        )"
    elif [[ "${current_virtual_path}" != "${target_virtual_path}" ]]; then
        api_request PUT "/folders/${folder_id}" "${token}" \
            "{\"virtual_path\":\"${target_virtual_path}\"}" >/dev/null
    fi

    if [[ -z "${folder_id}" || "${folder_id}" == "null" ]]; then
        echo "ERROR: unable to resolve tracked folder id for ${folder_name}." >&2
        exit 1
    fi

    echo "Tracked folder ${folder_name} (id ${folder_id}, virtual_path ${target_virtual_path})."
}

process_job() {
    local token="$1"
    local drive_id="$2"
    local folder_name="$3"
    local file_count="$4"
    local file_size="$5"

    validate_job "${folder_name}" "${file_count}" "${file_size}"
    populate_primary_folder "${folder_name}" "${file_count}" "${file_size}"
    ensure_folder_tracking "${token}" "${drive_id}" "${folder_name}"
}

require_commands
TOKEN="$(api_login)"
DRIVE_ID="$(ensure_pm_drive "${TOKEN}")"
echo "Using drive pair id ${DRIVE_ID} (name: pm, primary: /mnt/primary, secondary: /mnt/mirror)."

if [[ -n "${BATCH_FILE}" ]]; then
    if [[ ! -f "${BATCH_FILE}" ]]; then
        echo "ERROR: batch file not found: ${BATCH_FILE}" >&2
        exit 1
    fi

    line_no=0
    while IFS= read -r line || [[ -n "${line}" ]]; do
        line_no=$((line_no + 1))
        [[ -z "${line//[[:space:]]/}" ]] && continue
        [[ "${line}" =~ ^[[:space:]]*# ]] && continue

        folder_name=""
        file_count=""
        file_size=""
        extra=""
        read -r folder_name file_count file_size extra <<<"${line}"
        if [[ -n "${extra}" || -z "${folder_name}" || -z "${file_count}" || -z "${file_size}" ]]; then
            echo "ERROR: invalid batch row ${line_no}: '${line}'" >&2
            echo "Expected format: <folder-name> <file-count> <file-size>" >&2
            exit 1
        fi

        process_job "${TOKEN}" "${DRIVE_ID}" "${folder_name}" "${file_count}" "${file_size}"
    done <"${BATCH_FILE}"
else
    process_job "${TOKEN}" "${DRIVE_ID}" "${FOLDER_NAME}" "${FILE_COUNT}" "${FILE_SIZE}"
fi
