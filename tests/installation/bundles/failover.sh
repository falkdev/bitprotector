#!/bin/bash
# tests/installation/bundles/failover.sh
# Failover bundle: end-to-end planned and emergency failover tests for BitProtector.
#
# Cloud-init provisions: 4 extra virtio disks
# (primary ext4, mirror ext4, replacement-primary xfs, replacement-secondary ext4).
#
# Scenarios:
#   E   failover-01-planned
#   E   failover-02-emergency-qmp
#   #17 failover-03-bit-flip-auto-repair
#   #-- failover-04-both-corrupted
#   #-- failover-05-large-file-streaming
#   #-- failover-06-integrity-triggered-auto-recovery
#   #-- failover-07-virtual-path-folder-retarget
#   #18 failover-08-unicode-whitespace-long-paths
#   #19 failover-09-two-pairs-one-disk
#   #20 failover-10-cross-fs-matrix
#   #27 failover-11-device-add-hot-insert
#   #-- failover-12-qmp-hot-remove-secondary
#
# Prerequisites:
#   - qemu-system-x86_64, qemu-img, cloud-localds, socat, ssh, ssh-keygen, readlink
#   - Ubuntu cloud image (GUEST_IMAGE / UBUNTU_IMAGE env vars)
#   - SSH public key in ~/.ssh or BITPROTECTOR_QEMU_SSH_KEY set
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./tests/installation/bundles/failover.sh [/path/to/bitprotector.deb]
#
# Exit codes: 0 all passed, non-zero on failure

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/failover"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"
# shellcheck source=tests/installation/lib/snapshots.sh
source "${LIB_DIR}/snapshots.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2223}"
API_PORT="${API_PORT:-18444}"
TIMEOUT="${TIMEOUT:-900}"

require_commands qemu-system-x86_64 qemu-img cloud-localds socat ssh ssh-keygen readlink
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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-failover-$$"
mkdir -p "${WORKDIR}"
QMP_SOCKET="${WORKDIR}/qmp.sock"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true

qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/vm.qcow2"
qemu-img create -f qcow2 "${WORKDIR}/primary.qcow2" 4G
qemu-img create -f qcow2 "${WORKDIR}/mirror.qcow2" 4G
qemu-img create -f qcow2 "${WORKDIR}/replacement-primary.qcow2" 4G
qemu-img create -f qcow2 "${WORKDIR}/replacement-secondary.qcow2" 4G

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
  - path: /usr/local/bin/bitprotector-qemu-storage.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -euo pipefail

      setup_disk() {
          local serial="\$1"
          local mount_point="\$2"
          local fs_type="\${3:-ext4}"
          local dev="/dev/disk/by-id/virtio-\${serial}"
          local uuid

          for _ in \$(seq 1 30); do
              if [[ -b "\${dev}" ]]; then
                  break
              fi
              sleep 1
          done

          if [[ ! -b "\${dev}" ]]; then
              echo "Disk \${serial} not found at \${dev}" >&2
              exit 1
          fi

          mkdir -p "\${mount_point}"

          if ! blkid "\${dev}" >/dev/null 2>&1; then
              if [[ "\${fs_type}" == "xfs" ]]; then
                  mkfs.xfs -f "\${dev}"
              else
                  mkfs.ext4 -F "\${dev}"
              fi
          fi

          uuid=\$(blkid -s UUID -o value "\${dev}")
          if ! grep -q "\${uuid}" /etc/fstab; then
              echo "UUID=\${uuid} \${mount_point} \${fs_type} defaults,nofail 0 2" >> /etc/fstab
          fi
      }

      setup_disk bpprimary /mnt/primary ext4
      setup_disk bpmirror /mnt/mirror ext4
      setup_disk bpreplprimary /mnt/replacement-primary xfs
      setup_disk bpreplsecondary /mnt/replacement-secondary ext4

      mount -a
      mkdir -p /tmp/bitprotector-virtual
      chown -R testuser:testuser \
          /mnt/primary \
          /mnt/mirror \
          /mnt/replacement-primary \
          /mnt/replacement-secondary \
          /tmp/bitprotector-virtual

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-qemu-storage.sh
  - apt-get update -q
  - apt-get install -y -q xfsprogs /mnt/debpkg/bitprotector*.deb
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-failover-test
local-hostname: bitprotector-failover-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM with extra failover disks (failover bundle)..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -qmp "unix:${QMP_SOCKET},server=on,wait=off" \
    -drive "file=${WORKDIR}/vm.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-primary,file=${WORKDIR}/primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-primary,id=dev-primary,serial=bpprimary" \
    -drive "if=none,id=drive-mirror,file=${WORKDIR}/mirror.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-mirror,id=dev-mirror,serial=bpmirror" \
    -drive "if=none,id=drive-replacement-primary,file=${WORKDIR}/replacement-primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-replacement-primary,id=dev-replacement-primary,serial=bpreplprimary" \
    -drive "if=none,id=drive-replacement-secondary,file=${WORKDIR}/replacement-secondary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-replacement-secondary,id=dev-replacement-secondary,serial=bpreplsecondary" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"

# --- Scenarios ---

BUNDLE_START_TIME="$(date -Iseconds)"

# shellcheck source=tests/installation/scenarios/failover/failover-01-planned.sh
source "${SCENARIOS_DIR}/failover-01-planned.sh"
run_scenario "failover-01-planned" failover_01_planned

# Pass QMP_SOCKET through to scenario-02 (emergency failover needs it)
export QMP_SOCKET

# shellcheck source=tests/installation/scenarios/failover/failover-02-emergency-qmp.sh
source "${SCENARIOS_DIR}/failover-02-emergency-qmp.sh"
run_scenario "failover-02-emergency-qmp" failover_02_emergency_qmp

# shellcheck source=tests/installation/scenarios/failover/failover-03-bit-flip-auto-repair.sh
source "${SCENARIOS_DIR}/failover-03-bit-flip-auto-repair.sh"
run_scenario "failover-03-bit-flip-auto-repair" failover_03_bit_flip_auto_repair

# shellcheck source=tests/installation/scenarios/failover/failover-04-both-corrupted.sh
source "${SCENARIOS_DIR}/failover-04-both-corrupted.sh"
run_scenario "failover-04-both-corrupted" failover_04_both_corrupted

# shellcheck source=tests/installation/scenarios/failover/failover-05-large-file-streaming.sh
source "${SCENARIOS_DIR}/failover-05-large-file-streaming.sh"
run_scenario "failover-05-large-file-streaming" failover_05_large_file_streaming

# shellcheck source=tests/installation/scenarios/failover/failover-06-integrity-triggered-auto-recovery.sh
source "${SCENARIOS_DIR}/failover-06-integrity-triggered-auto-recovery.sh"
run_scenario "failover-06-integrity-triggered-auto-recovery" failover_06_integrity_triggered_auto_recovery

# shellcheck source=tests/installation/scenarios/failover/failover-07-virtual-path-folder-retarget.sh
source "${SCENARIOS_DIR}/failover-07-virtual-path-folder-retarget.sh"
run_scenario "failover-07-virtual-path-folder-retarget" failover_07_virtual_path_folder_retarget

# shellcheck source=tests/installation/scenarios/failover/failover-08-unicode-whitespace-long-paths.sh
source "${SCENARIOS_DIR}/failover-08-unicode-whitespace-long-paths.sh"
run_scenario "failover-08-unicode-whitespace-long-paths" failover_08_unicode_whitespace_long_paths

# shellcheck source=tests/installation/scenarios/failover/failover-09-two-pairs-one-disk.sh
source "${SCENARIOS_DIR}/failover-09-two-pairs-one-disk.sh"
run_scenario "failover-09-two-pairs-one-disk" failover_09_two_pairs_one_disk

# shellcheck source=tests/installation/scenarios/failover/failover-10-cross-fs-matrix.sh
source "${SCENARIOS_DIR}/failover-10-cross-fs-matrix.sh"
run_scenario "failover-10-cross-fs-matrix" failover_10_cross_fs_matrix

# shellcheck source=tests/installation/scenarios/failover/failover-11-device-add-hot-insert.sh
source "${SCENARIOS_DIR}/failover-11-device-add-hot-insert.sh"
run_scenario "failover-11-device-add-hot-insert" failover_11_device_add_hot_insert

# shellcheck source=tests/installation/scenarios/failover/failover-12-qmp-hot-remove-secondary.sh
source "${SCENARIOS_DIR}/failover-12-qmp-hot-remove-secondary.sh"
run_scenario "failover-12-qmp-hot-remove-secondary" failover_12_qmp_hot_remove_secondary

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All failover tests passed ==="
