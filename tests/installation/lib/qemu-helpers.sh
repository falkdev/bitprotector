#!/bin/bash
# tests/installation/lib/qemu-helpers.sh
# Shared helpers sourced by qemu_test.sh, qemu_failover_test.sh, and qemu_uninstall_test.sh.
# Do not execute directly.

# ---------------------------------------------------------------------------
# Logging with optional GitHub Actions annotations
# ---------------------------------------------------------------------------

log() {
    local level="$1"
    shift
    local msg="$*"
    local ts
    ts="$(date -u '+%H:%M:%S')"

    if [[ "${CI:-}" == "1" ]]; then
        case "${level}" in
            ERROR)  echo "::error::${msg}" ;;
            WARN)   echo "::warning::${msg}" ;;
            GROUP)  echo "::group::${msg}" ;;
            ENDGROUP) echo "::endgroup::" ;;
            *)      echo "[${ts}] ${msg}" ;;
        esac
    else
        case "${level}" in
            ERROR)  echo "ERROR: ${msg}" >&2 ;;
            WARN)   echo "WARNING: ${msg}" >&2 ;;
            GROUP)  echo "=== ${msg} ===" ;;
            ENDGROUP) ;;
            *)      echo "[${ts}] ${msg}" ;;
        esac
    fi
}

# ---------------------------------------------------------------------------
# Command availability check
# ---------------------------------------------------------------------------

require_commands() {
    local missing=()
    for cmd in "$@"; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            missing+=("${cmd}")
        fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        log ERROR "missing required commands: ${missing[*]}"
        echo "Install with: sudo apt install qemu-system-x86 qemu-utils cloud-image-utils socat openssh-client"
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# SSH key resolution
# ---------------------------------------------------------------------------

resolve_ssh_key() {
    if [[ -n "${BITPROTECTOR_QEMU_SSH_KEY:-}" ]]; then
        printf '%s\n' "${BITPROTECTOR_QEMU_SSH_KEY}"
        return 0
    fi

    local key
    for key in "${HOME}/.ssh/id_ed25519.pub" "${HOME}/.ssh/id_rsa.pub"; do
        if [[ -f "${key}" ]]; then
            cat "${key}"
            return 0
        fi
    done

    log ERROR "no SSH public key found. Generate one with: ssh-keygen -t ed25519"
    echo "       or set BITPROTECTOR_QEMU_SSH_KEY to the public key text." >&2
    exit 1
}

# ---------------------------------------------------------------------------
# Guest image resolution
# GUEST_IMAGE accepts:
#   - an absolute path (used as-is)
#   - "ubuntu-24.04" -> ~/images/noble-server-cloudimg-amd64.img
#   - "ubuntu-26.04" -> ~/images/oracular-server-cloudimg-amd64.img
#     (codename "oracular" is provisional; update once 26.04 LTS name is final)
# UBUNTU_IMAGE still works as a deprecated alias when GUEST_IMAGE is unset.
# ---------------------------------------------------------------------------

resolve_guest_image() {
    local guest="${GUEST_IMAGE:-${UBUNTU_IMAGE:-ubuntu-24.04}}"

    case "${guest}" in
        ubuntu-24.04)
            echo "${HOME}/images/noble-server-cloudimg-amd64.img"
            ;;
        ubuntu-26.04)
            echo "${HOME}/images/plucky-server-cloudimg-amd64.img"
            ;;
        /*)
            echo "${guest}"
            ;;
        *)
            log ERROR "Unrecognised GUEST_IMAGE value: '${guest}'. Use an absolute path, 'ubuntu-24.04', or 'ubuntu-26.04'."
            exit 1
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Wait for QEMU VM to become SSH-accessible after cloud-init installs the .deb
# Usage: wait_for_vm <QEMU_PID> <SSH_PORT> <TIMEOUT> <WORKDIR>
# ---------------------------------------------------------------------------

wait_for_vm() {
    local qemu_pid="$1"
    local ssh_port="$2"
    local timeout="$3"
    local workdir="$4"
    local last_line=""

    log INFO "Waiting for VM to boot (up to ${timeout}s)..."

    for i in $(seq 1 "${timeout}"); do
        if ! kill -0 "${qemu_pid}" 2>/dev/null; then
            log ERROR "QEMU exited before VM became ready"
            echo "QEMU log:"
            tail -40 "${workdir}/qemu.log" 2>/dev/null || true
            exit 1
        fi

        if [[ -f "${workdir}/serial.log" ]]; then
            local new_line
            new_line=$(tail -1 "${workdir}/serial.log" | sed 's/^\[[ 0-9.]*\] //')
            if [[ "${new_line}" != "${last_line}" && -n "${new_line}" ]]; then
                printf "  [%3ds] %s\n" "${i}" "${new_line}"
                last_line="${new_line}"
            fi
        fi

        if ssh -o StrictHostKeyChecking=no -o ConnectTimeout=2 \
               -p "${ssh_port}" testuser@localhost \
               "test -f /tmp/install-done" 2>/dev/null; then
            log INFO "VM ready after ${i}s"
            return 0
        fi

        sleep 1

        if [[ $i -eq ${timeout} ]]; then
            log ERROR "VM did not become ready within ${timeout}s"
            echo "Last serial output:"
            tail -20 "${workdir}/serial.log" 2>/dev/null || true
            exit 1
        fi
    done
}

# ---------------------------------------------------------------------------
# Wait for the BitProtector API to respond on the forwarded port
# Usage: wait_for_api <API_PORT> <TIMEOUT>
# ---------------------------------------------------------------------------

wait_for_api() {
    local api_port="$1"
    local timeout="${2:-60}"

    log INFO "Waiting for API on port ${api_port} (up to ${timeout}s)..."
    for i in $(seq 1 "${timeout}"); do
        if nc -z localhost "${api_port}" 2>/dev/null; then
            log INFO "API reachable after ${i}s"
            return 0
        fi
        sleep 1
    done
    log WARN "API on port ${api_port} not reachable after ${timeout}s (may need TLS certs)"
    return 1
}
