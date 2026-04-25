#!/bin/bash
# tests/installation/scenarios/uninstall/uninstall-03-purge.sh
# Scenario E — Purge package and verify all package-owned paths are removed.
# Bundle: uninstall. Assumes: uninstall-02 completed (package-owned DB and backups exist).

uninstall_03_purge() {
    ssh_vm 'sudo DEBIAN_FRONTEND=noninteractive apt-get purge -y bitprotector'

    ssh_vm '
set -euo pipefail

if dpkg -s bitprotector >/dev/null 2>&1; then
    echo "bitprotector package is still installed." >&2
    exit 1
fi

if [ -e /usr/bin/bitprotector ]; then
    echo "/usr/bin/bitprotector is still present." >&2
    exit 1
fi

if [ -e /var/lib/bitprotector ]; then
    echo "/var/lib/bitprotector is still present." >&2
    exit 1
fi

if [ -e /var/log/bitprotector ]; then
    echo "/var/log/bitprotector is still present." >&2
    exit 1
fi

if [ -e /etc/bitprotector ]; then
    echo "/etc/bitprotector is still present." >&2
    exit 1
fi
'
}
