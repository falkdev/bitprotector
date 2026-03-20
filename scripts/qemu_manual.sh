#!/bin/bash
# scripts/qemu_manual.sh
# Start a persistent QEMU VM with BitProtector installed for manual testing.
#
# The VM disk image is reused across runs so your state is preserved.
# To start fresh, delete the workdir: rm -rf ~/.cache/bitprotector-qemu
#
# Prerequisites:
#   - qemu-system-x86_64 installed
#   - Ubuntu 24 cloud image (noble-server-cloudimg-amd64.img)
#   - cloud-image-utils (for cloud-init)
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./scripts/qemu_manual.sh [/path/to/bitprotector.deb]
#
#   UBUNTU_IMAGE=/path/to/image.img ./scripts/qemu_manual.sh
#   FRESH=1 ./scripts/qemu_manual.sh   # destroy and recreate the VM disk
#
# Ports forwarded to host:
#   SSH  -> localhost:2222
#   API  -> localhost:18443
#
# Connect:
#   ssh -o StrictHostKeyChecking=no -p 2222 testuser@localhost
#   curl -k https://localhost:18443/api/status

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
UBUNTU_IMAGE="${UBUNTU_IMAGE:-${HOME}/images/noble-server-cloudimg-amd64.img}"
WORKDIR="${HOME}/.cache/bitprotector-qemu"
SSH_PORT=2222
API_PORT=18443

# ── Preflight checks ──────────────────────────────────────────────────────────

for cmd in qemu-system-x86_64 qemu-img cloud-localds; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "ERROR: '$cmd' not found."
        echo "Install with: sudo apt install qemu-system-x86_64 cloud-image-utils"
        exit 1
    fi
done

DEB_FILE=$(ls -1 ${DEB_PATH} 2>/dev/null | head -1 || true)
if [[ -z "${DEB_FILE}" ]]; then
    echo "ERROR: .deb file not found at ${DEB_PATH}"
    echo "Build with: cargo deb"
    exit 1
fi

if [[ ! -f "${UBUNTU_IMAGE}" ]]; then
    echo "ERROR: Ubuntu 24 cloud image not found at ${UBUNTU_IMAGE}"
    echo "Download with:"
    echo "  mkdir -p ~/images"
    echo "  wget -P ~/images https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"
    exit 1
fi

# ── Workdir / disk image ──────────────────────────────────────────────────────

if [[ "${FRESH:-0}" == "1" && -d "${WORKDIR}" ]]; then
    echo "FRESH=1: removing existing VM at ${WORKDIR}"
    rm -rf "${WORKDIR}"
fi

mkdir -p "${WORKDIR}"

FIRST_RUN=false
if [[ ! -f "${WORKDIR}/vm.qcow2" ]]; then
    FIRST_RUN=true
    echo "Creating new VM disk image (copy-on-write from base image)..."
    qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/vm.qcow2"
fi

# ── Cloud-init seed (only needed on first boot) ───────────────────────────────

if [[ "${FIRST_RUN}" == "true" ]]; then
    echo "Creating cloud-init seed ISO..."

    # Use the current user's authorized_keys if available, otherwise generate guidance
    SSH_KEY=""
    if [[ -f "${HOME}/.ssh/id_ed25519.pub" ]]; then
        SSH_KEY="$(cat "${HOME}/.ssh/id_ed25519.pub")"
    elif [[ -f "${HOME}/.ssh/id_rsa.pub" ]]; then
        SSH_KEY="$(cat "${HOME}/.ssh/id_rsa.pub")"
    else
        echo "WARNING: No SSH public key found in ~/.ssh/. You will need to log in via the console."
        echo "         Generate one with: ssh-keygen -t ed25519"
        SSH_KEY="# no key found - add your key here and re-run with FRESH=1"
    fi

    cat > "${WORKDIR}/user-data" << CLOUDINIT
#cloud-config
users:
  - default
  - name: testuser
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    passwd: "\$6\$rounds=4096\$saltsalt\$testpassword"
    ssh_authorized_keys:
      - ${SSH_KEY}
    groups: bitprotector

package_update: true

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get install -y /mnt/debpkg/$(basename "${DEB_FILE}") && touch /tmp/install-done
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
CLOUDINIT

    cat > "${WORKDIR}/meta-data" << CLOUDINIT
instance-id: bitprotector-manual
local-hostname: bitprotector-dev
CLOUDINIT

    cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"
fi

# ── Launch QEMU ───────────────────────────────────────────────────────────────

echo ""
echo "========================================="
echo "  BitProtector Manual QEMU Session"
echo "========================================="
echo ""
echo "  SSH:  ssh -o StrictHostKeyChecking=no -p ${SSH_PORT} testuser@localhost"
echo "  API:  https://localhost:${API_PORT}"
echo ""
echo "  Shared package dir (host -> /mnt/debpkg in VM):"
echo "    $(dirname "${DEB_FILE}")"
echo ""
echo "  VM state persists at: ${WORKDIR}/vm.qcow2"
echo "  To start fresh:       FRESH=1 $0"
echo ""
echo "  Press Ctrl+A X to exit QEMU console"
echo "========================================="
echo ""

exec qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -nographic \
    -drive "file=${WORKDIR}/vm.qcow2,format=qcow2" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=$(dirname "${DEB_FILE}"),mount_tag=debpkg,security_model=passthrough,id=debpkg"
