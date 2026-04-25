#!/bin/bash
# tests/installation/scenarios/smoke/smoke-10-tls-cert-rotation.sh
# Scenario #15 — TLS cert rotation: regenerate cert in place; new fingerprint differs.
# Bundle: smoke. Assumes: TLS active at /etc/bitprotector/tls/, service running.

smoke_10_tls_cert_rotation() {
    ssh_vm '
set -euo pipefail
BEFORE=$(echo | openssl s_client -connect localhost:8443 2>/dev/null \
    | openssl x509 -fingerprint -sha256 -noout 2>/dev/null)
[ -n "${BEFORE}" ] || { echo "Could not get initial TLS fingerprint" >&2; exit 1; }

sudo openssl req -x509 -nodes -newkey rsa:2048 -days 365 \
    -subj "/CN=localhost-rotated" \
    -keyout /etc/bitprotector/tls/key.pem \
    -out /etc/bitprotector/tls/cert.pem 2>/dev/null

sudo systemctl restart bitprotector
sleep 5

AFTER=$(echo | openssl s_client -connect localhost:8443 2>/dev/null \
    | openssl x509 -fingerprint -sha256 -noout 2>/dev/null)
[ "${BEFORE}" != "${AFTER}" ] || {
    echo "TLS fingerprint unchanged after cert rotation" >&2
    exit 1
}
'
}
