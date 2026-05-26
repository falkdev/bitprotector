#!/bin/bash
# Scenario: scheduled DB backups remain valid during metadata churn.

app_02_database_backup_during_churn() {
    set -euo pipefail
    local token suffix pair_id start_epoch
    local primary mirror backup_a backup_b

    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null
    suffix="$(date +%s)-$RANDOM"
    primary="${APP_PRIMARY_ROOT}/app02-${suffix}"
    mirror="${APP_MIRROR_ROOT}/app02-${suffix}"
    backup_a="${APP_SPARE_ROOT}/app02-backup-a"
    backup_b="${APP_SPARE_ROOT}/app02-backup-b"
    start_epoch="$(date +%s)"

    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
sudo mkdir -p '${primary}/data' '${mirror}/data' '${backup_a}' '${backup_b}'
sudo chown -R testuser:testuser '${primary}' '${mirror}' '${backup_a}' '${backup_b}'
"

    pair_id="$(ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' drives add 'app02-${suffix}' '${primary}' '${mirror}' | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    [[ -n "${pair_id}" ]] || { echo "app-02 failed to create drive pair" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' folders add '${pair_id}' data >/dev/null"
    local folder_id
    folder_id="$(ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' folders list | awk -F'[[:space:]]+' -v pid='${pair_id}' '\$2==pid{print \$1}' | tail -1")"
    [[ -n "${folder_id}" ]] || { echo "app-02 failed to resolve folder_id for pair ${pair_id}" >&2; exit 1; }

    ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' database add '${backup_a}' --drive-label app02-a"
    ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' database add '${backup_b}' --drive-label app02-b"

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":true,"backup_interval_seconds":1,"integrity_enabled":false}' >/dev/null

    local wave
    for wave in 1 2 3; do
        ssh_vm "
set -euo pipefail
mkdir -p '${primary}/data/wave-${wave}'
for i in \$(seq 1 100); do
  printf 'app02-wave-%s-file-%s\\n' '${wave}' \"\$i\" > '${primary}/data/wave-${wave}/file-'\$i'.txt'
done
"
        ssh_vm "sudo bitprotector --db '${APP_SERVICE_DB}' folders scan '${folder_id}' >/dev/null"

        poll_until "app-02 wave ${wave}: backup files exist" 180 "
test -f '${backup_a}/bitprotector.db' &&
test -f '${backup_b}/bitprotector.db'
"

        verify_sqlite "${backup_a}/bitprotector.db"
        verify_sqlite "${backup_b}/bitprotector.db"
    done

    local settings_condition
    settings_condition="$(cat <<CHECK
SETTINGS=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/database/backups/settings')
LAST=\$(echo "\$SETTINGS" | jq -r '.last_backup_run // empty')
[[ -n "\$LAST" ]] || exit 1
[[ \$(date -d "\$LAST" +%s) -ge ${start_epoch} ]] || exit 1
BACKUPS=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/database/backups')
printf '%s' "\$BACKUPS" | jq -e --arg a '${backup_a}' --arg b '${backup_b}' '
  [ .[] | select(.backup_path == \$a or .backup_path == \$b) ] as \$rows
  | (\$rows | length) == 2
  and ([ \$rows[] | .last_error == null ] | all)
' >/dev/null
CHECK
)"
    poll_until "app-02 backup settings updated with no errors" 180 "${settings_condition}"

    local backups_json
    backups_json="$(api_json GET '/database/backups' "${token}")"
    printf '%s' "${backups_json}" | jq -e --arg a "${backup_a}" --arg b "${backup_b}" '
      [ .[] | select(.backup_path == $a or .backup_path == $b) ] as $rows
      | ($rows | length) == 2
      and ([ $rows[] | .last_error == null ] | all)
    ' >/dev/null

    poll_until "app-02 no stale tmp files in backup dirs" 15 "
TMP_COUNT=\$(find '${backup_a}' '${backup_b}' -maxdepth 1 -name '*.tmp' | wc -l)
[[ \"\${TMP_COUNT}\" -eq 0 ]]
"

    api_json POST '/sync/process' "${token}" >/dev/null

    local queue_drain_condition
    queue_drain_condition="$(cat <<CHECK
PENDING=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/sync/queue?status=pending&page=1&per_page=200' | jq -r '.total // 0' || echo 0)
IN_PROGRESS=\$(curl -sk -H 'Authorization: Bearer ${token}' 'https://localhost:8443/api/v1/sync/queue?status=in_progress&page=1&per_page=200' | jq -r '.total // 0' || echo 0)
(( PENDING == 0 && IN_PROGRESS == 0 ))
CHECK
)"
    poll_until "app-02 sync queue drained" 360 "${queue_drain_condition}"

    api_json PUT '/database/backups/settings' "${token}" '{"backup_enabled":false,"integrity_enabled":false}' >/dev/null
}
