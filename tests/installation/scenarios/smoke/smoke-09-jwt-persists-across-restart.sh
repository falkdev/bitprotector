#!/bin/bash
# tests/installation/scenarios/smoke/smoke-09-jwt-persists-across-restart.sh
# Scenario #14 — JWT persists across restart: a token issued before restart is still valid after.
# Bundle: smoke. Assumes: TLS active, service running, PAM user testauth/hunter2 exists.

smoke_09_jwt_persists_across_restart() {
    ssh_vm '
set -euo pipefail
TOKEN=$(curl -sk -X POST https://localhost:8443/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}" | jq -r .token)
[ -n "${TOKEN}" ] && [ "${TOKEN}" != "null" ] || { echo "Failed to get token" >&2; exit 1; }

sudo systemctl restart bitprotector
sleep 5

status=$(curl -sk -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer ${TOKEN}" \
    https://localhost:8443/api/v1/drives)
[ "${status}" = "200" ] || { echo "Token rejected after restart, got ${status}" >&2; exit 1; }
'
}
