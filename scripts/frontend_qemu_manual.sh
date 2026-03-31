#!/bin/bash
# scripts/frontend_qemu_manual.sh
# Prepare the local frontend dev server for use against the manual QEMU backend.
#
# This helper:
#   1. Verifies a usable Node.js runtime is available for frontend manual testing
#   2. Prepares the QEMU guest so PAM login works in the web UI
#   3. Starts the Vite dev server with its proxy pointed at the QEMU API port
#
# Usage:
#   ./scripts/frontend_qemu_manual.sh
#   QEMU_API_PORT=18443 QEMU_SSH_PORT=2222 ./scripts/frontend_qemu_manual.sh
#   QEMU_WEB_PASSWORD=secret123 ./scripts/frontend_qemu_manual.sh
#   SKIP_NPM_CI=1 ./scripts/frontend_qemu_manual.sh
#
# Default credentials configured by this script:
#   username: testuser
#   password: bitprotector

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
FRONTEND_DIR="${PROJECT_ROOT}/frontend"
PREP_SCRIPT="${SCRIPT_DIR}/frontend_qemu_prepare.sh"

QEMU_API_HOST="${QEMU_API_HOST:-localhost}"
QEMU_API_PORT="${QEMU_API_PORT:-18443}"
QEMU_SSH_HOST="${QEMU_SSH_HOST:-localhost}"
QEMU_SSH_PORT="${QEMU_SSH_PORT:-2222}"
QEMU_SSH_USER="${QEMU_SSH_USER:-testuser}"
QEMU_WEB_USER="${QEMU_WEB_USER:-testuser}"
QEMU_WEB_PASSWORD="${QEMU_WEB_PASSWORD:-bitprotector}"

FRONTEND_HOST="${FRONTEND_HOST:-127.0.0.1}"
FRONTEND_PORT="${FRONTEND_PORT:-5173}"
PROXY_TARGET="${BITPROTECTOR_DEV_PROXY_TARGET:-https://${QEMU_API_HOST}:${QEMU_API_PORT}}"

SSH_OPTS=(
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o ConnectTimeout=5
    -p "${QEMU_SSH_PORT}"
)

require_commands() {
    local missing=()
    local cmd
    for cmd in node npm; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            missing+=("${cmd}")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "ERROR: missing required commands: ${missing[*]}" >&2
        exit 1
    fi
}

check_node_runtime() {
    local node_version
    local node_major
    local node_minor
    node_version="$(node --version 2>/dev/null)"
    node_version="${node_version#v}"

    if [[ ! "${node_version}" =~ ^([0-9]+)\.([0-9]+)(\.[0-9]+)?$ ]]; then
        echo "ERROR: could not parse Node.js version from $(node --version)" >&2
        exit 1
    fi

    node_major="${BASH_REMATCH[1]}"
    node_minor="${BASH_REMATCH[2]}"

    if [[ "${node_major}" -lt 20 || ( "${node_major}" -eq 20 && "${node_minor}" -lt 19 ) ]]; then
        echo "ERROR: Node.js 20.19+ is required for the frontend helper (found $(node --version))" >&2
        exit 1
    fi
}

install_frontend_dependencies() {
    if [[ "${SKIP_NPM_CI:-0}" == "1" ]]; then
        return 0
    fi

    (cd "${FRONTEND_DIR}" && npm ci)
}

print_summary() {
    cat <<EOF

=========================================
  BitProtector Frontend Manual Setup
=========================================

Frontend URL:
  http://${FRONTEND_HOST}:${FRONTEND_PORT}

Proxied backend target:
  ${PROXY_TARGET}

Web login credentials:
  username: ${QEMU_WEB_USER}
  password: ${QEMU_WEB_PASSWORD}

SSH to the guest:
  ssh -o StrictHostKeyChecking=no -p ${QEMU_SSH_PORT} ${QEMU_SSH_USER}@${QEMU_SSH_HOST}

Notes:
  - This script configures a password on the guest user so the PAM-backed web login works.
  - API requests stay same-origin through the Vite proxy, so the guest's self-signed TLS cert is handled server-side.
  - If you do not want npm ci on every run, use SKIP_NPM_CI=1.

=========================================

EOF
}

start_frontend() {
    cd "${FRONTEND_DIR}"
    BITPROTECTOR_DEV_PROXY_TARGET="${PROXY_TARGET}" \
        npm run dev -- --host "${FRONTEND_HOST}" --port "${FRONTEND_PORT}"
}

require_commands
check_node_runtime
bash "${PREP_SCRIPT}"
install_frontend_dependencies
print_summary
start_frontend
