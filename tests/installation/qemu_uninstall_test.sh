#!/bin/bash
# tests/installation/qemu_uninstall_test.sh
# QEMU-based full uninstall (apt purge) test for BitProtector.
#
# Prerequisites:
#   - qemu-system-x86_64 installed
#   - Ubuntu cloud image (see GUEST_IMAGE / UBUNTU_IMAGE env vars)
#   - cloud-image-utils (for cloud-init)
#   - an SSH public key in ~/.ssh, or BITPROTECTOR_QEMU_SSH_KEY set
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./tests/installation/qemu_uninstall_test.sh [/path/to/bitprotector.deb]
#
# Guest selection: same env vars as qemu_test.sh (GUEST_IMAGE / UBUNTU_IMAGE)
#
# Exit codes:
#   0  All tests passed
#   1  Build/install/bootstrap failed
#   2  Failed to create package-owned database/backup data
#   3  Package purge failed
#   4  Purge did not fully remove package-owned paths

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${SCRIPT_DIR}/lib/qemu-helpers.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2226}"
API_PORT="${API_PORT:-18447}"
TIMEOUT="${TIMEOUT:-600}"

require_commands qemu-system-x86_64 qemu-img cloud-localds ssh ssh-keygen
SSH_KEY="$(resolve_ssh_key)"

UBUNTU_IMAGE="$(resolve_guest_image)"

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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-uninstall-$$"
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
instance-id: bitprotector-uninstall-test
local-hostname: bitprotector-uninstall-test
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
$SSH "which bitprotector && bitprotector --version" || { log ERROR "binary not found"; exit 1; }
echo "PASS"
log ENDGROUP

log GROUP "Test 2: Create package-owned DB and backup data"
$SSH '
set -euo pipefail
DB_PATH=/var/lib/bitprotector/bitprotector.db
BACKUP_DIR=/var/lib/bitprotector/backups/uninstall-test
NO_CFG=/tmp/bitprotector-missing-config.toml

sudo systemctl stop bitprotector || true
sudo install -d -m 0750 -o bitprotector -g bitprotector "${BACKUP_DIR}"

# Ensure the package-owned DB path is initialized.
sudo bitprotector --config "${NO_CFG}" --db "${DB_PATH}" status >/dev/null

add_output=$(sudo bitprotector --config "${NO_CFG}" --db "${DB_PATH}" database add "${BACKUP_DIR}" 2>&1)
printf "%s\n" "${add_output}" | grep -q "Backup destination #"

run_output=$(sudo bitprotector --config "${NO_CFG}" --db "${DB_PATH}" database run "${DB_PATH}" 2>&1)
printf "%s\n" "${run_output}" | grep -Fq "[OK] Destination #"
printf "%s\n" "${run_output}" | grep -Eq "[0-9]+/[0-9]+ backups succeeded\."

sudo test -f "${DB_PATH}"
backup_count=$(sudo find "${BACKUP_DIR}" -maxdepth 1 -type f -name "bitprotector-*.db" | wc -l)
test "${backup_count}" -ge 1
' || { log ERROR "could not prepare package-owned database/backup data"; exit 2; }
echo "PASS"
log ENDGROUP

log GROUP "Test 3: Purge package"
$SSH "sudo DEBIAN_FRONTEND=noninteractive apt-get purge -y bitprotector" || \
    { log ERROR "package purge failed"; exit 3; }
echo "PASS"
log ENDGROUP

log GROUP "Test 4: Verify complete uninstall"
$SSH '
set -euo pipefail

if dpkg -s bitprotector >/dev/null 2>&1; then
    echo "bitprotector package is still installed." >&2
    exit 1
fi

if [ -e /usr/bin/bitprotector ]; then
    echo "/usr/bin/bitprotector is still present." >&2
    exit 1
fi

if [ -e /var/lib/bitprotector ]; then
    echo "/var/lib/bitprotector is still present." >&2
    exit 1
fi

if [ -e /var/log/bitprotector ]; then
    echo "/var/log/bitprotector is still present." >&2
    exit 1
fi

if [ -e /etc/bitprotector ]; then
    echo "/etc/bitprotector is still present." >&2
    exit 1
fi
' || { log ERROR "purge did not remove package-owned paths"; exit 4; }
echo "PASS"
log ENDGROUP

echo ""
echo "=== Full uninstall test passed ==="
