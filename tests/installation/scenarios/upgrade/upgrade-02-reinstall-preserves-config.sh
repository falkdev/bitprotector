#!/bin/bash
# tests/installation/scenarios/upgrade/upgrade-02-reinstall-preserves-config.sh
# Scenario #25 — Reinstall keeps user config edits.

upgrade_02_reinstall_preserves_config() {
    ssh_vm '
set -euo pipefail
source /etc/bitprotector-upgrade.env

sudo cp /etc/bitprotector/config.toml /etc/bitprotector/config.toml.bak
if ! grep -q "upgrade_marker" /etc/bitprotector/config.toml; then
  echo "# upgrade_marker = true" | sudo tee -a /etc/bitprotector/config.toml >/dev/null
fi

sudo apt-get install --reinstall -y -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" "/mnt/debpkg/${CURRENT_DEB_NAME}"
grep -q "upgrade_marker" /etc/bitprotector/config.toml
'
}
