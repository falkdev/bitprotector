#!/bin/bash
# tests/installation/e2e-guest.sh
# Boot a transient QEMU guest with BitProtector installed for Playwright E2E tests.
#
# The guest is started in the background and its PID is saved to:
#   ${RUNNER_TEMP:-/tmp}/e2e-qemu.pid
#
# After tests complete the caller is responsible for stopping the VM:
#   kill "$(cat "${RUNNER_TEMP:-/tmp}/e2e-qemu.pid")"
#
# The guest provisions four virtual disks that back the drive paths used by the
# Playwright test fixtures:
#   /mnt/primary              (serial=bpprimary)
#   /mnt/mirror               (serial=bpmirror)
#   /mnt/replacement-primary  (serial=bpreplprimary)
#   /mnt/spare1               (serial=bpspare1)
# All directories are owned by testuser so the SSH-based fixture helpers can
# create files and directories without sudo.
#
# The bitprotector service is configured via a cloud-init-written systemd
# override (e2e-qemu.conf) that binds to 0.0.0.0 and uses root, matching the
# manual-qemu.conf that frontend_qemu_manual.sh applies later.  When
# frontend_qemu_manual.sh runs as Playwright's webServer it finds the VM
# already up, re-applies the override (idempotent), and starts the Vite proxy.
#
# Usage:
#   ./tests/installation/e2e-guest.sh [/path/to/bitprotector.deb]
#   SSH_PORT=2280 API_PORT=18480 ./tests/installation/e2e-guest.sh
#
# Environment variables (all optional):
#   GUEST_IMAGE  — ubuntu-24.04 (default), ubuntu-26.04, or absolute path
#   SSH_PORT     — host-side port forwarded to guest SSH   (default: 2280)
#   API_PORT     — host-side port forwarded to guest :8443 (default: 18480)
#   TIMEOUT      — seconds to wait for VM boot             (default: 600)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
LIB_DIR="${SCRIPT_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2280}"
API_PORT="${API_PORT:-18480}"
TIMEOUT="${TIMEOUT:-600}"

PID_FILE="${RUNNER_TEMP:-/tmp}/e2e-qemu.pid"

require_commands qemu-system-x86_64 qemu-img cloud-localds ssh ssh-keygen

SSH_KEY="$(resolve_ssh_key)"
UBUNTU_IMAGE="$(resolve_guest_image)"

DEB_FILE=$(ls -1 ${DEB_PATH} 2>/dev/null | head -1 || true)
if [[ -z "${DEB_FILE}" ]]; then
    log ERROR ".deb file not found at ${DEB_PATH}"
    echo "Build with: cargo deb"
    exit 1
fi
DEB_NAME="$(basename "${DEB_FILE}")"

if [[ ! -f "${UBUNTU_IMAGE}" ]]; then
    log ERROR "cloud image not found at ${UBUNTU_IMAGE}"
    echo "Run: ./scripts/setup-qemu.sh"
    exit 1
fi

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-e2e-$$"
mkdir -p "${WORKDIR}"
mkdir -p "${WORKDIR}/debpkg"
cp -f "${DEB_FILE}" "${WORKDIR}/debpkg/${DEB_NAME}"

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true

# ---------------------------------------------------------------------------
# Disk images
# ---------------------------------------------------------------------------

qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/test.qcow2"

create_disk() { qemu-img create -f qcow2 "${WORKDIR}/${1}.qcow2" 4G; }
create_disk primary
create_disk mirror
create_disk replacement-primary
create_disk spare1

# ---------------------------------------------------------------------------
# Cloud-init
# ---------------------------------------------------------------------------

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
  - path: /usr/local/bin/bitprotector-e2e-storage.sh
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
          if [[ ! -b "\${dev}" ]]; then
              echo "ERROR: disk \${serial} not found at \${dev}" >&2
              exit 1
          fi
          mkdir -p "\${mount_point}"
          if ! blkid "\${dev}" >/dev/null 2>&1; then
              mkfs.ext4 -F "\${dev}"
          fi
          local uuid
          uuid=\$(blkid -s UUID -o value "\${dev}")
          grep -q "\${uuid}" /etc/fstab || \\
              echo "UUID=\${uuid} \${mount_point} ext4 defaults,nofail 0 2" >> /etc/fstab
      }
      setup_disk bpprimary     /mnt/primary
      setup_disk bpmirror      /mnt/mirror
      setup_disk bpreplprimary /mnt/replacement-primary
      setup_disk bpspare1      /mnt/spare1
      mount -a
      mkdir -p /tmp/bitprotector-virtual
      chown -R testuser:testuser \\
          /mnt/primary \\
          /mnt/mirror \\
          /mnt/replacement-primary \\
          /mnt/spare1 \\
          /tmp/bitprotector-virtual

  - path: /etc/systemd/system/bitprotector.service.d/e2e-qemu.conf
    permissions: '0644'
    content: |
      [Service]
      NoNewPrivileges=false
      User=root
      Group=root
      ReadWritePaths=
      ReadWritePaths=/var/lib/bitprotector /var/log/bitprotector /var/lib/bitprotector/virtual /mnt
      ExecStart=
      ExecStart=/usr/bin/bitprotector \\
          --db /var/lib/bitprotector/bitprotector.db \\
          serve \\
          --host 0.0.0.0 \\
          --port 8443 \\
          --tls-cert /etc/bitprotector/tls/cert.pem \\
          --tls-key /etc/bitprotector/tls/key.pem

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - /usr/local/bin/bitprotector-e2e-storage.sh
  - apt-get install -y -q /mnt/debpkg/${DEB_NAME}
  - usermod -a -G bitprotector testuser || true
  - mkdir -p /etc/bitprotector/tls
  - openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' -keyout /etc/bitprotector/tls/key.pem -out /etc/bitprotector/tls/cert.pem
  - chown -R bitprotector:bitprotector /etc/bitprotector/tls
  - chmod 600 /etc/bitprotector/tls/key.pem
  - chmod 644 /etc/bitprotector/tls/cert.pem
  - |
    cat > /etc/bitprotector/config.toml <<'EOF'
    [server]
    host = "0.0.0.0"
    port = 8443
    rate_limit_rps = 100
    jwt_secret = "change-me-in-production"
    tls_cert = "/etc/bitprotector/tls/cert.pem"
    tls_key = "/etc/bitprotector/tls/key.pem"

    [database]
    path = "/var/lib/bitprotector/bitprotector.db"
    EOF
  - systemctl daemon-reload
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-e2e
local-hostname: bitprotector-e2e
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

# ---------------------------------------------------------------------------
# Start QEMU (background — VM stays alive after this script exits)
# ---------------------------------------------------------------------------

log INFO "Starting QEMU e2e guest (SSH=localhost:${SSH_PORT}, API=localhost:${API_PORT})..."

qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/test.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-primary,file=${WORKDIR}/primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-primary,id=dev-primary,serial=bpprimary" \
    -drive "if=none,id=drive-mirror,file=${WORKDIR}/mirror.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-mirror,id=dev-mirror,serial=bpmirror" \
    -drive "if=none,id=drive-replacement-primary,file=${WORKDIR}/replacement-primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-replacement-primary,id=dev-replacement-primary,serial=bpreplprimary" \
    -drive "if=none,id=drive-spare1,file=${WORKDIR}/spare1.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-spare1,id=dev-spare1,serial=bpspare1" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${WORKDIR}/debpkg,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &

QEMU_PID=$!

# Save PID immediately so the caller can clean up even if wait_for_vm fails.
echo "${QEMU_PID}" > "${PID_FILE}"
log INFO "QEMU PID ${QEMU_PID} saved to ${PID_FILE}"

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"

log INFO "E2E guest ready — SSH=localhost:${SSH_PORT}, API=localhost:${API_PORT}"
