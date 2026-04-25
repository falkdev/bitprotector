#!/bin/bash
# tests/installation/scenarios/smoke/smoke-04-profile-d-installed.sh
# Scenario E3 — profile.d hook installed: verify /etc/profile.d/bitprotector-status.sh exists.
# Bundle: smoke. Assumes: package installed via cloud-init.

smoke_04_profile_d_installed() {
    ssh_vm 'test -f /etc/profile.d/bitprotector-status.sh && echo "hook installed"'
}
