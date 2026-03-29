#!/bin/bash
# scripts/frontend_qemu_manual.sh
# Prepare the local frontend dev server for use against the manual QEMU backend.
#
# This helper:
#   1. Verifies a usable Node.js runtime is available for frontend manual testing
#   2. Waits for the QEMU VM SSH/API forwards to become reachable
#   3. Sets a password on the guest test user so PAM login works in the web UI
#   4. Starts the Vite dev server with its proxy pointed at the QEMU API port
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
    for cmd in node npm ssh; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            missing+=("${cmd}")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "ERROR: missing required commands: ${missing[*]}" >&2
        exit 1
    fi
}

fetch_status_code() {
    local url="$1"

    if command -v curl >/dev/null 2>&1; then
        curl -sk -o /dev/null -w '%{http_code}' --max-time 5 "${url}" 2>/dev/null || true
        return 0
    fi

    if command -v wget >/dev/null 2>&1; then
        wget \
            --server-response \
            --spider \
            --timeout=5 \
            --tries=1 \
            --no-check-certificate \
            "${url}" 2>&1 | awk '/^  HTTP\// { code=$2 } END { print code }'
        return 0
    fi

    return 1
}

check_node_runtime() {
    local node_major
    local node_minor
    node_major="$(node -p 'Number(process.versions.node.split(".")[0])')"
    node_minor="$(node -p 'Number(process.versions.node.split(".")[1])')"

    if [[ "${node_major}" -lt 20 || ( "${node_major}" -eq 20 && "${node_minor}" -lt 19 ) ]]; then
        echo "ERROR: Node.js 20.19+ is required for the frontend helper (found $(node --version))" >&2
        exit 1
    fi
}

wait_for_ssh() {
    local attempt
    for attempt in $(seq 1 30); do
        if ssh "${SSH_OPTS[@]}" "${QEMU_SSH_USER}@${QEMU_SSH_HOST}" true >/dev/null 2>&1; then
            return 0
        fi
        sleep 2
    done

    echo "ERROR: could not reach the QEMU VM over SSH at ${QEMU_SSH_HOST}:${QEMU_SSH_PORT}" >&2
    echo "Start the VM first with ./scripts/qemu_manual.sh" >&2
    exit 1
}

wait_for_api() {
    local attempt
    local status

    if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
        echo "WARNING: neither curl nor wget is available, skipping API readiness check." >&2
        return 0
    fi

    for attempt in $(seq 1 30); do
        status="$(fetch_status_code "${PROXY_TARGET}/api/v1/status")"
        if [[ "${status}" == "200" || "${status}" == "401" ]]; then
            return 0
        fi
        sleep 2
    done

    echo "WARNING: the API at ${PROXY_TARGET} did not answer before timeout." >&2
    echo "         The frontend dev server will still start, but requests may fail until the VM service is ready." >&2
    echo "         Guest service diagnostics:" >&2
    report_guest_service_status >&2
}

set_guest_password() {
    local quoted_user
    local quoted_password
    quoted_user="$(printf '%q' "${QEMU_WEB_USER}")"
    quoted_password="$(printf '%q' "${QEMU_WEB_PASSWORD}")"

    ssh "${SSH_OPTS[@]}" "${QEMU_SSH_USER}@${QEMU_SSH_HOST}" \
        "QEMU_WEB_USER=${quoted_user} QEMU_WEB_PASSWORD=${quoted_password} bash -s" <<'EOF' >/dev/null
            set -euo pipefail
            printf "%s:%s\n" "$QEMU_WEB_USER" "$QEMU_WEB_PASSWORD" | sudo chpasswd
            sudo usermod --unlock "$QEMU_WEB_USER"
            if getent group bitprotector >/dev/null 2>&1; then
                sudo usermod -a -G bitprotector "$QEMU_WEB_USER" || true
            fi
            sudo install -d -m 0770 -o bitprotector -g bitprotector /var/lib/bitprotector
            sudo install -d -m 0755 -o bitprotector -g bitprotector /var/lib/bitprotector/frontend
            sudo chown -R bitprotector:bitprotector /var/lib/bitprotector/frontend
            sudo find /var/lib/bitprotector -maxdepth 1 -name "bitprotector.db*" \
                -exec chown bitprotector:bitprotector {} +
            sudo install -d -m 0755 /etc/systemd/system/bitprotector.service.d
            cat <<'OVERRIDE' | sudo tee /etc/systemd/system/bitprotector.service.d/manual-qemu.conf >/dev/null
[Service]
ExecStart=
ExecStart=/usr/bin/bitprotector \
    --db /var/lib/bitprotector/bitprotector.db \
    serve \
    --host 0.0.0.0 \
    --port 8443 \
    --tls-cert /etc/bitprotector/tls/cert.pem \
    --tls-key /etc/bitprotector/tls/key.pem
OVERRIDE
            sudo systemctl daemon-reload
            sudo systemctl restart bitprotector
EOF
}

report_guest_service_status() {
    ssh "${SSH_OPTS[@]}" "${QEMU_SSH_USER}@${QEMU_SSH_HOST}" \
        "bash -c 'sudo systemctl status bitprotector --no-pager -l || true; echo; sudo journalctl -u bitprotector -n 40 --no-pager || true'" \
        || true
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
wait_for_ssh
set_guest_password
wait_for_api
install_frontend_dependencies
print_summary
start_frontend
