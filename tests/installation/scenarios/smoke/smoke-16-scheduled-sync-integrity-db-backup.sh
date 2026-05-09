#!/bin/bash
# Scenario: tiny scheduled sync + integrity + DB-backup scheduler sweep.
# Bundle: smoke.

smoke_16_scheduled_sync_integrity_db_backup() {
    set -euo pipefail
    local service_db="/var/lib/bitprotector/bitprotector.db"
    local suffix token pair_id
    local primary mirror backup_a backup_b
    local sync_schedule_id=""
    local integrity_schedule_id=""

    wait_for_api "${API_PORT}" 120
    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null
    suffix="$(date +%s)-$RANDOM"
    primary="/tmp/bp-smoke16-primary-${suffix}"
    mirror="/tmp/bp-smoke16-mirror-${suffix}"
    backup_a="/tmp/bp-smoke16-backup-a-${suffix}"
    backup_b="/tmp/bp-smoke16-backup-b-${suffix}"

    smoke16_diag() {
        echo "=== smoke-16 diagnostics: scheduler list ===" >&2
        api_json GET "/scheduler/schedules" "${token:-}" >&2 || true
        echo "=== smoke-16 diagnostics: sync queue ===" >&2
        api_json GET "/sync/queue?page=1&per_page=200" "${token:-}" >&2 || true
        echo "=== smoke-16 diagnostics: latest integrity run ===" >&2
        api_json GET "/integrity/runs/latest?issues_only=false&page=1&per_page=1" "${token:-}" >&2 || true
        echo "=== smoke-16 diagnostics: database backups ===" >&2
        api_json GET "/database/backups" "${token:-}" >&2 || true
        echo "=== smoke-16 diagnostics: database backup settings ===" >&2
        api_json GET "/database/backups/settings" "${token:-}" >&2 || true
        echo "=== smoke-16 diagnostics: service status ===" >&2
        ssh_vm "sudo systemctl --no-pager --full status bitprotector || true" >&2 || true
        echo "=== smoke-16 diagnostics: journal tail ===" >&2
        ssh_vm "sudo journalctl -u bitprotector -n 120 --no-pager || true" >&2 || true
    }
    trap smoke16_diag ERR

    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
sudo mkdir -p '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
sudo chown -R testuser:testuser '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
for i in 1 2 3 4 5; do
  printf 'smoke16-%s\n' \"\$i\" > '${primary}/file-'\$i'.txt'
done
"

    pair_id="$(ssh_vm "sudo bitprotector --db '${service_db}' drives add 'smoke16-${suffix}' '${primary}' '${mirror}' | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    if [[ -z "${pair_id}" ]]; then
        echo "Failed to create smoke16 drive pair" >&2
        exit 1
    fi

    for i in 1 2 3 4 5; do
        ssh_vm "sudo bitprotector --db '${service_db}' files track '${pair_id}' 'file-${i}.txt' >/dev/null"
    done

    local queue_resp pending_total
    queue_resp="$(api_json GET '/sync/queue?status=pending&page=1&per_page=200' "${token}")"
    pending_total="$(printf '%s' "${queue_resp}" | jq -r '.total // 0')"
    if [[ "${pending_total}" -le 0 ]]; then
        echo "Expected pending sync queue rows after tracking files" >&2
        exit 1
    fi

    ssh_vm "sudo bitprotector --db '${service_db}' database add '${backup_a}' --drive-label 'smoke16-a-${suffix}'"
    ssh_vm "sudo bitprotector --db '${service_db}' database add '${backup_b}' --drive-label 'smoke16-b-${suffix}'"

    sync_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"sync","interval_seconds":1,"enabled":true}' | jq -r '.id')"
    integrity_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"integrity_check","interval_seconds":1,"max_duration_seconds":10,"enabled":true}' | jq -r '.id')"

    [[ -n "${sync_schedule_id}" && "${sync_schedule_id}" != "null" ]] || {
        echo "Failed to create sync schedule" >&2
        exit 1
    }
    [[ -n "${integrity_schedule_id}" && "${integrity_schedule_id}" != "null" ]] || {
        echo "Failed to create integrity schedule" >&2
        exit 1
    }

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":true,"backup_interval_seconds":1,"integrity_enabled":true,"integrity_interval_seconds":1}' >/dev/null

    poll_until "smoke-16 mirror files exist" 240 "
test -f '${mirror}/file-1.txt' &&
test -f '${mirror}/file-2.txt' &&
test -f '${mirror}/file-3.txt' &&
test -f '${mirror}/file-4.txt' &&
test -f '${mirror}/file-5.txt'
"

    local integrity_condition
    integrity_condition="RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/integrity/runs/latest?issues_only=false&page=1&per_page=1'); echo \"\$RESP\" | jq -e '.run != null and .run.trigger == \"scheduler\" and .run.status == \"completed\" and ((.run.active_workers // 0) == 0)' >/dev/null"
    poll_until "smoke-16 scheduler integrity completed" 180 "${integrity_condition}"

    poll_until "smoke-16 backup files exist" 120 "
test -f '${backup_a}/bitprotector.db' &&
test -f '${backup_b}/bitprotector.db'
"

    verify_sqlite "${backup_a}/bitprotector.db"
    verify_sqlite "${backup_b}/bitprotector.db"

    local settings_condition
    settings_condition="RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/database/backups/settings'); echo \"\$RESP\" | jq -e '.last_backup_run != null and .last_integrity_run != null' >/dev/null"
    poll_until "smoke-16 backup settings show scheduler runs" 120 "${settings_condition}"

    cleanup_schedules "${token}" "${sync_schedule_id}" "${integrity_schedule_id}"
    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":false,"integrity_enabled":false}' >/dev/null

    trap - ERR
}
