#!/bin/bash
# scripts/ci-local.sh
# Run CI pipeline layers locally via `act` (https://github.com/nektos/act).
# Single source of truth is the workflow YAML in .github/workflows/ci.yml.
#
# Usage:
#   ./scripts/ci-local.sh <layer> [extra act args...]
#
# Layers:
#   lint   — Layer 0: cargo fmt + clippy + npm lint + prettier
#   fast   — Layers 0-3: lint + unit + integration (no scaling_100k or QEMU)
#   smoke  — Layers 0-5: fast + build .deb + QEMU smoke (requires /dev/kvm)
#   full   — Layers 0-7: smoke + QEMU failover + uninstall (requires /dev/kvm)
#
# Examples:
#   ./scripts/ci-local.sh lint
#   ./scripts/ci-local.sh smoke
#   ./scripts/ci-local.sh smoke --matrix guest:ubuntu-24.04
#   ./scripts/ci-local.sh full --job qemu-failover

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# ---------------------------------------------------------------------------
# Preflight checks
# ---------------------------------------------------------------------------

preflight() {
    local errors=0

    if ! command -v act &>/dev/null; then
        echo "ERROR: 'act' is not installed." >&2
        echo "Install from: https://github.com/nektos/act#installation" >&2
        echo "  brew install act   (macOS)"
        echo "  curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash   (Linux)"
        errors=$((errors + 1))
    fi

    if ! docker info &>/dev/null 2>&1; then
        echo "ERROR: Docker is not running. Start Docker before using ci-local.sh." >&2
        errors=$((errors + 1))
    fi

    if [[ "${LAYER}" == "smoke" || "${LAYER}" == "full" ]]; then
        if [[ ! -r /dev/kvm ]]; then
            echo "WARNING: /dev/kvm is not readable. QEMU layers will use TCG (very slow)." >&2
            echo "         Fix with: sudo chmod 666 /dev/kvm" >&2
        fi
    fi

    if [[ ${errors} -gt 0 ]]; then
        exit 1
    fi
}

# ---------------------------------------------------------------------------
# Job lists per layer
# ---------------------------------------------------------------------------

jobs_for_lint="lint"
jobs_for_fast="lint,unit,rust-integration-fast,rust-integration-heavy"
jobs_for_smoke="lint,unit,rust-integration-fast,rust-integration-heavy,build-artifacts,qemu-smoke"
jobs_for_full="lint,unit,rust-integration-fast,rust-integration-heavy,build-artifacts,qemu-smoke,qemu-failover,qemu-uninstall"

# ---------------------------------------------------------------------------
# Parse args
# ---------------------------------------------------------------------------

LAYER="${1:-}"
if [[ -z "${LAYER}" ]]; then
    grep '^# ' "$0" | sed 's/^# //' | tail -n +2
    exit 1
fi
shift

case "${LAYER}" in
    lint)  JOBS="${jobs_for_lint}" ;;
    fast)  JOBS="${jobs_for_fast}" ;;
    smoke) JOBS="${jobs_for_smoke}" ;;
    full)  JOBS="${jobs_for_full}" ;;
    *)
        echo "Unknown layer: ${LAYER}"
        exit 1
        ;;
esac

preflight

# ---------------------------------------------------------------------------
# Build the act command
# ---------------------------------------------------------------------------

ACT_ARGS=(
    # Use catthehacker/ubuntu:full-latest so qemu-system-x86_64 is available
    -P "ubuntu-24.04=catthehacker/ubuntu:full-latest"
    # Pass KVM into the container
    --container-options "--device /dev/kvm --privileged"
    # Don't pull if image already present
    --pull=false
    # Restrict to the jobs this layer needs
    --job "${JOBS}"
    # Trigger as push to simulate main (or PR — adjust if needed)
    push
    # Extra args forwarded from the command line
    "$@"
)

echo "Running layer '${LAYER}' via act..."
echo "Jobs: ${JOBS}"
echo ""

cd "${PROJECT_ROOT}"
act "${ACT_ARGS[@]}"
