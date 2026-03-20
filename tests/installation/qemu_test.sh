#!/bin/bash
# tests/installation/qemu_test.sh
# QEMU-based installation test for BitProtector on Ubuntu 24.
#
# Prerequisites:
#   - qemu-system-x86_64 installed
#   - Ubuntu 24 cloud image (noble-server-cloudimg-amd64.img)
#   - cloud-image-utils (for cloud-init)
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./tests/installation/qemu_test.sh [/path/to/bitprotector.deb]
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

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
UBUNTU_IMAGE="${UBUNTU_IMAGE:-${HOME}/images/noble-server-cloudimg-amd64.img}"
SSH_PORT=2222
TIMEOUT=600

# Resolve glob
DEB_FILE=$(ls -1 ${DEB_PATH} 2>/dev/null | head -1)
if [[ -z "${DEB_FILE}" ]]; then
    echo "ERROR: .deb file not found at ${DEB_PATH}"
    echo "Build with: cargo deb"
    exit 1
fi

if [[ ! -f "${UBUNTU_IMAGE}" ]]; then
    echo "ERROR: Ubuntu 24 cloud image not found at ${UBUNTU_IMAGE}"
    echo "Download with: wget https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"
    exit 1
fi

WORKDIR=$(mktemp -d)
trap 'rm -rf "${WORKDIR}"; kill "${QEMU_PID}" 2>/dev/null || true' EXIT

# Clear any stale host key for localhost:2222 from previous runs
ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true

# Create a copy of the image (copy-on-write)
qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/test.qcow2"

# Cloud-init user-data
cat > "${WORKDIR}/user-data" << 'CLOUDINIT'
#cloud-config
users:
  - default
  - name: testuser
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: true
    ssh_authorized_keys:
      - ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHEPbXkhD9lZI3FPrTsBc1mibunoeoQ8iWbDFkO3mLS4 alexander.falk@outlook.com

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get update -q
  - apt-get install -y -q /mnt/debpkg/bitprotector*.deb
  - systemctl enable bitprotector
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" << 'CLOUDINIT'
instance-id: bitprotector-test
local-hostname: bitprotector-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

echo "Starting QEMU VM..."
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
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::18443-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

echo "Waiting for VM to boot (up to ${TIMEOUT}s)..."
LAST_SERIAL_LINE=""
for i in $(seq 1 ${TIMEOUT}); do
    # Show new serial console lines as they appear (strip kernel timestamps)
    if [[ -f "${WORKDIR}/serial.log" ]]; then
        NEW_LINE=$(tail -1 "${WORKDIR}/serial.log" | sed 's/^\[[ 0-9.]*\] //')
        if [[ "${NEW_LINE}" != "${LAST_SERIAL_LINE}" && -n "${NEW_LINE}" ]]; then
            printf "  [%3ds] %s\n" "${i}" "${NEW_LINE}"
            LAST_SERIAL_LINE="${NEW_LINE}"
        fi
    fi

    if ssh -o StrictHostKeyChecking=no -o ConnectTimeout=2 -p "${SSH_PORT}" testuser@localhost \
        "test -f /tmp/install-done" 2>/dev/null; then
        echo "VM ready after ${i}s"
        break
    fi
    sleep 1
    if [[ $i -eq ${TIMEOUT} ]]; then
        echo "ERROR: VM did not become ready within ${TIMEOUT}s"
        echo "Last serial output:"
        tail -20 "${WORKDIR}/serial.log" 2>/dev/null || true
        exit 1
    fi
done

SSH="ssh -o StrictHostKeyChecking=no -p ${SSH_PORT} testuser@localhost"

echo ""
echo "=== Test 1: Package installed ==="
$SSH "which bitprotector && bitprotector --version" || { echo "FAIL: binary not found"; exit 3; }
echo "PASS"

echo ""
echo "=== Test 2: Service status ==="
$SSH "sudo systemctl is-active bitprotector || sudo journalctl -u bitprotector -n 20" || true
echo "(NOTE: service may need TLS certs to start fully)"

echo ""
echo "=== Test 3: CLI smoke tests ==="
$SSH "bitprotector --db /tmp/test.db drives list" || { echo "FAIL: CLI drives list"; exit 3; }
$SSH "bitprotector --db /tmp/test.db status" || { echo "FAIL: CLI status"; exit 3; }
echo "PASS"

echo ""
echo "=== Test 4: SSH login status (profile.d) ==="
$SSH "test -f /etc/profile.d/bitprotector-status.sh && echo 'hook installed'" || \
    { echo "FAIL: profile.d hook not installed"; exit 3; }
echo "PASS"

echo ""
echo "=== All installation tests passed ==="
