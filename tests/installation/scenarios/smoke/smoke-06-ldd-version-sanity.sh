#!/bin/bash
# tests/installation/scenarios/smoke/smoke-06-ldd-version-sanity.sh
# Scenario #28 — ldd and version sanity: no missing shared libs, PAM linked,
# and upstream binary version matches the package upstream version.
# Bundle: smoke. Assumes: package installed via cloud-init.

smoke_06_ldd_version_sanity() {
    ssh_vm '
set -euo pipefail
# No missing shared libraries
ldd /usr/bin/bitprotector | grep -v "not found" || true
missing=$(ldd /usr/bin/bitprotector | grep "not found" || true)
[ -z "${missing}" ] || { echo "Missing shared libs: ${missing}" >&2; exit 1; }

# PAM library must be linked
ldd /usr/bin/bitprotector | grep -qi "pam" || { echo "PAM not linked" >&2; exit 1; }

# Compare the binary version to the package upstream version.
# Debian revisions/suffixes are allowed to differ.
bin_ver=$(bitprotector --version | grep -oP "[0-9]+\.[0-9]+\.[0-9][-~\w]*" | head -1)
pkg_ver=$(dpkg -s bitprotector | grep "^Version:" | awk "{print \$2}" | head -1)
pkg_upstream="${pkg_ver%%-*}"
pkg_upstream="${pkg_upstream//\~/-}"
[[ "${pkg_upstream}" == "${bin_ver}" ]] || {
    echo "Version mismatch: binary=${bin_ver} package=${pkg_ver} normalized_package=${pkg_upstream}" >&2
    exit 1
}
'
}
