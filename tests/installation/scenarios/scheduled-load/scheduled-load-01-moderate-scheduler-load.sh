#!/bin/bash
# Scenario: moderate scheduler load with 10k+ tracked files and timing capture.

scheduled_load_01_moderate_scheduler_load() {
    set -euo pipefail
    local token suffix pair_id
    local primary mirror
    local sync_schedule_id=""
    local integrity_schedule_id=""
    local files_per_dir=100
    local dir_count=120
    local generated_files=$((files_per_dir * dir_count))

    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null
    suffix="$(date +%s)-$RANDOM"
    primary="${SLOAD_PRIMARY_ROOT}/load01-${suffix}"
    mirror="${SLOAD_MIRROR_ROOT}/load01-${suffix}"

    ssh_vm '
set -euo pipefail
echo "=== guest diagnostics ==="
grep -E "^(NAME|VERSION)=" /etc/os-release || true
uname -r
nproc
free -h
df -h / /mnt/scale-primary /mnt/bitprotector-db
findmnt /mnt/scale-primary || true
findmnt /mnt/scale-mirror || true
findmnt /mnt/bitprotector-db || true
echo "=== end diagnostics ==="
'

    local gen_start gen_end gen_seconds
    gen_start="$(date +%s)"
    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${mirror}'
sudo mkdir -p '${primary}/data' '${mirror}/data'
sudo chown -R testuser:testuser '${primary}' '${mirror}'
for d in \$(seq 1 ${dir_count}); do
  mkdir -p '${primary}/data/d-'\"\$d\"
  for f in \$(seq 1 ${files_per_dir}); do
    printf 'load01-%s-%s\\n' \"\$d\" \"\$f\" > '${primary}/data/d-'\"\$d\"'/f-'\"\$f\"'.txt'
  done
done
"
    gen_end="$(date +%s)"
    gen_seconds=$((gen_end - gen_start))
    echo "timing: scheduled-load-01 generation_seconds=${gen_seconds} files=${generated_files}"

    pair_id="$(ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' drives add 'scheduled-load-01-${suffix}' '${primary}' '${mirror}' | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    [[ -n "${pair_id}" ]] || { echo "scheduled-load-01 failed to create drive pair" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders add '${pair_id}' data >/dev/null"
    local folder_id
    folder_id="$(ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders list | awk -F'[[:space:]]+' -v pid='${pair_id}' '\$2==pid{print \$1}' | tail -1")"
    [[ -n "${folder_id}" ]] || { echo "scheduled-load-01 failed to resolve folder_id for pair ${pair_id}" >&2; exit 1; }

    local scan_start scan_end scan_seconds
    scan_start="$(date +%s)"
    ssh_vm "sudo bitprotector --db '${SLOAD_SERVICE_DB}' folders scan '${folder_id}' >/dev/null"
    scan_end="$(date +%s)"
    scan_seconds=$((scan_end - scan_start))
    echo "timing: scheduled-load-01 scan_seconds=${scan_seconds}"

    local initial_pending
    initial_pending="$(api_json GET '/sync/queue?status=pending&page=1&per_page=200' "${token}" | jq -r '.total // 0')"
    if [[ "${initial_pending}" -le 0 ]]; then
        echo "scheduled-load-01 expected pending queue rows after scan" >&2
        exit 1
    fi

    sync_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"sync","interval_seconds":1,"enabled":true}' | jq -r '.id')"
    integrity_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"integrity_check","interval_seconds":2,"max_duration_seconds":60,"enabled":true}' | jq -r '.id')"

    [[ -n "${sync_schedule_id}" && "${sync_schedule_id}" != "null" ]] || { echo "scheduled-load-01 failed to create sync schedule" >&2; exit 1; }
    [[ -n "${integrity_schedule_id}" && "${integrity_schedule_id}" != "null" ]] || { echo "scheduled-load-01 failed to create integrity schedule" >&2; exit 1; }

    local sync_start pending completed
    local drain_started_seconds=-1
    local pending_zero_seconds=-1
    sync_start="$(date +%s)"

    local i in_progress
    for i in $(seq 1 900); do
        pending="$(api_json GET '/sync/queue?status=pending&page=1&per_page=200' "${token}" | jq -r '.total // 0')"
        completed="$(api_json GET '/sync/queue?status=completed&page=1&per_page=200' "${token}" | jq -r '.total // 0')"
        in_progress="$(api_json GET '/sync/queue?status=in_progress&page=1&per_page=200' "${token}" | jq -r '.total // 0')"

        if [[ "${drain_started_seconds}" -lt 0 && "${pending}" -lt "${initial_pending}" ]]; then
            drain_started_seconds=$(( $(date +%s) - sync_start ))
        fi
        if [[ "${pending}" -eq 0 && "${in_progress}" -eq 0 ]]; then
            pending_zero_seconds=$(( $(date +%s) - sync_start ))
            break
        fi
        sleep 1
    done

    [[ "${drain_started_seconds}" -ge 0 ]] || {
        echo "scheduled-load-01 queue never started draining (initial_pending=${initial_pending})" >&2
        exit 1
    }
    [[ "${pending_zero_seconds}" -ge 0 ]] || {
        echo "scheduled-load-01 queue did not reach zero pending rows within timeout" >&2
        exit 1
    }

    echo "timing: scheduled-load-01 queue_drain_started_seconds=${drain_started_seconds}"
    echo "timing: scheduled-load-01 pending_zero_seconds=${pending_zero_seconds}"

    local in_progress_total completed_total
    in_progress_total="$(api_json GET '/sync/queue?status=in_progress&page=1&per_page=200' "${token}" | jq -r '.total // 0')"
    completed_total="$(api_json GET '/sync/queue?status=completed&page=1&per_page=200' "${token}" | jq -r '.total // 0')"

    [[ "${in_progress_total}" -eq 0 ]] || {
        echo "scheduled-load-01 has stuck in_progress rows: ${in_progress_total}" >&2
        exit 1
    }
    [[ "${completed_total}" -gt 0 ]] || {
        echo "scheduled-load-01 expected completed queue rows > 0" >&2
        exit 1
    }

    local integrity_condition
    integrity_condition="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/integrity/runs/latest?issues_only=false&page=1&per_page=1')
echo "\$RESP" | jq -e '.run != null and .run.trigger == "scheduler" and (.run.status == "completed" or .run.status == "stopped" or .run.status == "failed")' >/dev/null
CHECK
)"
    poll_until "scheduled-load-01 scheduler integrity produced a terminal run" 420 "${integrity_condition}"

    local latest
    latest="$(api_json GET '/integrity/runs/latest?issues_only=false&page=1&per_page=1' "${token}")"
    local latest_status latest_workers processed_files
    latest_status="$(printf '%s' "${latest}" | jq -r '.run.status // empty')"
    latest_workers="$(printf '%s' "${latest}" | jq -r '.run.active_workers // 0')"
    processed_files="$(printf '%s' "${latest}" | jq -r '.run.processed_files // 0')"

    [[ "${latest_status}" != "failed" ]] || {
        echo "scheduled-load-01 latest integrity run failed" >&2
        echo "${latest}" >&2
        exit 1
    }
    [[ "${latest_workers}" -eq 0 ]] || {
        echo "scheduled-load-01 latest integrity run has active_workers=${latest_workers}" >&2
        exit 1
    }

    echo "timing: scheduled-load-01 integrity_processed_files=${processed_files}"

    cleanup_schedules "${token}" "${sync_schedule_id}" "${integrity_schedule_id}"
}
