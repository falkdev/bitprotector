#!/bin/bash
# scripts/run-tests.sh
# Native (no Docker) test runner — mirrors the CI layer structure.
# Requires: Rust toolchain, Node 24+, and (for smoke/full) QEMU prerequisites.
#
# Usage:
#   ./scripts/run-tests.sh <layer>
#
# Layers:
#   lint   — cargo fmt + clippy, npm lint + prettier (Layer 0)
#   fast   — lint + unit tests + integration tests excluding scaling_100k (Layers 0-3)
#   smoke  — fast + build .deb + QEMU smoke on ubuntu-24.04 and ubuntu-26.04 (Layers 0-5)
#   full   — smoke + application-workflows + failover + uninstall + resilience + upgrade + degraded-boot + drive-media-type (Layers 0-12)
#   e2e    — boot dedicated QEMU guest + run Playwright E2E suite (Layer 13, requires Playwright browsers)

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
    npx prettier --check "src/**/*.{ts,tsx,css}"
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
    cargo test --test api_drives
    cargo test --test api_files
    cargo test --test api_folders
    cargo test --test api_virtual_paths
    cargo test --test api_integrity
    cargo test --test api_scheduler
    cargo test --test api_sync
    cargo test --test api_logs
    cargo test --test api_database
    cargo test --test api_routes
    cargo test --test api_filesystem_browser
    cargo test --test core_mirror
    cargo test --test core_change_detection
    cargo test --test core_scheduler
    cargo test --test core_checksum_strategy
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
    cd "${PROJECT_ROOT}"
    scripts/build-deb.sh --ubuntu-version 24.04
    scripts/build-deb.sh --ubuntu-version 26.04
    echo "build-artifacts: OK"
    echo "  .debs:"
    ls -1 target/debian/ubuntu-*/bitprotector_*.deb
}

run_qemu_smoke() {
    echo "--- Layer 5: QEMU smoke (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/qemu_test.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_test.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found — skipping 26.04 smoke (run ./scripts/setup-qemu.sh 26.04 first)"
    fi
    echo "qemu-smoke: OK"
}

run_qemu_application_workflows() {
    echo "--- Layer 6: QEMU application workflows (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/bundles/application_workflows.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/bundles/application_workflows.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found - skipping 26.04 application workflows"
    fi
    echo "qemu-application-workflows: OK"
}

run_qemu_failover() {
    echo "--- Layer 7: QEMU failover (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/qemu_failover_test.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_failover_test.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found — skipping 26.04 failover"
    fi
    echo "qemu-failover: OK"
}

run_qemu_uninstall() {
    echo "--- Layer 8: QEMU uninstall (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/qemu_uninstall_test.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_uninstall_test.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found — skipping 26.04 uninstall"
    fi
    echo "qemu-uninstall: OK"
}

run_qemu_resilience() {
    echo "--- Layer 9: QEMU resilience (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/bundles/resilience.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/bundles/resilience.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found — skipping 26.04 resilience"
    fi
    echo "qemu-resilience: OK"
}

resolve_previous_release_deb() {
    local deb_suffix="$1"
    local cache_dir baseline_tag baseline_deb

    cache_dir="${PROJECT_ROOT}/target/debian/previous-release"

    if [[ -n "${BASELINE_DEB:-}" ]]; then
        baseline_deb=$(ls -1 ${BASELINE_DEB} 2>/dev/null | head -1 || true)
        if [[ -n "${baseline_deb}" ]]; then
            printf '%s\n' "${baseline_deb}"
            return 0
        fi
        echo "WARN: BASELINE_DEB is set but no file matches: ${BASELINE_DEB}" >&2
    fi

    if ! command -v gh >/dev/null 2>&1; then
        echo "WARN: gh CLI not found; cannot auto-resolve previous release baseline" >&2
        return 1
    fi

    baseline_tag=$(gh release list --exclude-drafts --limit 1 --json tagName --jq '.[0].tagName' 2>/dev/null || true)
    if [[ -z "${baseline_tag}" || "${baseline_tag}" == "null" ]]; then
        echo "WARN: unable to determine previous tagged release baseline" >&2
        return 1
    fi

    mkdir -p "${cache_dir}"
    if ! gh release download "${baseline_tag}" --pattern "*${deb_suffix}*.deb" --dir "${cache_dir}" >/dev/null 2>&1; then
        echo "WARN: failed to download baseline .deb for ${deb_suffix} from ${baseline_tag}" >&2
        return 1
    fi

    baseline_deb=$(ls -1 "${cache_dir}"/*"${deb_suffix}"*.deb 2>/dev/null | head -1 || true)
    if [[ -z "${baseline_deb}" ]]; then
        echo "WARN: no downloaded baseline .deb matched suffix ${deb_suffix}" >&2
        return 1
    fi

    echo "Using previous release baseline ${baseline_tag}: ${baseline_deb}" >&2
    printf '%s\n' "${baseline_deb}"
}

run_qemu_upgrade() {
    echo "--- Layer 10: QEMU upgrade (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"

    local baseline24 baseline26
    baseline24=$(resolve_previous_release_deb "24.04.1" || true)
    if [[ -z "${baseline24}" ]]; then
        echo "WARN: could not resolve 24.04 baseline .deb; skipping 24.04 upgrade"
    else
        GUEST_IMAGE=ubuntu-24.04 BASELINE_DEB="${baseline24}" ./tests/installation/bundles/upgrade.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    fi

    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        baseline26=$(resolve_previous_release_deb "26.04.1" || true)
        if [[ -z "${baseline26}" ]]; then
            echo "WARN: could not resolve 26.04 baseline .deb; skipping 26.04 upgrade"
        else
            GUEST_IMAGE=ubuntu-26.04 BASELINE_DEB="${baseline26}" ./tests/installation/bundles/upgrade.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
        fi
    else
        echo "WARN: 26.04 image not found — skipping 26.04 upgrade"
    fi

    if [[ -z "${baseline24}" && -z "${baseline26}" ]]; then
        echo "WARN: no upgrade baselines resolved; upgrade bundle was skipped"
        return 0
    fi

    echo "qemu-upgrade: OK"
}

run_qemu_degraded_boot() {
    echo "--- Layer 11: QEMU degraded-boot (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/bundles/degraded_boot.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/bundles/degraded_boot.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found — skipping 26.04 degraded-boot"
    fi
    echo "qemu-degraded-boot: OK"
}

run_qemu_drive_media_type() {
    echo "--- Layer 12: QEMU drive media type (ubuntu-24.04 + ubuntu-26.04) ---"
    cd "${PROJECT_ROOT}"
    GUEST_IMAGE=ubuntu-24.04 ./tests/installation/bundles/drive_media_type.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"
    if [[ -f "${HOME}/images/resolute-server-cloudimg-amd64.img" ]]; then
        GUEST_IMAGE=ubuntu-26.04 ./tests/installation/bundles/drive_media_type.sh "${PROJECT_ROOT}/target/debian/ubuntu-26.04/bitprotector_*.deb"
    else
        echo "WARN: 26.04 image not found — skipping 26.04 drive-media-type"
    fi
    echo "qemu-drive-media-type: OK"
}

run_e2e() {
    echo "--- Layer 13: Playwright E2E (ubuntu-24.04 only) ---"
    cd "${PROJECT_ROOT}"

    echo "Ensuring Playwright Chromium browser is installed"
    (cd frontend && npx playwright install --with-deps chromium)

    local e2e_pid_file="${RUNNER_TEMP:-/tmp}/e2e-qemu.pid"
    local e2e_cleanup_done=0

    cleanup_e2e() {
        if [[ "${e2e_cleanup_done}" == "1" ]]; then return; fi
        e2e_cleanup_done=1
        if [[ -f "${e2e_pid_file}" ]]; then
            kill "$(cat "${e2e_pid_file}")" 2>/dev/null || true
            echo "QEMU e2e guest stopped"
        fi
    }
    trap cleanup_e2e EXIT

    SSH_PORT=2280 API_PORT=18480 ./tests/installation/e2e-guest.sh "${PROJECT_ROOT}/target/debian/ubuntu-24.04/bitprotector_*.deb"

    QEMU_SSH_PORT=2280 QEMU_API_PORT=18480 CI=true \
        npm --prefix frontend run test:e2e:qemu

    cleanup_e2e
    trap - EXIT
    echo "e2e: OK"
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
        run_qemu_application_workflows
        run_qemu_failover
        run_qemu_uninstall
        run_qemu_resilience
        run_qemu_upgrade
        run_qemu_degraded_boot
        run_qemu_drive_media_type
        run_e2e
        ;;
    e2e)
        run_e2e
        ;;
    *)
        echo "Unknown layer: ${LAYER}"
        usage
        ;;
esac

echo ""
echo "=== ${LAYER}: all done ==="
