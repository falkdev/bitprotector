#!/bin/bash
# Scenario: create SSD+HDD pair via CLI, verify via API, update via API.
# Bundle: drive_media_type.

smoke_13_drive_media_type() {
    ssh_vm '
set -euo pipefail
DB="${BP_DB:-/mnt/bitprotector-db/db/bp-test.db}"
API="https://localhost:8443/api/v1"
TOKEN="${BP_TOKEN:-}"
if [[ -z "$TOKEN" ]]; then
    TOKEN=$(curl -sk -X POST "$API/auth/login" \
        -H "Content-Type: application/json" \
        -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}" | jq -r .token)
fi
[[ -n "$TOKEN" && "$TOKEN" != "null" ]] || { echo "Failed to obtain API token" >&2; exit 1; }

PAIR_ID=$(bitprotector --db "$DB" drives add "media-type-test" /tmp/bp-primary /tmp/bp-mirror \
    --primary-media-type ssd --secondary-media-type hdd \
    | grep -oP "Drive pair #\K[0-9]+" | head -1)

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

echo "smoke-13 passed"
'
}
