#!/bin/bash
# scripts/setup-qemu.sh
# Install prerequisites for qemu_manual.sh and the QEMU test suites, then
# download the requested Ubuntu cloud image(s).
#
# Usage:
#   ./scripts/setup-qemu.sh [GUEST]
#
# GUEST may be:
#   24.04   — download Ubuntu 24.04 Noble only
#   26.04   — download Ubuntu 26.04 Plucky only
#   all     — download both (default)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
GUEST="${1:-all}"
IMAGES_DIR="${HOME}/images"

NOBLE_IMAGE="${IMAGES_DIR}/noble-server-cloudimg-amd64.img"
NOBLE_URL="https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"

# 26.04 LTS codename is "Plucky Puffin". Image URL follows the standard pattern.
PLUCKY_IMAGE="${IMAGES_DIR}/plucky-server-cloudimg-amd64.img"
PLUCKY_URL="https://cloud-images.ubuntu.com/plucky/current/plucky-server-cloudimg-amd64.img"

echo "=== Installing system packages ==="

MISSING_PKGS=()
command -v qemu-system-x86_64 &>/dev/null || MISSING_PKGS+=(qemu-system-x86)
command -v qemu-img            &>/dev/null || MISSING_PKGS+=(qemu-utils)
command -v cloud-localds       &>/dev/null || MISSING_PKGS+=(cloud-image-utils)
command -v socat               &>/dev/null || MISSING_PKGS+=(socat)
command -v ssh                 &>/dev/null || MISSING_PKGS+=(openssh-client)

if [[ ${#MISSING_PKGS[@]} -gt 0 ]]; then
    echo "Installing: ${MISSING_PKGS[*]}"
    sudo apt-get update -q
    sudo apt-get install -y "${MISSING_PKGS[@]}"
else
    echo "All required packages already installed."
fi

echo ""
echo "=== cargo-deb ==="

if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo is required to install cargo-deb, but it was not found on PATH." >&2
    echo "Install the Rust toolchain first, or add ~/.cargo/bin to PATH if it is already installed." >&2
    exit 1
fi

if cargo deb --version &>/dev/null; then
    echo "cargo-deb already installed: $(cargo deb --version)"
else
    echo "Installing cargo-deb..."
    cargo install cargo-deb
fi

download_image() {
    local dest="$1"
    local url="$2"
    if [[ -f "${dest}" ]]; then
        echo "Already present: ${dest}"
    else
        echo "Downloading $(basename "${dest}") to ${dest}..."
        mkdir -p "$(dirname "${dest}")"
        wget --show-progress -O "${dest}.tmp" "${url}"
        mv "${dest}.tmp" "${dest}"
        echo "Download complete."
    fi
}

echo ""
echo "=== Ubuntu cloud image(s) ==="

case "${GUEST}" in
    24.04)
        download_image "${NOBLE_IMAGE}" "${NOBLE_URL}"
        ;;
    26.04)
        download_image "${PLUCKY_IMAGE}" "${PLUCKY_URL}"
        ;;
    all)
        download_image "${NOBLE_IMAGE}" "${NOBLE_URL}"
        download_image "${PLUCKY_IMAGE}" "${PLUCKY_URL}"
        ;;
    *)
        echo "ERROR: unknown GUEST '${GUEST}'. Use 24.04, 26.04, or all." >&2
        exit 1
        ;;
esac

echo ""
echo "=== SSH key ==="

SSH_KEY_FILE=""
for candidate in "${HOME}/.ssh/id_ed25519" "${HOME}/.ssh/id_rsa"; do
    if [[ -f "${candidate}.pub" ]]; then
        SSH_KEY_FILE="${candidate}"
        break
    fi
done

if [[ -z "${SSH_KEY_FILE}" ]]; then
    echo "No SSH key found. Generating ~/.ssh/id_ed25519 ..."
    mkdir -p "${HOME}/.ssh"
    chmod 700 "${HOME}/.ssh"
    ssh-keygen -t ed25519 -N "" -f "${HOME}/.ssh/id_ed25519"
    SSH_KEY_FILE="${HOME}/.ssh/id_ed25519"
    echo "Generated: ${SSH_KEY_FILE}.pub"
else
    echo "Using existing key: ${SSH_KEY_FILE}.pub"
fi

echo ""
echo "========================================="
echo "  Setup complete. You can now run:"
echo ""
echo "  Manual VM:"
echo "    ./scripts/qemu_manual.sh"
echo "    ENABLE_QMP=1 ./scripts/qemu_manual.sh"
echo ""
echo "  Installation smoke test:"
echo "    ./tests/installation/qemu_test.sh"
echo "    GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_test.sh"
echo ""
echo "  Failover / replacement QEMU test:"
echo "    ./tests/installation/qemu_failover_test.sh"
echo ""
echo "  Full uninstall (purge) QEMU test:"
echo "    ./tests/installation/qemu_uninstall_test.sh"
echo ""
echo "  Run all local tests natively:"
echo "    ./scripts/run-tests.sh smoke"
echo "========================================="
