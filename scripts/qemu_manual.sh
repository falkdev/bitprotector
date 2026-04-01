#!/bin/bash
# scripts/qemu_manual.sh
# Start a persistent QEMU VM with BitProtector installed for manual testing.
#
# The VM disk image is reused across runs so your state is preserved.
# To start fresh, delete the workdir: rm -rf ~/.cache/bitprotector-qemu
#
# This manual VM provisions an extended extra-disk layout:
#   /mnt/primary
#   /mnt/mirror
#   /mnt/replacement-primary
#   /mnt/replacement-secondary
#   /mnt/spare1
#   /mnt/spare2
#
# Optional QMP support:
#   ENABLE_QMP=1 ./scripts/qemu_manual.sh
#   socat - UNIX-CONNECT:~/.cache/bitprotector-qemu/qmp.sock
#
# Usage:
#   ./scripts/qemu_manual.sh [/path/to/bitprotector.deb]
#   UBUNTU_IMAGE=/path/to/image.img ./scripts/qemu_manual.sh
#   FRESH=1 ./scripts/qemu_manual.sh
#
# Ports forwarded to host:
#   SSH  -> localhost:2222
#   API  -> localhost:18443

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
UBUNTU_IMAGE="${UBUNTU_IMAGE:-${HOME}/images/noble-server-cloudimg-amd64.img}"
WORKDIR="${HOME}/.cache/bitprotector-qemu"
SSH_PORT=2222
API_PORT=18443
ENABLE_QMP="${ENABLE_QMP:-0}"
QMP_SOCKET="${WORKDIR}/qmp.sock"

require_commands() {
    local missing=()
    for cmd in qemu-system-x86_64 qemu-img cloud-localds ssh; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            missing+=("${cmd}")
        fi
    done
    if [[ "${ENABLE_QMP}" == "1" ]] && ! command -v socat >/dev/null 2>&1; then
        missing+=("socat")
    fi
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "ERROR: missing required commands: ${missing[*]}"
        echo "Install with: sudo apt install qemu-system-x86 cloud-image-utils socat"
        exit 1
    fi
}

resolve_ssh_key() {
    if [[ -n "${BITPROTECTOR_QEMU_SSH_KEY:-}" ]]; then
        printf '%s\n' "${BITPROTECTOR_QEMU_SSH_KEY}"
        return 0
    fi

    local key
    for key in "${HOME}/.ssh/id_ed25519.pub" "${HOME}/.ssh/id_rsa.pub"; do
        if [[ -f "${key}" ]]; then
            cat "${key}"
            return 0
        fi
    done

    echo "ERROR: no SSH public key found. Generate one with: ssh-keygen -t ed25519" >&2
    echo "       or set BITPROTECTOR_QEMU_SSH_KEY to the public key text." >&2
    exit 1
}

require_commands
SSH_KEY="$(resolve_ssh_key)"

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

create_data_disk() {
    local path="$1"
    if [[ ! -f "${path}" ]]; then
        qemu-img create -f qcow2 "${path}" 4G >/dev/null
    fi
}

create_data_disk "${WORKDIR}/primary.qcow2"
create_data_disk "${WORKDIR}/mirror.qcow2"
create_data_disk "${WORKDIR}/replacement-primary.qcow2"
create_data_disk "${WORKDIR}/replacement-secondary.qcow2"
create_data_disk "${WORKDIR}/spare1.qcow2"
create_data_disk "${WORKDIR}/spare2.qcow2"

if [[ "${FIRST_RUN}" == "true" ]]; then
    echo "Creating cloud-init seed ISO..."

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
      setup_disk bpspare1 /mnt/spare1
      setup_disk bpspare2 /mnt/spare2

      mount -a
      mkdir -p /tmp/bitprotector-virtual
      chown -R testuser:testuser \
          /mnt/primary \
          /mnt/mirror \
          /mnt/replacement-primary \
          /mnt/replacement-secondary \
          /mnt/spare1 \
          /mnt/spare2 \
          /tmp/bitprotector-virtual
  - path: /etc/systemd/system/bitprotector.service.d/manual-qemu.conf
    permissions: '0644'
    content: |
      [Service]
      NoNewPrivileges=false
      User=root
      Group=root
      ReadWritePaths=
      ReadWritePaths=/var/lib/bitprotector /var/log/bitprotector /var/lib/bitprotector/virtual /mnt
      ExecStart=
      ExecStart=/usr/bin/bitprotector \
          --db /var/lib/bitprotector/bitprotector.db \
          serve \
          --host 0.0.0.0 \
          --port 8443 \
          --tls-cert /etc/bitprotector/tls/cert.pem \
          --tls-key /etc/bitprotector/tls/key.pem

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-qemu-storage.sh
  - apt-get update -q
  - apt-get install -y /mnt/debpkg/$(basename "${DEB_FILE}")
  - usermod -a -G bitprotector testuser || true
  - systemctl daemon-reload
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

    cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-manual
local-hostname: bitprotector-dev
CLOUDINIT

    cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"
fi

QMP_ARGS=()
if [[ "${ENABLE_QMP}" == "1" ]]; then
    QMP_ARGS=(-qmp "unix:${QMP_SOCKET},server=on,wait=off")
fi

echo ""
echo "========================================="
echo "  BitProtector Manual QEMU Session"
echo "========================================="
echo ""
echo "  SSH:  ssh -o StrictHostKeyChecking=no -p ${SSH_PORT} testuser@localhost"
echo "  API:  https://localhost:${API_PORT}"
echo ""
echo "  Mounted guest disks:"
echo "    /mnt/primary"
echo "    /mnt/mirror"
echo "    /mnt/replacement-primary"
echo "    /mnt/replacement-secondary"
echo "    /mnt/spare1"
echo "    /mnt/spare2"
echo "    /tmp/bitprotector-virtual"
echo ""
echo "  Shared package dir (host -> /mnt/debpkg in VM):"
echo "    $(dirname "${DEB_FILE}")"
echo ""
echo "  VM state persists at: ${WORKDIR}"
echo "  To start fresh:       FRESH=1 $0"
if [[ "${ENABLE_QMP}" == "1" ]]; then
    echo "  QMP socket:           ${QMP_SOCKET}"
fi
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
    "${QMP_ARGS[@]}" \
    -drive "file=${WORKDIR}/vm.qcow2,format=qcow2" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-primary,file=${WORKDIR}/primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-primary,id=dev-primary,serial=bpprimary" \
    -drive "if=none,id=drive-mirror,file=${WORKDIR}/mirror.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-mirror,id=dev-mirror,serial=bpmirror" \
    -drive "if=none,id=drive-replacement-primary,file=${WORKDIR}/replacement-primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-replacement-primary,id=dev-replacement-primary,serial=bpreplprimary" \
    -drive "if=none,id=drive-replacement-secondary,file=${WORKDIR}/replacement-secondary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-replacement-secondary,id=dev-replacement-secondary,serial=bpreplsecondary" \
    -drive "if=none,id=drive-spare1,file=${WORKDIR}/spare1.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-spare1,id=dev-spare1,serial=bpspare1" \
    -drive "if=none,id=drive-spare2,file=${WORKDIR}/spare2.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-spare2,id=dev-spare2,serial=bpspare2" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=$(dirname "${DEB_FILE}"),mount_tag=debpkg,security_model=passthrough,id=debpkg"
