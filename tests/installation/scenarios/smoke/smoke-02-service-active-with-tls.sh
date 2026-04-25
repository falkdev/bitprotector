#!/bin/bash
# tests/installation/scenarios/smoke/smoke-02-service-active-with-tls.sh
# Scenario #— — Service active with TLS: systemd reports active and the login endpoint responds.
# Bundle: smoke. Assumes: TLS cert at /etc/bitprotector/tls/, service configured and running.

smoke_02_service_active_with_tls() {
    ssh_vm '
set -euo pipefail
systemctl is-active bitprotector

# Login endpoint should respond (expect 400 for empty body, not connection-refused)
code=$(curl -sk -o /dev/null -w "%{http_code}" \
    -X POST https://localhost:8443/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d "{}")
[ "${code}" != "000" ] || { echo "API not reachable (connection refused)" >&2; exit 1; }
'
}
