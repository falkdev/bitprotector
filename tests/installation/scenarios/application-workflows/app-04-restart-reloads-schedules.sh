#!/bin/bash
# Scenario: restart persists and reloads scheduler + backup settings.

app_04_restart_reloads_schedules() {
    set -euo pipefail
    local token suffix restart_epoch pair_id
    local primary mirror
    local sync_schedule_id=""
    local integrity_schedule_id=""

    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null
    suffix="$(date +%s)-$RANDOM"
    primary="${APP_PRIMARY_ROOT}/app04-${suffix}"
    mirror="${APP_MIRROR_ROOT}/app04-${suffix}"

    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${mirror}'
sudo mkdir -p '${primary}' '${mirror}'
sudo chown -R testuser:testuser '${primary}' '${mirror}'
printf 'app04-restart\n' > '${primary}/post-restart.txt'
"

    pair_id="$(ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' drives add 'app04-${suffix}' '${primary}' '${mirror}' | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    [[ -n "${pair_id}" ]] || { echo "app-04 failed to create drive pair" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' files track '${pair_id}' 'post-restart.txt' >/dev/null"

    sync_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"sync","interval_seconds":1,"enabled":true}' | jq -r '.id')"
    integrity_schedule_id="$(api_json POST '/scheduler/schedules' "${token}" '{"task_type":"integrity_check","interval_seconds":1,"max_duration_seconds":20,"enabled":true}' | jq -r '.id')"

    [[ -n "${sync_schedule_id}" && "${sync_schedule_id}" != "null" ]] || { echo "app-04 failed to create sync schedule" >&2; exit 1; }
    [[ -n "${integrity_schedule_id}" && "${integrity_schedule_id}" != "null" ]] || { echo "app-04 failed to create integrity schedule" >&2; exit 1; }

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":true,"backup_interval_seconds":1,"integrity_enabled":true,"integrity_interval_seconds":1}' >/dev/null

    restart_epoch="$(date +%s)"
    ssh_vm "sudo systemctl restart bitprotector"

    local i
    for i in $(seq 1 120); do
        if ssh_vm "curl -sk https://localhost:8443/api/v1/health | jq -e '.status == \"ok\"' >/dev/null 2>&1"; then
            break
        fi
        sleep 1
    done
    if ! ssh_vm "curl -sk https://localhost:8443/api/v1/health | jq -e '.status == \"ok\"' >/dev/null 2>&1"; then
        echo "app-04 ERROR: API not healthy after restart" >&2
        ssh_vm "curl -sk https://localhost:8443/api/v1/health" >&2 || true
        ssh_vm "sudo systemctl --no-pager --full status bitprotector" >&2 || true
        exit 1
    fi

    local schedules_json
    schedules_json="$(api_json GET '/scheduler/schedules' "${token}")"
    printf '%s' "${schedules_json}" | jq -e --argjson sid "${sync_schedule_id}" --argjson iid "${integrity_schedule_id}" '
      [.schedules[].id] as $ids | ($ids | index($sid) != null) and ($ids | index($iid) != null)
    ' >/dev/null

    poll_until "app-04 post-restart file mirrored" 240 "test -f '${mirror}/post-restart.txt'"

    local integrity_after_restart
    integrity_after_restart="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/integrity/runs/latest?issues_only=false&page=1&per_page=1')
CREATED=\$(echo "\$RESP" | jq -r '.run.started_at // empty')
STATUS=\$(echo "\$RESP" | jq -r '.run.status // empty')
TRIGGER=\$(echo "\$RESP" | jq -r '.run.trigger // empty')
WORKERS=\$(echo "\$RESP" | jq -r '.run.active_workers // 0')
[[ -n "\$CREATED" ]] || exit 1
[[ \$(date -d "\$CREATED" +%s) -ge ${restart_epoch} ]] || exit 1
[[ "\$TRIGGER" == "scheduler" ]] || exit 1
[[ "\$STATUS" == "completed" || "\$STATUS" == "stopped" ]] || exit 1
[[ "\$STATUS" != "failed" ]] || exit 1
[[ "\$WORKERS" -eq 0 ]]
CHECK
)"
    poll_until "app-04 scheduler integrity resumed after restart" 300 "${integrity_after_restart}"

    local backup_after_restart
    backup_after_restart="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/database/backups/settings')
LAST_BACKUP=\$(echo "\$RESP" | jq -r '.last_backup_run // empty')
LAST_INTEGRITY=\$(echo "\$RESP" | jq -r '.last_integrity_run // empty')
if [[ -n "\$LAST_BACKUP" ]] && [[ \$(date -d "\$LAST_BACKUP" +%s) -ge ${restart_epoch} ]]; then
  exit 0
fi
if [[ -n "\$LAST_INTEGRITY" ]] && [[ \$(date -d "\$LAST_INTEGRITY" +%s) -ge ${restart_epoch} ]]; then
  exit 0
fi
exit 1
CHECK
)"
    poll_until "app-04 backup scheduler resumed after restart" 300 "${backup_after_restart}"

    cleanup_schedules "${token}" "${sync_schedule_id}" "${integrity_schedule_id}"
    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":false,"integrity_enabled":false}' >/dev/null
}
