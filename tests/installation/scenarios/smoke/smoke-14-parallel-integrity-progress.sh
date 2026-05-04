#!/bin/bash
# Scenario: SSD pair runs show active_workers > 0 during run, 0 after completion.
# Bundle: drive_media_type.

smoke_14_parallel_integrity_progress() {
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

PAIR_ID=$(bitprotector --db "$DB" drives add "parallel-test" /tmp/bp-ssd-p /tmp/bp-ssd-m \
    --primary-media-type ssd --secondary-media-type ssd \
    | grep -oP "Drive pair #\K[0-9]+" | head -1)

mkdir -p /tmp/bp-ssd-p /tmp/bp-ssd-m
for i in $(seq 1 15); do
    echo "content-$i" > /tmp/bp-ssd-p/file-$i.txt
    echo "content-$i" > /tmp/bp-ssd-m/file-$i.txt
    bitprotector --db "$DB" files track "$PAIR_ID" "file-$i.txt" >/dev/null
done

RUN=$(curl -sk -X POST "$API/integrity/runs" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"drive_id\":$PAIR_ID,\"recover\":false}")
RUN_ID=$(echo "$RUN" | jq -r ".id")
[[ "$RUN_ID" != "null" ]] || { echo "Failed to start run: $RUN" >&2; exit 1; }

SAW_WORKERS=0
for _ in $(seq 1 20); do
    ACTIVE=$(curl -sk -H "Authorization: Bearer $TOKEN" "$API/integrity/runs/active")
    STATUS=$(echo "$ACTIVE" | jq -r ".run.status // \"null\"")
    WORKERS=$(echo "$ACTIVE" | jq -r ".run.active_workers // 0")
    if [[ "$WORKERS" -gt 0 ]]; then
        SAW_WORKERS=1
        break
    fi
    [[ "$STATUS" == "running" ]] || break
    sleep 0.5
done
[[ "$SAW_WORKERS" -eq 1 ]] || { echo "Never observed active_workers > 0" >&2; exit 1; }

for _ in $(seq 1 30); do
    STATUS=$(curl -sk -H "Authorization: Bearer $TOKEN" \
        "$API/integrity/runs/$RUN_ID/results?issues_only=false&page=1&per_page=1" 2>/dev/null \
        | jq -r ".run.status // \"\"")
    [[ "$STATUS" == "completed" || "$STATUS" == "stopped" || "$STATUS" == "failed" ]] && break
    sleep 1
done

FINAL=$(curl -sk -H "Authorization: Bearer $TOKEN" "$API/integrity/runs/latest?issues_only=false&page=1&per_page=1")
FINAL_WORKERS=$(echo "$FINAL" | jq -r ".run.active_workers")
[[ "$FINAL_WORKERS" == "0" ]] || {
    echo "active_workers should be 0 after run, got: $FINAL_WORKERS" >&2; exit 1
}

CLI_OUT=$(bitprotector --db "$DB" integrity check-all --drive-id "$PAIR_ID" 2>&1 || true)
echo "$CLI_OUT" | grep -q "Parallelism used:" || {
    echo "CLI output missing parallelism info. Got: $CLI_OUT" >&2; exit 1
}

echo "smoke-14 passed"
'
}
