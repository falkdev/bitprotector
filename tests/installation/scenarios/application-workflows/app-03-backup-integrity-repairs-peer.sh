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
