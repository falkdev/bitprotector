#!/bin/bash
# Scenario: scheduled backup integrity repairs a corrupt peer backup.

app_03_backup_integrity_repairs_peer() {
    set -euo pipefail
    local token
    local backup_a backup_b

    token="$(api_login)"
    backup_a="${APP_SPARE_ROOT}/app02-backup-a"
    backup_b="${APP_SPARE_ROOT}/app02-backup-b"

    if ! ssh_vm "test -f '${backup_a}/bitprotector.db' && test -f '${backup_b}/bitprotector.db'" >/dev/null 2>&1; then
        api_json POST '/database/backups/run' "${token}" >/dev/null
        poll_until "app-03 bootstrap backup files exist" 180 "
test -f '${backup_a}/bitprotector.db' &&
test -f '${backup_b}/bitprotector.db'
"
    else
        # Backup files already exist from a previous scenario (app-02).
        # app-02's backup scheduler may still have an in-flight write to
        # backup_b (stop_database_backup_threads sends a signal but does
        # not join the thread). Wait for last_backup to stabilise across
        # four consecutive readings (≤45 s) before we corrupt the file,
        # so the in-flight write cannot overwrite our intentional corruption.
        local prev_ts="" cur_ts="" stable=0 waited=0
        while [[ ${waited} -lt 45 ]]; do
            cur_ts=$(api_json GET '/database/backups' "${token}" \
                | jq -r '[.[].last_backup // ""] | sort | last // ""' 2>/dev/null || true)
            if [[ "${cur_ts}" == "${prev_ts}" ]]; then
                (( stable++ )) || true
                [[ ${stable} -ge 4 ]] && break
            else
                stable=0
            fi
            prev_ts="${cur_ts}"
            sleep 1
            (( waited++ )) || true
        done
        echo "timing: app-03 backup_quiesce_seconds=${waited}"
    fi

    ssh_vm "printf 'not sqlite\n' | sudo tee '${backup_b}/bitprotector.db' >/dev/null"

    api_json PUT '/database/backups/settings' "${token}" '{"integrity_enabled":true,"integrity_interval_seconds":1}' >/dev/null

    local repair_condition
    repair_condition="$(cat <<CHECK
RESP=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/database/backups')
echo "\$RESP" | jq -e --arg b "${backup_b}" '
  [ .[] | select(.backup_path == \$b) ][0] as \$row
  | \$row != null and \$row.last_integrity_status == "repaired"
' >/dev/null
CHECK
)"
    poll_until "app-03 backup peer repaired" 240 "${repair_condition}"

    verify_sqlite "${backup_b}/bitprotector.db"
    ssh_vm "test -f '${backup_b}/bitprotector.db.blake3'"

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":false,"integrity_enabled":false}' >/dev/null
}
