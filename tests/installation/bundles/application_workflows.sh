#!/bin/bash
# tests/installation/bundles/application_workflows.sh
# Application workflows bundle: scheduler + backup workflows under moderate load.

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/application-workflows"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2302}"
API_PORT="${API_PORT:-19343}"
TIMEOUT="${TIMEOUT:-900}"
export SSH_VM_TIMEOUT="${SSH_VM_TIMEOUT:-600}"

export APP_SERVICE_DB="/var/lib/bitprotector/bitprotector.db"
export APP_PRIMARY_ROOT="/mnt/app-primary"
export APP_MIRROR_ROOT="/mnt/app-mirror"
export APP_SPARE_ROOT="/mnt/app-spare"

require_commands qemu-system-x86_64 qemu-img cloud-localds ssh ssh-keygen curl jq
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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-application-workflows-$$"
mkdir -p "${WORKDIR}"
_cleanup() {
    local _exit=$?
    if [[ $_exit -ne 0 ]] && [[ -n "${RUNNER_TEMP:-}" ]]; then
        local _art="${RUNNER_TEMP}/qemu-application-workflows-artifacts-$$"
        mkdir -p "${_art}"
        cp "${WORKDIR}/serial.log" "${_art}/" 2>/dev/null || true
        cp "${WORKDIR}/qemu.log" "${_art}/" 2>/dev/null || true
    fi
    rm -rf "${WORKDIR}"
    if [[ -n "${QEMU_PID:-}" ]]; then
        kill "${QEMU_PID}" 2>/dev/null || true
    fi
}
trap _cleanup EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true
qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/vm.qcow2"
qemu-img create -f qcow2 "${WORKDIR}/primary.qcow2" 12G
qemu-img create -f qcow2 "${WORKDIR}/mirror.qcow2" 12G
qemu-img create -f qcow2 "${WORKDIR}/spare.qcow2" 12G
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
  - path: /usr/local/bin/bitprotector-app-workflow-storage.sh
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
        if ! [[ -b "\${dev}" ]]; then
          echo "Disk not present: \${serial} (expected \${dev})" >&2
          ls -l /dev/disk/by-id >&2 || true
          exit 1
        fi
        mkdir -p "\${mount_point}"
        if ! blkid "\${dev}" >/dev/null 2>&1; then
          mkfs.ext4 -F "\${dev}"
        fi
        local uuid
        uuid=\$(blkid -s UUID -o value "\${dev}")
        grep -q "\${uuid}" /etc/fstab || echo "UUID=\${uuid} \${mount_point} ext4 defaults,nofail 0 2" >> /etc/fstab
      }
      setup_disk bpappprimary /mnt/app-primary
      setup_disk bpappmirror /mnt/app-mirror
      setup_disk bpappspare /mnt/app-spare
      setup_disk bpdb /mnt/bitprotector-db
      mount -a
      mkdir -p /mnt/bitprotector-db/db
      chown -R testuser:testuser /mnt/app-primary /mnt/app-mirror /mnt/app-spare /mnt/bitprotector-db

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get update -q
  - apt-get install -y -q jq openssl curl /mnt/debpkg/bitprotector*.deb
  - /usr/local/bin/bitprotector-app-workflow-storage.sh
  - mkdir -p /etc/bitprotector/tls
  - openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' -keyout /etc/bitprotector/tls/key.pem -out /etc/bitprotector/tls/cert.pem
  - chown -R bitprotector:bitprotector /etc/bitprotector/tls
  - chmod 600 /etc/bitprotector/tls/key.pem
  - chmod 644 /etc/bitprotector/tls/cert.pem
  - |
    cat > /etc/bitprotector/config.toml <<'APPWORKFLOWCFG'
    [server]
    host = "127.0.0.1"
    port = 8443
    rate_limit_rps = 100
    jwt_secret = "change-me-in-production"
    tls_cert = "/etc/bitprotector/tls/cert.pem"
    tls_key = "/etc/bitprotector/tls/key.pem"

    [database]
    path = "/var/lib/bitprotector/bitprotector.db"

    [checksum]
    hdd_max_parallel = 2
    ssd_max_parallel = 0
    APPWORKFLOWCFG
  - id -u testauth >/dev/null 2>&1 || useradd -m testauth
  - echo 'testauth:hunter2' | chpasswd
  - mkdir -p /etc/systemd/system/bitprotector.service.d
  - |
    cat > /etc/systemd/system/bitprotector.service.d/qemu-application-workflows.conf <<'APPWORKFLOWSVC'
    [Service]
    User=root
    Group=root
    PrivateTmp=no
    ProtectSystem=no
    APPWORKFLOWSVC
  - systemctl daemon-reload
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-application-workflows
local-hostname: bitprotector-application-workflows
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (application_workflows bundle)..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/vm.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -drive "if=none,id=drive-primary,file=${WORKDIR}/primary.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-primary,id=dev-primary,serial=bpappprimary" \
    -drive "if=none,id=drive-mirror,file=${WORKDIR}/mirror.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-mirror,id=dev-mirror,serial=bpappmirror" \
    -drive "if=none,id=drive-spare,file=${WORKDIR}/spare.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-spare,id=dev-spare,serial=bpappspare" \
    -drive "if=none,id=drive-bpdb,file=${WORKDIR}/bpdb.qcow2,format=qcow2" \
    -device "virtio-blk-pci,drive=drive-bpdb,id=dev-bpdb,serial=bpdb" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"
wait_for_api "${API_PORT}" 180
ssh_vm '
set -euo pipefail
for mp in /mnt/app-primary /mnt/app-mirror /mnt/app-spare /mnt/bitprotector-db; do
  findmnt "$mp" >/dev/null 2>&1 || { echo "Expected mount missing: $mp" >&2; exit 1; }
  touch "$mp/.write-test"
  rm -f "$mp/.write-test"
done
'

BUNDLE_START_TIME="$(date -Iseconds)"

# shellcheck source=tests/installation/scenarios/application-workflows/app-01-scheduled-sync-integrity-moderate-dataset.sh
source "${SCENARIOS_DIR}/app-01-scheduled-sync-integrity-moderate-dataset.sh"
run_scenario "app-01-scheduled-sync-integrity-moderate-dataset" app_01_scheduled_sync_integrity_moderate_dataset

# shellcheck source=tests/installation/scenarios/application-workflows/app-02-database-backup-during-churn.sh
source "${SCENARIOS_DIR}/app-02-database-backup-during-churn.sh"
run_scenario "app-02-database-backup-during-churn" app_02_database_backup_during_churn

# shellcheck source=tests/installation/scenarios/application-workflows/app-03-backup-integrity-repairs-peer.sh
source "${SCENARIOS_DIR}/app-03-backup-integrity-repairs-peer.sh"
run_scenario "app-03-backup-integrity-repairs-peer" app_03_backup_integrity_repairs_peer

# shellcheck source=tests/installation/scenarios/application-workflows/app-04-restart-reloads-schedules.sh
source "${SCENARIOS_DIR}/app-04-restart-reloads-schedules.sh"
run_scenario "app-04-restart-reloads-schedules" app_04_restart_reloads_schedules

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All application-workflow tests passed ==="
