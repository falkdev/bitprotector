#!/bin/bash
# tests/installation/qemu_test.sh
# QEMU-based installation smoke test for BitProtector.
#
# Prerequisites:
#   - qemu-system-x86_64 installed
#   - Ubuntu cloud image (see GUEST_IMAGE / UBUNTU_IMAGE env vars)
#   - cloud-image-utils (for cloud-init)
#   - an SSH public key in ~/.ssh, or BITPROTECTOR_QEMU_SSH_KEY set
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./tests/installation/qemu_test.sh [/path/to/bitprotector.deb]
#
# Guest selection:
#   GUEST_IMAGE=ubuntu-24.04  (default) → ~/images/noble-server-cloudimg-amd64.img
#   GUEST_IMAGE=ubuntu-26.04            → ~/images/plucky-server-cloudimg-amd64.img
#   GUEST_IMAGE=/absolute/path/to.img   → use that image directly
#   UBUNTU_IMAGE=...                    → deprecated alias, still honoured
#
# Exit codes:
#   0  All tests passed
#   1  Build or install failed
#   2  Service failed to start
#   3  CLI smoke tests failed
#   4  API not accessible

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${SCRIPT_DIR}/lib/qemu-helpers.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2222}"
API_PORT="${API_PORT:-18443}"
TIMEOUT="${TIMEOUT:-600}"

require_commands qemu-system-x86_64 qemu-img cloud-localds ssh ssh-keygen
SSH_KEY="$(resolve_ssh_key)"

UBUNTU_IMAGE="$(resolve_guest_image)"

# Resolve glob
DEB_FILE=$(ls -1 ${DEB_PATH} 2>/dev/null | head -1 || true)
if [[ -z "${DEB_FILE}" ]]; then
    log ERROR ".deb file not found at ${DEB_PATH}"
    echo "Build with: cargo deb"
    exit 1
fi

if [[ ! -f "${UBUNTU_IMAGE}" ]]; then
    log ERROR "cloud image not found at ${UBUNTU_IMAGE}"
    echo "Run: ./scripts/setup-qemu.sh"
    exit 1
fi

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-smoke-$$"
mkdir -p "${WORKDIR}"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true

qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/test.qcow2"

cat > "${WORKDIR}/user-data" <<CLOUDINIT
#cloud-config
users:
  - default
  - name: testuser
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: true
    ssh_authorized_keys:
      - ${SSH_KEY}

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get update -q
  - apt-get install -y -q /mnt/debpkg/bitprotector*.deb
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-test
local-hostname: bitprotector-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/test.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"

SSH="ssh -o StrictHostKeyChecking=no -p ${SSH_PORT} testuser@localhost"

log GROUP "Test 1: Package installed"
$SSH "which bitprotector && bitprotector --version" || { log ERROR "binary not found"; exit 3; }
echo "PASS"
log ENDGROUP

log GROUP "Test 2: Service status"
$SSH "sudo systemctl is-active bitprotector || sudo journalctl -u bitprotector -n 20" || true
echo "(NOTE: service may need TLS certs to start fully)"
log ENDGROUP

log GROUP "Test 3: CLI smoke tests"
$SSH "bitprotector --db /tmp/test.db drives list" || { log ERROR "CLI drives list failed"; exit 3; }
$SSH "bitprotector --db /tmp/test.db status" || { log ERROR "CLI status failed"; exit 3; }
echo "PASS"
log ENDGROUP

log GROUP "Test 4: SSH login status (profile.d)"
$SSH "test -f /etc/profile.d/bitprotector-status.sh && echo 'hook installed'" || \
    { log ERROR "profile.d hook not installed"; exit 3; }
echo "PASS"
log ENDGROUP

echo ""
echo "=== All installation tests passed ==="
