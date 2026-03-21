#!/bin/bash
# scripts/setup-qemu.sh
# Install prerequisites for qemu_manual.sh, tests/installation/qemu_test.sh,
# and tests/installation/qemu_failover_test.sh.
#
# Usage:
#   ./scripts/setup-qemu.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
UBUNTU_IMAGE="${UBUNTU_IMAGE:-${HOME}/images/noble-server-cloudimg-amd64.img}"
UBUNTU_IMAGE_URL="https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img"

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

echo ""
echo "=== Ubuntu 24 cloud image ==="

if [[ -f "${UBUNTU_IMAGE}" ]]; then
    echo "Already present: ${UBUNTU_IMAGE}"
else
    echo "Downloading Ubuntu 24 (Noble) cloud image to ${UBUNTU_IMAGE}..."
    mkdir -p "$(dirname "${UBUNTU_IMAGE}")"
    wget --show-progress -O "${UBUNTU_IMAGE}.tmp" "${UBUNTU_IMAGE_URL}"
    mv "${UBUNTU_IMAGE}.tmp" "${UBUNTU_IMAGE}"
    echo "Download complete."
fi

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
echo ""
echo "  Failover / replacement QEMU test:"
echo "    ./tests/installation/qemu_failover_test.sh"
echo "========================================="
