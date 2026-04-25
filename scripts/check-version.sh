#!/usr/bin/env bash
# scripts/check-version.sh
# Verify that all version references are consistent with Cargo.toml.
#
# Usage:
#   ./scripts/check-version.sh                             # format check only (no artifacts)
#   ./scripts/check-version.sh --deb <path>                # also check .deb filename + metadata
#   ./scripts/check-version.sh --binary <path>             # also check binary --version output
#   ./scripts/check-version.sh --ubuntu-version 26.04      # target Ubuntu 26.04 (default: 24.04)
#   ./scripts/check-version.sh --deb <path> --binary <path> --ubuntu-version <ver>
#
# Expected Debian version: <upstream>-0ubuntu1~<ubuntu_version>.1
#   where <upstream> is Cargo.toml version with the first '-' replaced by '~'
#   e.g. Cargo.toml "1.0.0-alpha1" + ubuntu 24.04 → Debian "1.0.0~alpha1-0ubuntu1~24.04.1"
#        Cargo.toml "1.0.0"        + ubuntu 26.04 → Debian "1.0.0-0ubuntu1~26.04.1"
#
# Exit codes: 0 = all checks passed, 1 = one or more mismatches

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# --- argument parsing ---
DEB_PATH=""
BINARY_PATH=""
UBUNTU_VERSION="24.04"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --deb)            DEB_PATH="$2";        shift 2 ;;
        --binary)         BINARY_PATH="$2";     shift 2 ;;
        --ubuntu-version) UBUNTU_VERSION="$2";  shift 2 ;;
        *)
            echo "Unknown argument: $1" >&2
            echo "Usage: $0 [--deb <path>] [--binary <path>] [--ubuntu-version <24.04|26.04>]" >&2
            exit 1
            ;;
    esac
done

if ! [[ "${UBUNTU_VERSION}" =~ ^[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: --ubuntu-version must be in the form MAJOR.MINOR (e.g. 24.04 or 26.04)" >&2
    exit 1
fi

ERRORS=0

# --- Step 1: extract and validate Cargo.toml version ---
CARGO_VERSION=$(grep -m1 '^version = ' "${PROJECT_ROOT}/Cargo.toml" \
    | sed 's/version = "\(.*\)"/\1/')

if [[ -z "${CARGO_VERSION}" ]]; then
    echo "ERROR: could not extract version from Cargo.toml" >&2
    exit 1
fi
if ! [[ "${CARGO_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "ERROR: Cargo.toml version '${CARGO_VERSION}' is not MAJOR.MINOR.PATCH[-prerelease]" >&2
    exit 1
fi
echo "Cargo.toml version : ${CARGO_VERSION}"

# --- Step 2: derive expected Debian version ---
# Convert first '-' to '~' for Debian pre-release ordering, then append Ubuntu revision.
# "1.0.0-alpha1" + 24.04 → "1.0.0~alpha1-0ubuntu1~24.04.1"
# "1.0.0"        + 26.04 → "1.0.0-0ubuntu1~26.04.1"
DEB_UPSTREAM=$(echo "${CARGO_VERSION}" | sed 's/-/~/')
EXPECTED_DEB_VERSION="${DEB_UPSTREAM}-0ubuntu1~${UBUNTU_VERSION}.1"
EXPECTED_DEB_FILENAME="bitprotector_${EXPECTED_DEB_VERSION}_amd64.deb"
echo "Ubuntu version     : ${UBUNTU_VERSION}"
echo "Expected Debian ver: ${EXPECTED_DEB_VERSION}"

# --- Step 3: auto-detect artifacts if not supplied ---
if [[ -z "${DEB_PATH}" ]]; then
    CANDIDATE="${PROJECT_ROOT}/target/debian/${EXPECTED_DEB_FILENAME}"
    if [[ -f "${CANDIDATE}" ]]; then
        DEB_PATH="${CANDIDATE}"
        echo "Auto-detected .deb : ${DEB_PATH}"
    fi
fi
if [[ -z "${BINARY_PATH}" ]]; then
    CANDIDATE="${PROJECT_ROOT}/target/release/bitprotector"
    if [[ -f "${CANDIDATE}" ]]; then
        BINARY_PATH="${CANDIDATE}"
        echo "Auto-detected binary: ${BINARY_PATH}"
    fi
fi

# --- Step 4: .deb filename check ---
if [[ -n "${DEB_PATH}" ]]; then
    DEB_BASENAME=$(basename "${DEB_PATH}")
    if [[ "${DEB_BASENAME}" == "${EXPECTED_DEB_FILENAME}" ]]; then
        echo "PASS .deb filename  : ${DEB_BASENAME}"
    else
        echo "FAIL .deb filename  : got '${DEB_BASENAME}', expected '${EXPECTED_DEB_FILENAME}'" >&2
        ERRORS=$((ERRORS + 1))
    fi

    # --- Step 5: dpkg-deb --info metadata check ---
    if command -v dpkg-deb &>/dev/null; then
        DEB_META_VERSION=$(dpkg-deb --info "${DEB_PATH}" \
            | grep '^ Version:' | awk '{print $2}')
        if [[ "${DEB_META_VERSION}" == "${EXPECTED_DEB_VERSION}" ]]; then
            echo "PASS dpkg-deb Version: ${DEB_META_VERSION}"
        else
            echo "FAIL dpkg-deb Version: got '${DEB_META_VERSION}', expected '${EXPECTED_DEB_VERSION}'" >&2
            ERRORS=$((ERRORS + 1))
        fi
    else
        echo "SKIP dpkg-deb check : dpkg-deb not found"
    fi
else
    echo "SKIP .deb checks    : no .deb path provided or auto-detected"
fi

# --- Step 6: binary --version check ---
# The binary always reports the Cargo.toml version (not the Debian version).
if [[ -n "${BINARY_PATH}" ]]; then
    if [[ ! -x "${BINARY_PATH}" ]]; then
        echo "FAIL binary         : not executable: ${BINARY_PATH}" >&2
        ERRORS=$((ERRORS + 1))
    else
        CLI_OUTPUT=$("${BINARY_PATH}" --version 2>&1 || true)
        CLI_VERSION=$(echo "${CLI_OUTPUT}" | awk '{print $2}')
        if [[ "${CLI_VERSION}" == "${CARGO_VERSION}" ]]; then
            echo "PASS binary version : ${CLI_OUTPUT}"
        else
            echo "FAIL binary version : got '${CLI_VERSION}', expected '${CARGO_VERSION}'" >&2
            ERRORS=$((ERRORS + 1))
        fi
    fi
else
    echo "SKIP binary check   : no binary path provided or auto-detected"
fi

# --- Step 7: summary ---
echo ""
if [[ ${ERRORS} -eq 0 ]]; then
    echo "All version checks passed (Cargo.toml: ${CARGO_VERSION}, Debian: ${EXPECTED_DEB_VERSION})"
else
    echo "ERROR: ${ERRORS} version check(s) failed" >&2
    exit 1
fi
