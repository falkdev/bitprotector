#!/bin/bash
# Scenario: scheduled sync + scheduled integrity over a moderate nested dataset.

app_01_scheduled_sync_integrity_moderate_dataset() {
    set -euo pipefail
    local token suffix pair_id
    local primary mirror
    local sync_schedule_id=""
    local integrity_schedule_id=""

    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null
    suffix="$(date +%s)-$RANDOM"
    primary="${APP_PRIMARY_ROOT}/app01-${suffix}"
    mirror="${APP_MIRROR_ROOT}/app01-${suffix}"

    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${mirror}'
sudo mkdir -p '${primary}/docs' '${mirror}/docs'
sudo chown -R testuser:testuser '${primary}' '${mirror}'
for d in \$(seq 1 20); do
  mkdir -p '${primary}/docs/d-'\"\$d\"
  for f in \$(seq 1 30); do
    p='${primary}/docs/d-'\"\$d\"'/file-'\"\$f\"'.bin'
    if (( \$f % 25 == 0 )); then
      dd if=/dev/urandom of="\$p" bs=1M count=2 status=none
    elif (( \$f % 5 == 0 )); then
      dd if=/dev/urandom of="\$p" bs=64K count=1 status=none
    else
      printf 'app01-%s-%s\n' "\$d" "\$f" > "\$p"
    fi
  done
done
"

    pair_id="$(ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' drives add 'app01-${suffix}' '${primary}' '${mirror}' | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    [[ -n "${pair_id}" ]] || { echo "app-01 failed to create drive pair" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' folders add '${pair_id}' docs >/dev/null"
    local folder_id
    folder_id="$(ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' folders list | awk -F'[[:space:]]+' -v pid='${pair_id}' '\$2==pid{print \$1}' | tail -1")"
    [[ -n "${folder_id}" ]] || { echo "app-01 failed to resolve folder_id for pair ${pair_id}" >&2; exit 1; }
    ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' folders scan '${folder_id}' >/dev/null"

    sync_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"sync","interval_seconds":1,"enabled":true}' | jq -r '.id')"
    integrity_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"integrity_check","interval_seconds":2,"max_duration_seconds":30,"enabled":true}' | jq -r '.id')"

    [[ -n "${sync_schedule_id}" && "${sync_schedule_id}" != "null" ]] || { echo "app-01 failed to create sync schedule" >&2; exit 1; }
    [[ -n "${integrity_schedule_id}" && "${integrity_schedule_id}" != "null" ]] || { echo "app-01 failed to create integrity schedule" >&2; exit 1; }

    local queue_drain_condition
    queue_drain_condition="$(cat <<CHECK
PENDING=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/sync/queue?status=pending&page=1&per_page=200' | jq -r '.total // 0' || echo 0)
IN_PROGRESS=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/sync/queue?status=in_progress&page=1&per_page=200' | jq -r '.total // 0' || echo 0)
(( PENDING == 0 && IN_PROGRESS == 0 ))
CHECK
)"
    poll_until "app-01 sync queue drained" 360 "${queue_drain_condition}"

    ssh_vm "
set -euo pipefail
for rel in \
  docs/d-1/file-1.bin \
  docs/d-4/file-10.bin \
  docs/d-12/file-25.bin \
  docs/d-20/file-30.bin
  do
  src='${primary}/'\"\$rel\"
  dst='${mirror}/'\"\$rel\"
  test -f "\$src"
  test -f "\$dst"
  src_hash=\$(sha256sum "\$src" | awk '{print \$1}')
  dst_hash=\$(sha256sum "\$dst" | awk '{print \$1}')
  [[ "\$src_hash" == "\$dst_hash" ]]
done
"

    local integrity_condition
    integrity_condition="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/integrity/runs/latest?issues_only=false&page=1&per_page=1')
echo "\$RESP" | jq -e '.run != null and .run.trigger == "scheduler" and (.run.status == "completed" or .run.status == "stopped") and (.run.status != "failed") and ((.run.active_workers // 0) == 0)' >/dev/null
CHECK
)"
    poll_until "app-01 scheduler integrity completed or stopped" 240 "${integrity_condition}"

    local latest_integrity
    latest_integrity="$(api_json GET '/integrity/runs/latest?issues_only=false&page=1&per_page=1' "${token}")"
    if [[ "$(printf '%s' "${latest_integrity}" | jq -r '.run.status // empty')" == "failed" ]]; then
        echo "app-01 latest integrity run status is failed" >&2
        echo "${latest_integrity}" >&2
        exit 1
    fi

    cleanup_schedules "${token}" "${sync_schedule_id}" "${integrity_schedule_id}"
}
