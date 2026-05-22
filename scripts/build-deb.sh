#!/bin/bash
# scripts/build-deb.sh
# Build a Debian package using a Docker container.
#
# Usage:
#   ./scripts/build-deb.sh --ubuntu-version 24.04|26.04 [--deb-version <ver>] [--rebuild]
#
# Options:
#   --ubuntu-version <ver>   Ubuntu version for the builder image (24.04 or 26.04)
#   --deb-version <ver>      Debian package version string (computed from git if omitted)
#   --rebuild                Force rebuild of the Docker image even if it already exists
#
# Output: target/debian/bitprotector_*.deb

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

UBUNTU_VERSION=""
DEB_VERSION=""
REBUILD=0

usage() {
    grep '^# ' "$0" | sed 's/^# //' | tail -n +2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --ubuntu-version) UBUNTU_VERSION="$2"; shift 2 ;;
        --deb-version)    DEB_VERSION="$2"; shift 2 ;;
        --rebuild)        REBUILD=1; shift ;;
        -h|--help)        usage ;;
        *) echo "ERROR: Unknown argument: $1" >&2; usage ;;
    esac
done

if [[ -z "${UBUNTU_VERSION}" ]]; then
    echo "ERROR: --ubuntu-version is required" >&2
    usage
fi

IMAGE="bitprotector-deb-builder:ubuntu-${UBUNTU_VERSION}"

# Build Docker image if not present or --rebuild requested
if [[ "${REBUILD}" -eq 1 ]] || ! docker image inspect "${IMAGE}" &>/dev/null; then
    echo "Building Docker image ${IMAGE}..."
    docker build \
        --build-arg UBUNTU_VERSION="${UBUNTU_VERSION}" \
        -t "${IMAGE}" \
        -f "${PROJECT_ROOT}/docker/Dockerfile.deb-builder" \
        "${PROJECT_ROOT}"
else
    echo "Using existing Docker image ${IMAGE} (use --rebuild to force a rebuild)"
fi

# Compute dev version from git if not provided
if [[ -z "${DEB_VERSION}" ]]; then
    cd "${PROJECT_ROOT}"
    LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
    if [[ -n "${LAST_TAG}" ]]; then
        CARGO_BASE="${LAST_TAG#v}"
    else
        CARGO_BASE=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
    fi
    DEB_UPSTREAM=$(echo "${CARGO_BASE}" | sed 's/-/~/')
    SHORT_SHA=$(git rev-parse --short HEAD)
    DEB_VERSION="${DEB_UPSTREAM}-0ubuntu1~${UBUNTU_VERSION}.1+git.${SHORT_SHA}"
fi

echo "Building .deb (ubuntu-${UBUNTU_VERSION}) version: ${DEB_VERSION}"

# Ensure host Cargo cache dirs exist so Docker does not create them owned by root
mkdir -p "${HOME}/.cargo/registry" "${HOME}/.cargo/git"

docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e HOME=/tmp \
    -e CARGO_HOME=/tmp/.cargo \
    -v "${PROJECT_ROOT}:/workspace" \
    -v "${HOME}/.cargo/registry:/tmp/.cargo/registry" \
    -v "${HOME}/.cargo/git:/tmp/.cargo/git" \
    "${IMAGE}" \
    bash -c "cd /workspace/frontend && npm ci && npm run build && cd /workspace && cargo deb --deb-version '${DEB_VERSION}'"

echo "build-deb: OK"
echo "  .deb: $(ls -1 "${PROJECT_ROOT}/target/debian/bitprotector_"*.deb | head -1)"
