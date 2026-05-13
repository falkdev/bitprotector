#!/bin/bash
# tests/installation/scenarios/smoke/smoke-17-adopt-mirror-adoption.sh
# Scenario smoke-17 — adopt_mirror queue action end-to-end.
# Bundle: smoke. Assumes: service running, API reachable, PAM user available.
#
# Verifies that the three adopt_mirror cases all resolve correctly after
# queue processing:
#   (a) Matching standby  — file already on standby with identical content:
#                           no copy is performed, file is marked mirrored.
#   (b) Stale standby     — file on standby with different content:
#                           full copy overwrites the standby.
#   (c) Missing standby   — no file at the standby path:
#                           file is copied from primary to standby.

smoke_17_adopt_mirror_adoption() {
    set -euo pipefail
    local token suffix pair_id
    local primary secondary
    local file_match_id file_stale_id file_new_id

    wait_for_api "${API_PORT}" 120
    token="$(api_login)"
    api_json POST "/sync/resume" "${token}" >/dev/null

    suffix="$(date +%s)-$RANDOM"
    primary="/tmp/bp-smoke17-primary-${suffix}"
    secondary="/tmp/bp-smoke17-secondary-${suffix}"

    # ------------------------------------------------------------------ setup
    ssh_vm "
set -euo pipefail
sudo rm -rf '${primary}' '${secondary}'
sudo mkdir -p '${primary}' '${secondary}'
sudo chown -R testuser:testuser '${primary}' '${secondary}'

# (a) matching: identical bytes on both sides
printf 'identical content smoke17' > '${primary}/match.txt'
printf 'identical content smoke17' > '${secondary}/match.txt'

# (b) stale: primary has content A, secondary has content B
printf 'primary-content-A smoke17' > '${primary}/stale.txt'
printf 'secondary-content-B smoke17' > '${secondary}/stale.txt'

# (c) missing: only on primary
printf 'new content smoke17' > '${primary}/new.txt'
"

    # Record secondary state before queue processing.
    local secondary_match_before secondary_stale_before
    secondary_match_before="$(ssh_vm "sha256sum '${secondary}/match.txt' | awk '{print \$1}'")"
    secondary_stale_before="$(ssh_vm "sha256sum '${secondary}/stale.txt' | awk '{print \$1}'")"

    # ---------------------------------------------------- create drive pair
    pair_id="$(ssh_vm "sudo bitprotector --db /var/lib/bitprotector/bitprotector.db \
        drives add 'smoke17-${suffix}' '${primary}' '${secondary}' \
        | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\\1/p' | head -1")"
    [[ -n "${pair_id}" ]] || { echo "smoke-17 failed to create drive pair" >&2; exit 1; }

    # ------------------------------------------------- track files via API
    file_match_id="$(api_json POST "/files" "${token}" \
        "{\"drive_pair_id\":${pair_id},\"relative_path\":\"match.txt\"}" \
        | jq -r '.id')"
    file_stale_id="$(api_json POST "/files" "${token}" \
        "{\"drive_pair_id\":${pair_id},\"relative_path\":\"stale.txt\"}" \
        | jq -r '.id')"
    file_new_id="$(api_json POST "/files" "${token}" \
        "{\"drive_pair_id\":${pair_id},\"relative_path\":\"new.txt\"}" \
        | jq -r '.id')"

    for var in file_match_id file_stale_id file_new_id; do
        local v="${!var}"
        [[ -n "${v}" && "${v}" != "null" ]] || {
            echo "smoke-17 failed to track ${var}" >&2; exit 1
        }
    done

    # -------------------------------------- confirm action is adopt_mirror
    local queue_resp adopt_count
    queue_resp="$(api_json GET "/sync/queue?status=pending&page=1&per_page=20" "${token}")"
    adopt_count="$(printf '%s' "${queue_resp}" | jq '[.queue[] | select(.action=="adopt_mirror")] | length')"
    if [[ "${adopt_count}" -lt 3 ]]; then
        echo "smoke-17 expected ≥3 adopt_mirror queue items, got ${adopt_count}" >&2
        printf '%s\n' "${queue_resp}" >&2
        exit 1
    fi

    # ------------------------------------------- process the sync queue
    local process_resp processed
    process_resp="$(api_json POST "/sync/process" "${token}")"
    processed="$(printf '%s' "${process_resp}" | jq -r '.processed // 0')"
    if [[ "${processed}" -lt 3 ]]; then
        echo "smoke-17 expected ≥3 items processed, got ${processed}" >&2
        printf '%s\n' "${process_resp}" >&2
        exit 1
    fi

    # ----------------------------------------- verify (a): matching standby
    # The standby file must be unchanged (no copy was needed).
    local secondary_match_after
    secondary_match_after="$(ssh_vm "sha256sum '${secondary}/match.txt' | awk '{print \$1}'")"
    if [[ "${secondary_match_after}" != "${secondary_match_before}" ]]; then
        echo "smoke-17 (a) matching standby was overwritten unexpectedly" >&2
        echo "  before: ${secondary_match_before}" >&2
        echo "  after:  ${secondary_match_after}" >&2
        exit 1
    fi
    local match_mirrored
    match_mirrored="$(api_json GET "/files/${file_match_id}" "${token}" | jq -r '.is_mirrored')"
    [[ "${match_mirrored}" == "true" ]] || {
        echo "smoke-17 (a) match.txt is_mirrored != true (got ${match_mirrored})" >&2; exit 1
    }

    # ----------------------------------------- verify (b): stale standby
    # Primary content must now be on the standby.
    local primary_stale_hash secondary_stale_after
    primary_stale_hash="$(ssh_vm "sha256sum '${primary}/stale.txt' | awk '{print \$1}'")"
    secondary_stale_after="$(ssh_vm "sha256sum '${secondary}/stale.txt' | awk '{print \$1}'")"
    if [[ "${secondary_stale_after}" == "${secondary_stale_before}" ]]; then
        echo "smoke-17 (b) stale standby was NOT updated (content still matches old secondary)" >&2
        exit 1
    fi
    if [[ "${secondary_stale_after}" != "${primary_stale_hash}" ]]; then
        echo "smoke-17 (b) stale standby content does not match primary after copy" >&2
        echo "  primary:          ${primary_stale_hash}" >&2
        echo "  secondary after:  ${secondary_stale_after}" >&2
        exit 1
    fi
    local stale_mirrored
    stale_mirrored="$(api_json GET "/files/${file_stale_id}" "${token}" | jq -r '.is_mirrored')"
    [[ "${stale_mirrored}" == "true" ]] || {
        echo "smoke-17 (b) stale.txt is_mirrored != true (got ${stale_mirrored})" >&2; exit 1
    }

    # ----------------------------------------- verify (c): missing standby
    ssh_vm "test -f '${secondary}/new.txt'" || {
        echo "smoke-17 (c) new.txt was not created on standby" >&2; exit 1
    }
    local primary_new_hash secondary_new_hash
    primary_new_hash="$(ssh_vm "sha256sum '${primary}/new.txt' | awk '{print \$1}'")"
    secondary_new_hash="$(ssh_vm "sha256sum '${secondary}/new.txt' | awk '{print \$1}'")"
    if [[ "${secondary_new_hash}" != "${primary_new_hash}" ]]; then
        echo "smoke-17 (c) new.txt content mismatch after copy" >&2
        echo "  primary:   ${primary_new_hash}" >&2
        echo "  secondary: ${secondary_new_hash}" >&2
        exit 1
    fi
    local new_mirrored
    new_mirrored="$(api_json GET "/files/${file_new_id}" "${token}" | jq -r '.is_mirrored')"
    [[ "${new_mirrored}" == "true" ]] || {
        echo "smoke-17 (c) new.txt is_mirrored != true (got ${new_mirrored})" >&2; exit 1
    }
}
