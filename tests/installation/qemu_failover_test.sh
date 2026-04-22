#!/bin/bash
# tests/installation/qemu_failover_test.sh
# End-to-end QEMU failover and replacement test for BitProtector.
#
# This suite provisions extra virtio disks, mounts them inside the guest as
# /mnt/primary, /mnt/mirror, /mnt/replacement-primary, and /mnt/replacement-secondary,
# then exercises:
#   - planned primary failover via drives replace mark/confirm
#   - writes through the virtual-path symlink tree while secondary is active
#   - rebuild onto a replacement primary
#   - emergency failover by hot-removing the active replacement-primary disk via QMP
#
# Prerequisites:
#   - qemu-system-x86_64, qemu-img, cloud-localds, socat, ssh
#   - Ubuntu cloud image (see GUEST_IMAGE / UBUNTU_IMAGE env vars)
#   - an SSH public key in ~/.ssh, or BITPROTECTOR_QEMU_SSH_KEY set
#   - bitprotector.deb built via cargo deb
#
# Usage:
#   ./tests/installation/qemu_failover_test.sh [/path/to/bitprotector.deb]
#
# Guest selection: same env vars as qemu_test.sh (GUEST_IMAGE / UBUNTU_IMAGE)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${SCRIPT_DIR}/lib/qemu-helpers.sh"

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

ssh_vm() {
    ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 -p "${SSH_PORT}" testuser@localhost "$@"
}

qmp_device_del() {
    local device_id="$1"
    if [[ ! -S "${QMP_SOCKET}" ]]; then
        log ERROR "QMP socket not available at ${QMP_SOCKET}"
        exit 1
    fi

    printf '{ "execute": "qmp_capabilities" }\n{ "execute": "device_del", "arguments": { "id": "%s" } }\n' "${device_id}" \
        | socat - UNIX-CONNECT:"${QMP_SOCKET}" >/dev/null
}

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
      setup_disk bpreplprimary /mnt/replacement-primary
      setup_disk bpreplsecondary /mnt/replacement-secondary

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
  - apt-get install -y -q /mnt/debpkg/bitprotector*.deb
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-failover-test
local-hostname: bitprotector-failover-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM with extra failover disks..."
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

log GROUP "Scenario 1: Planned primary failover and replacement rebuild"
ssh_vm '
set -euo pipefail
DB=/tmp/failover.db
VIRTUAL_FILE=/tmp/bitprotector-virtual/docs/report.txt

mkdir -p /mnt/primary/docs
printf "before failover\n" > /mnt/primary/docs/report.txt

bitprotector --db "${DB}" drives add lab /mnt/primary /mnt/mirror
bitprotector --db "${DB}" files track 1 docs/report.txt --mirror
bitprotector --db "${DB}" files mirror 1
bitprotector --db "${DB}" folders add 1 docs
bitprotector --db "${DB}" virtual-paths set 1 "${VIRTUAL_FILE}"

readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/primary/"
cat "${VIRTUAL_FILE}" | grep -q "before failover"

bitprotector --db "${DB}" drives replace mark 1 --role primary
bitprotector --db "${DB}" drives replace confirm 1 --role primary
bitprotector --db "${DB}" drives show 1 | grep -q "Active Role:     secondary"
readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/mirror/"

printf "after planned failover\n" >> "${VIRTUAL_FILE}"
bitprotector --db "${DB}" folders scan 1
bitprotector --db "${DB}" files show 1 | grep -q "Mirrored:      no"

bitprotector --db "${DB}" drives replace assign 1 --role primary /mnt/replacement-primary --no-validate
bitprotector --db "${DB}" sync process

test -f /mnt/replacement-primary/docs/report.txt
diff -u "${VIRTUAL_FILE}" /mnt/replacement-primary/docs/report.txt
readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/replacement-primary/"
'

echo "PASS: planned failover and rebuild completed"
log ENDGROUP

log GROUP "Scenario 2: Emergency failover after hot-removing active primary"
log INFO "Hot-removing replacement primary disk through QMP..."
qmp_device_del "dev-replacement-primary"
sleep 5

ssh_vm '
set -euo pipefail
DB=/tmp/failover.db
VIRTUAL_FILE=/tmp/bitprotector-virtual/docs/report.txt

# Existing open file handles may fail after sudden device loss.
# We assert the supported contract: a follow-up operation triggers failover,
# then new opens through the virtual path work from the surviving mirror.
bitprotector --db "${DB}" integrity check 1
bitprotector --db "${DB}" drives show 1 | grep -q "Active Role:     secondary"
readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/mirror/"
cat "${VIRTUAL_FILE}" | grep -q "after planned failover"
'

echo "PASS: emergency failover redirected future opens to the mirror"
log ENDGROUP

echo ""
echo "=== All failover tests passed ==="
