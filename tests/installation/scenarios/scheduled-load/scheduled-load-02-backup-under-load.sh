#!/bin/bash
# Scenario: scheduled backup + integrity while scheduler load remains active.

scheduled_load_02_backup_under_load() {
    set -euo pipefail
    local token suffix pair_id
    local primary mirror backup_a backup_b
    local sync_schedule_id=""
    local integrity_schedule_id=""

    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null
    suffix="$(date +%s)-$RANDOM"
    primary="${SLOAD_PRIMARY_ROOT}/load02-${suffix}"
    mirror="${SLOAD_MIRROR_ROOT}/load02-${suffix}"
    backup_a="${SLOAD_SPARE_ROOT}/load02-backup-a"
    backup_b="${SLOAD_SPARE_ROOT}/load02-backup-b"

    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
sudo mkdir -p '${primary}/data' '${mirror}/data' '${backup_a}' '${backup_b}'
sudo chown -R testuser:testuser '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
for d in \$(seq 1 20); do
  mkdir -p '${primary}/data/d-'\"\$d\"
  for f in \$(seq 1 50); do
    printf 'load02-initial-%s-%s\\n' \"\$d\" \"\$f\" > '${primary}/data/d-'\"\$d\"'/f-'\"\$f\"'.txt'
  done
done
"

    pair_id="$(ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' drives add 'scheduled-load-02-${suffix}' '${primary}' '${mirror}' | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    [[ -n "${pair_id}" ]] || { echo "scheduled-load-02 failed to create drive pair" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders add '${pair_id}' data >/dev/null"
    local folder_id
    folder_id="$(ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders list | awk -F'[[:space:]]+' -v pid='${pair_id}' '\$2==pid{print \$1}' | tail -1")"
    [[ -n "${folder_id}" ]] || { echo "scheduled-load-02 failed to resolve folder_id for pair ${pair_id}" >&2; exit 1; }
    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders scan '${folder_id}' >/dev/null"

    sync_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"sync","interval_seconds":1,"enabled":true}' | jq -r '.id')"
    integrity_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"integrity_check","interval_seconds":2,"max_duration_seconds":60,"enabled":true}' | jq -r '.id')"

    [[ -n "${sync_schedule_id}" && "${sync_schedule_id}" != "null" ]] || { echo "scheduled-load-02 failed to create sync schedule" >&2; exit 1; }
    [[ -n "${integrity_schedule_id}" && "${integrity_schedule_id}" != "null" ]] || { echo "scheduled-load-02 failed to create integrity schedule" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' database add '${backup_a}' --drive-label load02-a"
    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' database add '${backup_b}' --drive-label load02-b"

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":true,"backup_interval_seconds":5,"integrity_enabled":true,"integrity_interval_seconds":5}' >/dev/null

    local backup_watch_start
    backup_watch_start="$(date +%s)"

    ssh_vm "
set -euo pipefail
for d in \$(seq 21 40); do
  mkdir -p '${primary}/data/d-'\"\$d\"
  for f in \$(seq 1 50); do
    printf 'load02-wave-%s-%s\\n' \"\$d\" \"\$f\" > '${primary}/data/d-'\"\$d\"'/f-'\"\$f\"'.txt'
  done
done
"
    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders scan '${pair_id}' >/dev/null"

    poll_until "scheduled-load-02 backups observed" 360 "
test -f '${backup_a}/bitprotector.db' &&
test -f '${backup_b}/bitprotector.db'
"

    local first_backup_seconds
    first_backup_seconds=$(( $(date +%s) - backup_watch_start ))
    echo "timing: scheduled-load-02 backup_first_observed_seconds=${first_backup_seconds}"

    verify_sqlite "${backup_a}/bitprotector.db"
    verify_sqlite "${backup_b}/bitprotector.db"

    # Record last_backup BEFORE disabling so the quiesce can detect when
    # the in-flight backup (if any) actually completes.
    local T0
    T0=$(api_json GET '/database/backups' "${token}" \
        | jq -r '[.[].last_backup // ""] | sort | last // ""')

    api_json PUT '/database/backups/settings' "${token}" \
      '{"backup_enabled":false,"integrity_enabled":true,"integrity_interval_seconds":5}' >/dev/null

    # Wait until the backup thread has finished any in-flight write.
    # Strategy:
    #   • If last_backup advances past T0 → an in-flight backup just completed;
    #     then wait for 4 consecutive stable readings before proceeding.
    #   • If last_backup stays at T0 for ≥20 s → the thread was sleeping when
    #     stop was signalled and exited within 100 ms; no further write is coming.
    # Bail out unconditionally after 90 s.
    local cur_ts="" prev_ts="${T0}" stable=0 waited=0 advanced=false
    while [[ ${waited} -lt 90 ]]; do
        cur_ts=$(api_json GET '/database/backups' "${token}" \
            | jq -r '[.[].last_backup // ""] | sort | last // ""')
        if [[ "${cur_ts}" != "${T0}" ]]; then
            advanced=true
        fi
        if [[ "${advanced}" == "true" ]]; then
            # In-flight backup completed; wait for timestamp to stop changing.
            if [[ "${cur_ts}" == "${prev_ts}" ]]; then
                (( stable++ )) || true
                [[ ${stable} -ge 4 ]] && break
            else
                stable=0
            fi
        else
            # last_backup unchanged from T0; thread was likely sleeping and has
            # already exited — safe to proceed after backup_interval + buffer.
            [[ ${waited} -ge 20 ]] && break
        fi
        prev_ts="${cur_ts}"
        sleep 1
        (( waited++ )) || true
    done
    if [[ ${waited} -ge 90 ]]; then
        echo "scheduled-load-02: timed out (90 s) waiting for backup thread to quiesce" >&2
        exit 1
    fi
    echo "timing: scheduled-load-02 backup_quiesce_seconds=${waited} advanced=${advanced}"

    ssh_vm "printf 'not sqlite\\n' | sudo tee '${backup_b}/bitprotector.db' >/dev/null"

    local repair_watch_start
    repair_watch_start="$(date +%s)"

    local repair_condition
    repair_condition="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/database/backups')
echo "\$RESP" | jq -e --arg b "${backup_b}" '
  [ .[] | select(.backup_path == \$b) ][0] as \$row
  | \$row != null and \$row.last_integrity_status == "repaired"
' >/dev/null
CHECK
)"
    poll_until "scheduled-load-02 backup repaired" 360 "${repair_condition}"

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":true,"backup_interval_seconds":5,"integrity_enabled":true,"integrity_interval_seconds":5}' >/dev/null

    local repair_seconds
    repair_seconds=$(( $(date +%s) - repair_watch_start ))
    echo "timing: scheduled-load-02 backup_repair_seconds=${repair_seconds}"

    verify_sqlite "${backup_b}/bitprotector.db"
    ssh_vm "test -f '${backup_b}/bitprotector.db.blake3'"

    cleanup_schedules "${token}" "${sync_schedule_id}" "${integrity_schedule_id}"
    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":false,"integrity_enabled":false}' >/dev/null

    local drain_condition
    drain_condition="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/sync/queue?status=in_progress&page=1&per_page=1')
RESP2=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/sync/queue?status=pending&page=1&per_page=1')
test "\$(echo "\$RESP" | jq -r '.total // 0')" -eq 0 &&
test "\$(echo "\$RESP2" | jq -r '.total // 0')" -eq 0
CHECK
)"
    poll_until "scheduled-load-02 sync queue drained" 120 "${drain_condition}"

    local in_progress_total
    in_progress_total="$(api_json GET '/sync/queue?status=in_progress&page=1&per_page=1' "${token}" | jq -r '.total // 0')"
    [[ "${in_progress_total}" -eq 0 ]] || {
        echo "scheduled-load-02 has stuck in_progress rows: ${in_progress_total}" >&2
        exit 1
    }
}
