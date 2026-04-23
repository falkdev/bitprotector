#!/bin/bash
# scripts/run-tests.sh
# Native (no Docker) test runner — mirrors the CI layer structure.
# Requires: Rust toolchain, Node 20.19+, and (for smoke/full) QEMU prerequisites.
#
# Usage:
#   ./scripts/run-tests.sh <layer>
#
# Layers:
#   lint   — cargo fmt + clippy, npm lint + prettier (Layer 0)
#   fast   — lint + unit tests + integration tests excluding scaling_100k (Layers 0-3)
#   smoke  — fast + build .deb + QEMU smoke on ubuntu-24.04 and ubuntu-26.04 (Layers 0-5)
#   full   — smoke + QEMU failover + uninstall (Layers 0-7)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
    grep '^# ' "$0" | sed 's/^# //' | tail -n +2
    exit 1
}

LAYER="${1:-}"
if [[ -z "${LAYER}" ]]; then
    usage
fi

# ---------------------------------------------------------------------------
# Layer definitions
# ---------------------------------------------------------------------------

run_lint() {
    echo "--- Layer 0: lint ---"
    cd "${PROJECT_ROOT}"
    cargo fmt --check
    cargo clippy -- -D warnings
    cd "${PROJECT_ROOT}/frontend"
    npm run lint
    npx prettier --check "src/**/*.{ts,tsx,css}" 2>/dev/null || true
    cd "${PROJECT_ROOT}"
    echo "lint: OK"
}

run_unit() {
    echo "--- Layer 1: unit tests ---"
    cd "${PROJECT_ROOT}"
    cargo test --lib
    cd "${PROJECT_ROOT}/frontend"
    npm test
    cd "${PROJECT_ROOT}"
    echo "unit: OK"
}

run_rust_integration_fast() {
    echo "--- Layer 2: rust integration (fast) ---"
    cd "${PROJECT_ROOT}"
    # Run all integration tests except scaling_100k
    cargo test --test cli_drives
    cargo test --test cli_files
    cargo test --test cli_integrity
    cargo test --test cli_virtual_paths
    cargo test --test cli_folders
    cargo test --test cli_sync
    cargo test --test cli_logs
    cargo test --test cli_database
    cargo test --test cli_auth
    cargo test --test cli_status
    cargo test --test packaging
    cargo test --test api_routes
    cargo test --test api_filesystem_browser
    cargo test --test core_mirror
    cargo test --test core_change_detection
    cargo test --test core_scheduler
    echo "rust-integration-fast: OK"
}

run_rust_integration_heavy() {
    echo "--- Layer 3: rust integration (heavy — scaling_100k) ---"
    cd "${PROJECT_ROOT}"
    cargo test --test scaling_100k
    echo "rust-integration-heavy: OK"
}

run_build_artifacts() {
    echo "--- Layer 4: build artifacts ---"
    cd "${PROJECT_ROOT}/frontend"
    npm ci
    npm run build
    cd "${PROJECT_ROOT}"
    if ! cargo deb --version &>/dev/null; then
        cargo install cargo-deb
    fi
    # Compute dev version matching CI logic.
    LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    if [[ -n "${LAST_TAG}" ]]; then
        CARGO_BASE="${LAST_TAG#v}"
    else
        CARGO_BASE=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
    fi
    DEB_UPSTREAM=$(echo "${CARGO_BASE}" | sed 's/-/~/')
    SHORT_SHA=$(git rev-parse --short HEAD)
    DEV_VERSION="${DEB_UPSTREAM}-0ubuntu1~24.04.1+git.${SHORT_SHA}"
    cargo deb --deb-version "${DEV_VERSION}"
    echo "build-artifacts: OK"
    echo "  .deb: $(ls -1 target/debian/bitprotector_*.deb | head -1)"
}

run_qemu_smoke() {
    echo "--- Layer 5: QEMU smoke (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/qemu_test.sh
    if [[ -f "${HOME}/images/plucky-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_test.sh
    else
        echo "WARN: 26.04 image not found — skipping 26.04 smoke (run ./scripts/setup-qemu.sh 26.04 first)"
    fi
    echo "qemu-smoke: OK"
}

run_qemu_failover() {
    echo "--- Layer 6: QEMU failover (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/qemu_failover_test.sh
    if [[ -f "${HOME}/images/plucky-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_failover_test.sh
    else
        echo "WARN: 26.04 image not found — skipping 26.04 failover"
    fi
    echo "qemu-failover: OK"
}

run_qemu_uninstall() {
    echo "--- Layer 7: QEMU uninstall (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/qemu_uninstall_test.sh
    if [[ -f "${HOME}/images/plucky-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_uninstall_test.sh
    else
        echo "WARN: 26.04 image not found — skipping 26.04 uninstall"
    fi
    echo "qemu-uninstall: OK"
}

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------

case "${LAYER}" in
    lint)
        run_lint
        ;;
    fast)
        run_lint
        run_unit
        run_rust_integration_fast
        run_rust_integration_heavy
        ;;
    smoke)
        run_lint
        run_unit
        run_rust_integration_fast
        run_rust_integration_heavy
        run_build_artifacts
        run_qemu_smoke
        ;;
    full)
        run_lint
        run_unit
        run_rust_integration_fast
        run_rust_integration_heavy
        run_build_artifacts
        run_qemu_smoke
        run_qemu_failover
        run_qemu_uninstall
        ;;
    *)
        echo "Unknown layer: ${LAYER}"
        usage
        ;;
esac

echo ""
echo "=== ${LAYER}: all done ==="
