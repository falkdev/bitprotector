#!/bin/bash
# tests/installation/scenarios/smoke/smoke-05-profile-d-execution.sh
# Scenario #29 — profile.d execution: the login hook runs and follows
# conditional output behavior.
# Bundle: smoke. Assumes: package installed via cloud-init.

smoke_05_profile_d_execution() {
    ssh_vm '
set -euo pipefail
output=$(bash /etc/profile.d/bitprotector-status.sh 2>&1 || true)
# The hook should print status only when the configured DB exists.
if [ -f /var/lib/bitprotector/bitprotector.db ]; then
    [ -n "${output}" ] || { echo "profile.d script produced no output with existing DB" >&2; exit 1; }
else
    [ -z "${output}" ] || { echo "profile.d script should be quiet without DB" >&2; exit 1; }
fi
'
}
