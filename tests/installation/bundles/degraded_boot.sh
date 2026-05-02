#!/bin/bash
# tests/installation/bundles/degraded_boot.sh
# Degraded-boot bundle: startup and status behavior with missing/invalid primary mount paths.

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/degraded-boot"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"
# shellcheck source=tests/installation/lib/cloud-init-db-disk.sh
source "${LIB_DIR}/cloud-init-db-disk.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2227}"
API_PORT="${API_PORT:-18448}"
TIMEOUT="${TIMEOUT:-900}"

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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-degraded-boot-$$"
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
$(cloudinit_bpdb_write_file)

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-db-storage.sh
  - apt-get update -q
  - apt-get install -y -q jq openssl curl /mnt/debpkg/bitprotector*.deb
  - mkdir -p /etc/bitprotector/tls
  - openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' -keyout /etc/bitprotector/tls/key.pem -out /etc/bitprotector/tls/cert.pem
  - chown -R bitprotector:bitprotector /etc/bitprotector/tls
  - chmod 600 /etc/bitprotector/tls/key.pem
  - chmod 644 /etc/bitprotector/tls/cert.pem
  - mkdir -p /mnt/absent-primary
  - echo 'UUID=00000000-0000-0000-0000-000000000000 /mnt/absent-primary ext4 defaults,nofail 0 2' >> /etc/fstab
  - mount -a || true
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-degraded-boot-test
local-hostname: bitprotector-degraded-boot-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (degraded-boot bundle)..."
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

# shellcheck source=tests/installation/scenarios/degraded-boot/degraded-boot-01-fake-mount-point.sh
source "${SCENARIOS_DIR}/degraded-boot-01-fake-mount-point.sh"
run_scenario "degraded-boot-01-fake-mount-point" degraded_boot_01_fake_mount_point

# shellcheck source=tests/installation/scenarios/degraded-boot/degraded-boot-02-device-absent-at-boot.sh
source "${SCENARIOS_DIR}/degraded-boot-02-device-absent-at-boot.sh"
run_scenario "degraded-boot-02-device-absent-at-boot" degraded_boot_02_device_absent_at_boot

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All degraded-boot tests passed ==="
