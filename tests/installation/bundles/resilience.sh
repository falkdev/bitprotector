#!/bin/bash
# tests/installation/bundles/resilience.sh
# Resilience bundle: filesystem and process failure-mode scenarios.

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/resilience"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"
# shellcheck source=tests/installation/lib/snapshots.sh
source "${LIB_DIR}/snapshots.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2224}"
API_PORT="${API_PORT:-18445}"
TIMEOUT="${TIMEOUT:-900}"

require_commands qemu-system-x86_64 qemu-img cloud-localds socat ssh ssh-keygen
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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-resilience-$$"
mkdir -p "${WORKDIR}"
QMP_SOCKET="${WORKDIR}/qmp.sock"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true

qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/vm.qcow2"
qemu-img create -f qcow2 "${WORKDIR}/primary.qcow2" 3G
qemu-img create -f qcow2 "${WORKDIR}/mirror.qcow2" 3G

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
              mkfs.ext4 -F "\${dev}"
          fi

          uuid=\$(blkid -s UUID -o value "\${dev}")
          if ! grep -q "\${uuid}" /etc/fstab; then
              echo "UUID=\${uuid} \${mount_point} ext4 defaults,nofail 0 2" >> /etc/fstab
          fi
      }

      setup_disk bpprimary /mnt/primary
      setup_disk bpmirror /mnt/mirror
      mount -a
      mkdir -p /tmp/bitprotector-virtual
      chown -R testuser:testuser /mnt/primary /mnt/mirror /tmp/bitprotector-virtual

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-qemu-storage.sh
  - apt-get update -q
  - apt-get install -y -q jq openssl curl /mnt/debpkg/bitprotector*.deb
  - mkdir -p /etc/bitprotector/tls
  - openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' -keyout /etc/bitprotector/tls/key.pem -out /etc/bitprotector/tls/cert.pem
  - chown -R bitprotector:bitprotector /etc/bitprotector/tls
  - chmod 600 /etc/bitprotector/tls/key.pem
  - chmod 644 /etc/bitprotector/tls/cert.pem
  - id -u testauth >/dev/null 2>&1 || useradd -m testauth
  - echo 'testauth:hunter2' | chpasswd
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-resilience-test
local-hostname: bitprotector-resilience-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (resilience bundle)..."
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
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"

restore_baseline() {
    qmp_loadvm baseline
    sleep 2
    wait_for_vm "${QEMU_PID}" "${SSH_PORT}" 240 "${WORKDIR}"
}

qmp_savevm baseline

# --- Scenarios ---

BUNDLE_START_TIME="$(date -Iseconds)"

# shellcheck source=tests/installation/scenarios/resilience/resilience-01-enospc.sh
source "${SCENARIOS_DIR}/resilience-01-enospc.sh"
run_scenario "resilience-01-enospc" resilience_01_enospc

# shellcheck source=tests/installation/scenarios/resilience/resilience-02-readonly-mirror.sh
source "${SCENARIOS_DIR}/resilience-02-readonly-mirror.sh"
run_scenario "resilience-02-readonly-mirror" resilience_02_readonly_mirror

# shellcheck source=tests/installation/scenarios/resilience/resilience-03-eacces-tracked-file.sh
source "${SCENARIOS_DIR}/resilience-03-eacces-tracked-file.sh"
run_scenario "resilience-03-eacces-tracked-file" resilience_03_eacces_tracked_file

# shellcheck source=tests/installation/scenarios/resilience/resilience-04-symlink-loop.sh
source "${SCENARIOS_DIR}/resilience-04-symlink-loop.sh"
run_scenario "resilience-04-symlink-loop" resilience_04_symlink_loop

restore_baseline
# shellcheck source=tests/installation/scenarios/resilience/resilience-05-sigterm-mid-sync.sh
source "${SCENARIOS_DIR}/resilience-05-sigterm-mid-sync.sh"
run_scenario "resilience-05-sigterm-mid-sync" resilience_05_sigterm_mid_sync

restore_baseline
# shellcheck source=tests/installation/scenarios/resilience/resilience-06-sigkill-recovery.sh
source "${SCENARIOS_DIR}/resilience-06-sigkill-recovery.sh"
run_scenario "resilience-06-sigkill-recovery" resilience_06_sigkill_recovery

restore_baseline
# shellcheck source=tests/installation/scenarios/resilience/resilience-07-auto-restart-after-panic.sh
source "${SCENARIOS_DIR}/resilience-07-auto-restart-after-panic.sh"
run_scenario "resilience-07-auto-restart-after-panic" resilience_07_auto_restart_after_panic

# Keep a dedicated scrape window after all expected-failure scenarios.
BUNDLE_START_TIME="$(date -Iseconds)"
run_scenario "resilience-08-journal-error-scraper" journal_error_scraper

echo ""
echo "=== All resilience tests passed ==="
