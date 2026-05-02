#!/bin/bash
# tests/installation/bundles/smoke.sh
# Smoke bundle: installation, service, auth, TLS, and reboot persistence tests.
#
# Cloud-init provisions:
#   - standard package install
#   - TLS cert/key at /etc/bitprotector/tls/
#   - config.toml with TLS settings
#   - PAM test user testauth/hunter2
#   - service start
#
# Scenarios:
#   E1   smoke-01-package-installed
#   #--  smoke-02-service-active-with-tls
#   E2   smoke-03-cli-smoke
#   E3   smoke-04-profile-d-installed
#   #29  smoke-05-profile-d-execution
#   #28  smoke-06-ldd-version-sanity
#   #11  smoke-07-journald-integration
#   #13  smoke-08-pam-login
#   #14  smoke-09-jwt-persists-across-restart
#   #15  smoke-10-tls-cert-rotation
#   #16  smoke-11-path-traversal-rejected
#   #10  smoke-12-reboot-persistence
#   #17  smoke-13-database-backup-repair-restore
#
# Prerequisites:
#   - qemu-system-x86_64, qemu-img, cloud-localds, ssh, ssh-keygen
#   - Ubuntu cloud image (GUEST_IMAGE / UBUNTU_IMAGE env vars)
#   - SSH public key in ~/.ssh or BITPROTECTOR_QEMU_SSH_KEY set
#   - bitprotector.deb built via: cargo deb
#
# Usage:
#   ./tests/installation/bundles/smoke.sh [/path/to/bitprotector.deb]
#
# Exit codes: 0 all passed, non-zero on failure

set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$(cd "${BUNDLE_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${INSTALL_DIR}/../.." && pwd)"
SCENARIOS_DIR="${INSTALL_DIR}/scenarios/smoke"
LIB_DIR="${INSTALL_DIR}/lib"

# shellcheck source=tests/installation/lib/qemu-helpers.sh
source "${LIB_DIR}/qemu-helpers.sh"
# shellcheck source=tests/installation/lib/scenarios.sh
source "${LIB_DIR}/scenarios.sh"

DEB_PATH="${1:-${PROJECT_ROOT}/target/debian/bitprotector_*.deb}"
SSH_PORT="${SSH_PORT:-2222}"
API_PORT="${API_PORT:-18443}"
TIMEOUT="${TIMEOUT:-600}"

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

WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/qemu-smoke-$$"
mkdir -p "${WORKDIR}"
trap 'rm -rf "${WORKDIR}"; if [[ -n "${QEMU_PID:-}" ]]; then kill "${QEMU_PID}" 2>/dev/null || true; fi' EXIT

ssh-keygen -f "${HOME}/.ssh/known_hosts" -R "[localhost]:${SSH_PORT}" 2>/dev/null || true

qemu-img create -f qcow2 -b "${UBUNTU_IMAGE}" -F qcow2 "${WORKDIR}/test.qcow2"

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

runcmd:
  - mkdir -p /mnt/debpkg
  - mount -t 9p -o trans=virtio debpkg /mnt/debpkg || true
  - apt-get update -q
  - apt-get install -y -q jq openssl curl /mnt/debpkg/bitprotector*.deb
  - mkdir -p /etc/bitprotector/tls
  - openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' -keyout /etc/bitprotector/tls/key.pem -out /etc/bitprotector/tls/cert.pem
  - chown -R bitprotector:bitprotector /etc/bitprotector/tls
  - chmod 600 /etc/bitprotector/tls/key.pem
  - chmod 644 /etc/bitprotector/tls/cert.pem
  - |
    cat > /etc/bitprotector/config.toml <<'EOF'
    [server]
    host = "127.0.0.1"
    port = 8443
    rate_limit_rps = 100
    jwt_secret = "change-me-in-production"
    tls_cert = "/etc/bitprotector/tls/cert.pem"
    tls_key = "/etc/bitprotector/tls/key.pem"

    [database]
    path = "/var/lib/bitprotector/bitprotector.db"
    EOF
  - id -u testauth >/dev/null 2>&1 || useradd -m testauth
  - echo 'testauth:hunter2' | chpasswd
  - mkdir -p /etc/systemd/system/bitprotector.service.d
  - |
    cat > /etc/systemd/system/bitprotector.service.d/smoke-qemu.conf <<'EOF'
    [Service]
    User=root
    Group=root
    EOF
  - systemctl daemon-reload
  - systemctl enable bitprotector || true
  - systemctl start bitprotector || true
  - touch /tmp/install-done
CLOUDINIT

cat > "${WORKDIR}/meta-data" <<'CLOUDINIT'
instance-id: bitprotector-test
local-hostname: bitprotector-test
CLOUDINIT

cloud-localds "${WORKDIR}/seed.iso" "${WORKDIR}/user-data" "${WORKDIR}/meta-data"

log INFO "Starting QEMU VM (smoke bundle)..."
qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4096 \
    -display none \
    -serial file:"${WORKDIR}/serial.log" \
    -drive "file=${WORKDIR}/test.qcow2,format=qcow2,cache=unsafe" \
    -drive "file=${WORKDIR}/seed.iso,format=raw,readonly=on,if=virtio" \
    -net nic \
    -net "user,hostfwd=tcp::${SSH_PORT}-:22,hostfwd=tcp::${API_PORT}-:8443" \
    -virtfs "local,path=${PROJECT_ROOT}/target/debian,mount_tag=debpkg,security_model=passthrough,id=debpkg" \
    > "${WORKDIR}/qemu.log" 2>&1 &
QEMU_PID=$!

wait_for_vm "${QEMU_PID}" "${SSH_PORT}" "${TIMEOUT}" "${WORKDIR}"

# --- Scenarios ---

BUNDLE_START_TIME="$(date -Iseconds)"

# shellcheck source=tests/installation/scenarios/smoke/smoke-01-package-installed.sh
source "${SCENARIOS_DIR}/smoke-01-package-installed.sh"
run_scenario "smoke-01-package-installed" smoke_01_package_installed

# shellcheck source=tests/installation/scenarios/smoke/smoke-02-service-active-with-tls.sh
source "${SCENARIOS_DIR}/smoke-02-service-active-with-tls.sh"
run_scenario "smoke-02-service-active-with-tls" smoke_02_service_active_with_tls

# shellcheck source=tests/installation/scenarios/smoke/smoke-03-cli-smoke.sh
source "${SCENARIOS_DIR}/smoke-03-cli-smoke.sh"
run_scenario "smoke-03-cli-smoke" smoke_03_cli_smoke

# shellcheck source=tests/installation/scenarios/smoke/smoke-04-profile-d-installed.sh
source "${SCENARIOS_DIR}/smoke-04-profile-d-installed.sh"
run_scenario "smoke-04-profile-d-installed" smoke_04_profile_d_installed

# shellcheck source=tests/installation/scenarios/smoke/smoke-05-profile-d-execution.sh
source "${SCENARIOS_DIR}/smoke-05-profile-d-execution.sh"
run_scenario "smoke-05-profile-d-execution" smoke_05_profile_d_execution

# shellcheck source=tests/installation/scenarios/smoke/smoke-06-ldd-version-sanity.sh
source "${SCENARIOS_DIR}/smoke-06-ldd-version-sanity.sh"
run_scenario "smoke-06-ldd-version-sanity" smoke_06_ldd_version_sanity

# shellcheck source=tests/installation/scenarios/smoke/smoke-07-journald-integration.sh
source "${SCENARIOS_DIR}/smoke-07-journald-integration.sh"
run_scenario "smoke-07-journald-integration" smoke_07_journald_integration

# shellcheck source=tests/installation/scenarios/smoke/smoke-08-pam-login.sh
source "${SCENARIOS_DIR}/smoke-08-pam-login.sh"
run_scenario "smoke-08-pam-login" smoke_08_pam_login

# shellcheck source=tests/installation/scenarios/smoke/smoke-09-jwt-persists-across-restart.sh
source "${SCENARIOS_DIR}/smoke-09-jwt-persists-across-restart.sh"
run_scenario "smoke-09-jwt-persists-across-restart" smoke_09_jwt_persists_across_restart

# shellcheck source=tests/installation/scenarios/smoke/smoke-10-tls-cert-rotation.sh
source "${SCENARIOS_DIR}/smoke-10-tls-cert-rotation.sh"
run_scenario "smoke-10-tls-cert-rotation" smoke_10_tls_cert_rotation

# shellcheck source=tests/installation/scenarios/smoke/smoke-11-path-traversal-rejected.sh
source "${SCENARIOS_DIR}/smoke-11-path-traversal-rejected.sh"
run_scenario "smoke-11-path-traversal-rejected" smoke_11_path_traversal_rejected

# shellcheck source=tests/installation/scenarios/smoke/smoke-12-reboot-persistence.sh
source "${SCENARIOS_DIR}/smoke-12-reboot-persistence.sh"
run_scenario "smoke-12-reboot-persistence" smoke_12_reboot_persistence

# shellcheck source=tests/installation/scenarios/smoke/smoke-13-database-backup-repair-restore.sh
source "${SCENARIOS_DIR}/smoke-13-database-backup-repair-restore.sh"
run_scenario "smoke-13-database-backup-repair-restore" smoke_13_database_backup_repair_restore

run_scenario "journal-error-scraper" journal_error_scraper

echo ""
echo "=== All smoke tests passed ==="
