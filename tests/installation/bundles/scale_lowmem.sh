#!/bin/bash
# tests/installation/bundles/scale_lowmem.sh
# Scale low-memory bundle: dataset stress checks under 1 GB guest memory.

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/scale-lowmem"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2229}"
API_PORT="${API_PORT:-18450}"
TIMEOUT="${TIMEOUT:-1200}"

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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-scale-lowmem-$$"
mkdir -p "${WORKDIR}"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true
qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/vm.qcow2"
qemu-img create -f qcow2 "${WORKDIR}/primary.qcow2" 8G
qemu-img create -f qcow2 "${WORKDIR}/mirror.qcow2" 8G
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
  - path: /usr/local/bin/bitprotector-lowmem-storage.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -euo pipefail
      setup_disk() {
        local serial="\$1"
        local mount_point="\$2"
        local dev="/dev/disk/by-id/virtio-\${serial}"
        for _ in \$(seq 1 30); do
          [[ -b "\${dev}" ]] && break
          sleep 1
        done
        [[ -b "\${dev}" ]]
        mkdir -p "\${mount_point}"
        if ! blkid "\${dev}" >/dev/null 2>&1; then
          mkfs.ext4 -F "\${dev}"
        fi
        uuid=\$(blkid -s UUID -o value "\${dev}")
        grep -q "\${uuid}" /etc/fstab || echo "UUID=\${uuid} \${mount_point} ext4 defaults,nofail 0 2" >> /etc/fstab
      }
      setup_disk bplowprimary /mnt/primary
      setup_disk bplowmirror /mnt/mirror
      setup_disk bpdb /mnt/bitprotector-db
      mount -a
      mkdir -p /mnt/bitprotector-db/db
      chown -R testuser:testuser /mnt/primary /mnt/mirror /mnt/bitprotector-db

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get update -q
  - apt-get install -y -q jq /mnt/debpkg/bitprotector*.deb
  - /usr/local/bin/bitprotector-lowmem-storage.sh
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-scale-lowmem-test
local-hostname: bitprotector-scale-lowmem-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (scale-lowmem bundle)..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 2 \
    -m 1024 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/vm.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-primary,file=${WORKDIR}/primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-primary,id=dev-primary,serial=bplowprimary" \
    -drive "if=none,id=drive-mirror,file=${WORKDIR}/mirror.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-mirror,id=dev-mirror,serial=bplowmirror" \
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

BUNDLE_START_TIME="$(date -Iseconds)"
SSH_VM_TIMEOUT=900

# shellcheck source=tests/installation/scenarios/scale-lowmem/scale-lowmem-01-4gb-dataset.sh
source "${SCENARIOS_DIR}/scale-lowmem-01-4gb-dataset.sh"
run_scenario "scale-lowmem-01-4gb-dataset" scale_lowmem_01_4gb_dataset

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All scale-lowmem tests passed ==="
