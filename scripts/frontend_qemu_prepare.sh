#!/bin/bash
# scripts/frontend_qemu_prepare.sh
# Prepare the manual QEMU guest for frontend and Playwright sessions.
#
# This helper:
#   1. Waits for the QEMU VM SSH/API forwards to become reachable
#   2. Sets a password on the guest test user so PAM login works in the web UI
#   3. Restarts the guest service and reports diagnostics when the API is not ready
#
# Usage:
#   ./scripts/frontend_qemu_prepare.sh
#   QEMU_API_PORT=18443 QEMU_SSH_PORT=2222 ./scripts/frontend_qemu_prepare.sh
#   QEMU_WEB_PASSWORD=secret123 ./scripts/frontend_qemu_prepare.sh

set -euo pipefail

QEMU_API_HOST="${QEMU_API_HOST:-localhost}"
QEMU_API_PORT="${QEMU_API_PORT:-18443}"
QEMU_SSH_HOST="${QEMU_SSH_HOST:-localhost}"
QEMU_SSH_PORT="${QEMU_SSH_PORT:-2222}"
QEMU_SSH_USER="${QEMU_SSH_USER:-testuser}"
QEMU_WEB_USER="${QEMU_WEB_USER:-testuser}"
QEMU_WEB_PASSWORD="${QEMU_WEB_PASSWORD:-bitprotector}"
PROXY_TARGET="${BITPROTECTOR_DEV_PROXY_TARGET:-https://${QEMU_API_HOST}:${QEMU_API_PORT}}"

SSH_OPTS=(
    -T
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o ConnectTimeout=5
    -p "${QEMU_SSH_PORT}"
)

require_commands() {
    local missing=()

    if ! command -v ssh >/dev/null 2>&1; then
        missing+=("ssh")
    fi

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

report_guest_service_status() {
    ssh "${SSH_OPTS[@]}" "${QEMU_SSH_USER}@${QEMU_SSH_HOST}" \
        "bash -c 'sudo systemctl status bitprotector --no-pager -l || true; echo; sudo journalctl -u bitprotector -n 60 --no-pager || true'" \
        || true
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
    echo "         The frontend session will still continue, but requests may fail until the VM service is ready." >&2
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
NoNewPrivileges=false
User=root
Group=root
ReadWritePaths=
ReadWritePaths=/var/lib/bitprotector /var/log/bitprotector /var/lib/bitprotector/virtual /mnt/primary /mnt/mirror /mnt/replacement-primary /mnt/replacement-secondary /mnt/spare1 /mnt/spare2
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

main() {
    require_commands
    wait_for_ssh
    set_guest_password
    wait_for_api
}

main "$@"
