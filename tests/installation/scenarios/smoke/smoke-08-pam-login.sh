#!/bin/bash
# tests/installation/scenarios/smoke/smoke-08-pam-login.sh
# Scenario #13 — PAM login: correct credentials return a token; wrong password returns 401.
# Bundle: smoke. Assumes: TLS active, service running, PAM user testauth/hunter2 exists.

smoke_08_pam_login() {
    ssh_vm '
set -euo pipefail
resp=$(curl -sk -X POST https://localhost:8443/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}")
echo "${resp}" | jq -e ".token" >/dev/null || {
    echo "Login failed, response: ${resp}" >&2
    exit 1
}

bad=$(curl -sk -o /dev/null -w "%{http_code}" -X POST https://localhost:8443/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"testauth\",\"password\":\"WRONG\"}")
[[ "${bad}" == "401" ]] || { echo "bad creds should return 401 but got ${bad}" >&2; exit 1; }
'
}
