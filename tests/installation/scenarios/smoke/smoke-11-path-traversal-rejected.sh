#!/bin/bash
# tests/installation/scenarios/smoke/smoke-11-path-traversal-rejected.sh
# Scenario #16 — Path traversal rejected: attempts to escape drive root return 400/403.
# Bundle: smoke. Assumes: TLS active, service running, PAM user testauth/hunter2 exists.

smoke_11_path_traversal_rejected() {
    ssh_vm '
set -euo pipefail
TOKEN=$(curl -sk -X POST https://localhost:8443/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}" | jq -r .token)
[ -n "${TOKEN}" ] && [ "${TOKEN}" != "null" ] || { echo "Failed to get token" >&2; exit 1; }

# Create a drive pair for the path checks
mkdir -p /tmp/ptrav-primary /tmp/ptrav-mirror
resp=$(curl -sk -X POST https://localhost:8443/api/v1/drives \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"ptrav-test\",\"primary_path\":\"/tmp/ptrav-primary\",\"secondary_path\":\"/tmp/ptrav-mirror\",\"skip_validation\":true}")
PAIR_ID=$(echo "${resp}" | jq -r .id)
[ -n "${PAIR_ID}" ] && [ "${PAIR_ID}" != "null" ] || { echo "Failed to create drive pair" >&2; exit 1; }

check_rejected() {
    local path="$1"
    local code
    code=$(curl -sk -o /dev/null -w "%{http_code}" \
        -X POST "https://localhost:8443/api/v1/files" \
        -H "Authorization: Bearer ${TOKEN}" \
        -H "Content-Type: application/json" \
        -d "{\"drive_pair_id\":${PAIR_ID},\"relative_path\":\"${path}\"}")
    if [[ "${code}" == "200" || "${code}" == "201" ]]; then
        echo "FAIL: expected rejection for path ${path} but got ${code}" >&2
        return 1
    fi
}

check_rejected "../../etc/passwd"
check_rejected "../../../etc/shadow"
'
}
