#!/bin/bash
# tests/installation/bundles/upgrade.sh
# Upgrade bundle: alpha1 -> current package upgrade and config preservation checks.

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/upgrade"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"
# shellcheck source=tests/installation/lib/cloud-init-db-disk.sh
source "${LIB_DIR}/cloud-init-db-disk.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
ALPHA1_GLOB="${ALPHA1_DEB:-${PROJECT_ROOT}/target/debian/bitprotector_1.0.0~alpha1*.deb}"
SSH_PORT="${SSH_PORT:-2225}"
API_PORT="${API_PORT:-18446}"
TIMEOUT="${TIMEOUT:-900}"

require_commands qemu-system-x86_64 qemu-img cloud-localds ssh ssh-keygen
SSH_KEY="$(resolve_ssh_key)"
UBUNTU_IMAGE="$(resolve_guest_image)"

CURRENT_DEB=$(ls -1 ${DEB_PATH} 2>/dev/null | grep -v 'alpha1' | head -1 || true)
ALPHA1_DEB_FILE=$(ls -1 ${ALPHA1_GLOB} 2>/dev/null | head -1 || true)
if [[ -z "${CURRENT_DEB}" ]]; then
    log ERROR "current .deb file not found at ${DEB_PATH}"
    echo "Build with: cargo deb"
    exit 1
fi
if [[ -z "${ALPHA1_DEB_FILE}" ]]; then
    log ERROR "alpha1 .deb file not found at ${ALPHA1_GLOB}"
    echo "Provide ALPHA1_DEB=/path/to/bitprotector_1.0.0~alpha1*.deb"
    exit 1
fi

if [[ ! -f "${UBUNTU_IMAGE}" ]]; then
    log ERROR "cloud image not found at ${UBUNTU_IMAGE}"
    echo "Run: ./scripts/setup-qemu.sh"
    exit 1
fi

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-upgrade-$$"
mkdir -p "${WORKDIR}" "${WORKDIR}/debpkg"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

cp -f "${CURRENT_DEB}" "${WORKDIR}/debpkg/"
cp -f "${ALPHA1_DEB_FILE}" "${WORKDIR}/debpkg/"
CURRENT_DEB_NAME="$(basename "${CURRENT_DEB}")"
ALPHA1_DEB_NAME="$(basename "${ALPHA1_DEB_FILE}")"

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
  - path: /etc/bitprotector-upgrade.env
    permissions: '0644'
    content: |
      CURRENT_DEB_NAME=${CURRENT_DEB_NAME}
      ALPHA1_DEB_NAME=${ALPHA1_DEB_NAME}
$(cloudinit_bpdb_write_file)

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-db-storage.sh
  - apt-get update -q
  - apt-get install -y -q jq openssl curl /mnt/debpkg/${ALPHA1_DEB_NAME}
  - mkdir -p /etc/bitprotector/tls
  - openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' -keyout /etc/bitprotector/tls/key.pem -out /etc/bitprotector/tls/cert.pem
  - chown -R bitprotector:bitprotector /etc/bitprotector/tls
  - chmod 600 /etc/bitprotector/tls/key.pem
  - chmod 644 /etc/bitprotector/tls/cert.pem
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-upgrade-test
local-hostname: bitprotector-upgrade-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (upgrade bundle)..."
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
    -virtfs "local,path=${WORKDIR}/debpkg,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
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

# shellcheck source=tests/installation/scenarios/upgrade/upgrade-01-alpha1-to-current-with-live-data.sh
source "${SCENARIOS_DIR}/upgrade-01-alpha1-to-current-with-live-data.sh"
run_scenario "upgrade-01-alpha1-to-current-with-live-data" upgrade_01_alpha1_to_current_with_live_data

# shellcheck source=tests/installation/scenarios/upgrade/upgrade-02-reinstall-preserves-config.sh
source "${SCENARIOS_DIR}/upgrade-02-reinstall-preserves-config.sh"
run_scenario "upgrade-02-reinstall-preserves-config" upgrade_02_reinstall_preserves_config

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All upgrade tests passed ==="
