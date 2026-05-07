#!/bin/bash
# Scenario: create SSD+HDD pair via CLI, verify via API, update via API.
# Bundle: drive_media_type.

smoke_14_drive_media_type() {
    ssh_vm '
set -euo pipefail
DB="${BP_DB:-/var/lib/bitprotector/bitprotector.db}"
API="https://localhost:8443/api/v1"
TOKEN="${BP_TOKEN:-}"
if [[ -z "$TOKEN" ]]; then
    for _ in $(seq 1 30); do
        TOKEN=$(curl -sk -X POST "$API/auth/login" \
            -H "Content-Type: application/json" \
            -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}" 2>/dev/null \
            | jq -r ".token // empty" 2>/dev/null || true)
        [[ -n "$TOKEN" && "$TOKEN" != "null" ]] && break
        sleep 1
    done
fi
if [[ -z "$TOKEN" || "$TOKEN" == "null" ]]; then
    RAW=$(curl -skS -w "\nHTTP:%{http_code}\n" -X POST "$API/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}" || true)
    echo "Failed to obtain API token. Raw login response:" >&2
    echo "$RAW" >&2
    echo "bitprotector service status:" >&2
    sudo systemctl --no-pager --full status bitprotector >&2 || true
    echo "Recent bitprotector journal:" >&2
    sudo journalctl -u bitprotector -n 80 --no-pager >&2 || true
    exit 1
fi

mkdir -p /tmp/bp-primary /tmp/bp-mirror
PAIR_ID=$(sudo bitprotector --db "$DB" drives add "media-type-test" /tmp/bp-primary /tmp/bp-mirror \
    --primary-media-type ssd --secondary-media-type hdd \
    | sed -nE "s/.*[Dd]rive pair #([0-9]+).*/\\1/p" | head -1)

[[ -n "$PAIR_ID" ]] || { echo "Failed to create drive pair" >&2; exit 1; }

RESP=$(curl -sk -H "Authorization: Bearer $TOKEN" "$API/drives/$PAIR_ID")
echo "$RESP" | jq -e ".primary_media_type == \"ssd\"" >/dev/null || {
    echo "Expected primary_media_type=ssd, got: $RESP" >&2; exit 1
}
echo "$RESP" | jq -e ".secondary_media_type == \"hdd\"" >/dev/null || {
    echo "Expected secondary_media_type=hdd, got: $RESP" >&2; exit 1
}

curl -sk -X PUT "$API/drives/$PAIR_ID" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"primary_media_type\":\"hdd\"}" | jq -e ".primary_media_type == \"hdd\"" >/dev/null || {
    echo "Update to hdd failed" >&2; exit 1
}

echo "smoke-14 passed"
'
}
