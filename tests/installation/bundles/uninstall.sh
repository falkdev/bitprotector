#!/bin/bash
# tests/installation/bundles/uninstall.sh
# Uninstall bundle: full apt-purge and post-purge verification for BitProtector.
#
# Cloud-init provisions: standard install + service start.
#
# Scenarios (Phase 1 — existing coverage only):
#   E  uninstall-01-package-installed  — binary in PATH, --version works
#   E  uninstall-02-create-data        — creates package-owned DB and backup data
#   E  uninstall-03-purge              — apt-get purge + verify all package paths removed
#
# Phase 8 adds:
#   #26 uninstall-04-purge-preserves-user-drive-data — user drive files survive purge
#
# Prerequisites:
#   - qemu-system-x86_64, qemu-img, cloud-localds, ssh, ssh-keygen
#   - Ubuntu cloud image (GUEST_IMAGE / UBUNTU_IMAGE env vars)
#   - SSH public key in ~/.ssh or BITPROTECTOR_QEMU_SSH_KEY set
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./tests/installation/bundles/uninstall.sh [/path/to/bitprotector.deb]
#
# Exit codes: 0 all passed, non-zero on failure

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/uninstall"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"

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
qemu-img create -f qcow2 "${WORKDIR}/bpdb.qcow2" 32G

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

write_files:
  - path: /usr/local/bin/bitprotector-db-storage.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -euo pipefail
      dev=/dev/disk/by-id/virtio-bpdb
      for _ in \$(seq 1 30); do
        [[ -b "\${dev}" ]] && break
        sleep 1
      done
      [[ -b "\${dev}" ]]
      mkdir -p /mnt/bitprotector-db
      if ! blkid "\${dev}" >/dev/null 2>&1; then
        mkfs.ext4 -F "\${dev}"
      fi
      uuid=\$(blkid -s UUID -o value "\${dev}")
      grep -q "\${uuid}" /etc/fstab || echo "UUID=\${uuid} /mnt/bitprotector-db ext4 defaults,nofail 0 2" >> /etc/fstab
      mount -a
      mkdir -p /mnt/bitprotector-db/db
      chown -R testuser:testuser /mnt/bitprotector-db

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-db-storage.sh
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

log INFO "Starting QEMU VM (uninstall bundle)..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/test.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-bpdb,file=${WORKDIR}/bpdb.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-bpdb,id=dev-bpdb,serial=bpdb" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"
ssh_vm '
set -euo pipefail
if ! findmnt /mnt/bitprotector-db >/dev/null 2>&1; then
  echo "Expected /mnt/bitprotector-db to be mounted" >&2
  exit 1
fi
touch /mnt/bitprotector-db/db/.write-test
rm -f /mnt/bitprotector-db/db/.write-test
'

# --- Scenarios ---

BUNDLE_START_TIME="$(date -Iseconds)"

# shellcheck source=tests/installation/scenarios/uninstall/uninstall-01-package-installed.sh
source "${SCENARIOS_DIR}/uninstall-01-package-installed.sh"
run_scenario "uninstall-01-package-installed" uninstall_01_package_installed

# shellcheck source=tests/installation/scenarios/uninstall/uninstall-02-create-data.sh
source "${SCENARIOS_DIR}/uninstall-02-create-data.sh"
run_scenario "uninstall-02-create-data" uninstall_02_create_data

# shellcheck source=tests/installation/scenarios/uninstall/uninstall-03-purge.sh
source "${SCENARIOS_DIR}/uninstall-03-purge.sh"
run_scenario "uninstall-03-purge" uninstall_03_purge

# shellcheck source=tests/installation/scenarios/uninstall/uninstall-04-purge-preserves-user-drive-data.sh
source "${SCENARIOS_DIR}/uninstall-04-purge-preserves-user-drive-data.sh"
run_scenario "uninstall-04-purge-preserves-user-drive-data" uninstall_04_purge_preserves_user_drive_data

# Journal scraper runs after purge: SSH still works (OS is up), journald retains
# entries from the bitprotector unit even after the package is removed.
run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== Full uninstall test passed ==="
