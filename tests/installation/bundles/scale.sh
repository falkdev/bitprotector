#!/bin/bash
# tests/installation/bundles/scale.sh
# Scale bundle: large dataset and watch-capacity scenarios (nightly only).

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/scale"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2228}"
API_PORT="${API_PORT:-18449}"
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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-scale-$$"
mkdir -p "${WORKDIR}"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true
qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/vm.qcow2"
qemu-img create -f qcow2 "${WORKDIR}/scale.qcow2" 100G

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
  - path: /usr/local/bin/bitprotector-scale-storage.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -euo pipefail
      dev=/dev/disk/by-id/virtio-bpscale
      for _ in $(seq 1 30); do
        [[ -b "${dev}" ]] && break
        sleep 1
      done
      [[ -b "${dev}" ]]
      mkdir -p /mnt/scale /mnt/scale-mirror
      if ! blkid "${dev}" >/dev/null 2>&1; then
        mkfs.ext4 -F "${dev}"
      fi
      uuid=$(blkid -s UUID -o value "${dev}")
      grep -q "${uuid}" /etc/fstab || echo "UUID=${uuid} /mnt/scale ext4 defaults,nofail 0 2" >> /etc/fstab
      mount -a
      mkdir -p /mnt/scale-mirror
      chown -R testuser:testuser /mnt/scale /mnt/scale-mirror

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get update -q
  - apt-get install -y -q jq /mnt/debpkg/bitprotector*.deb
  - /usr/local/bin/bitprotector-scale-storage.sh
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-scale-test
local-hostname: bitprotector-scale-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (scale bundle)..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 8192 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/vm.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-scale,file=${WORKDIR}/scale.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-scale,id=dev-scale,serial=bpscale" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"

BUNDLE_START_TIME="$(date -Iseconds)"

# shellcheck source=tests/installation/scenarios/scale/scale-01-100k-real-files.sh
source "${SCENARIOS_DIR}/scale-01-100k-real-files.sh"
run_scenario "scale-01-100k-real-files" scale_01_100k_real_files

# shellcheck source=tests/installation/scenarios/scale/scale-02-inotify-saturation.sh
source "${SCENARIOS_DIR}/scale-02-inotify-saturation.sh"
run_scenario "scale-02-inotify-saturation" scale_02_inotify_saturation

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All scale tests passed ==="
