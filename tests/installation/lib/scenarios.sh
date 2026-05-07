#!/bin/bash
# tests/installation/lib/scenarios.sh
# Shared scenario primitives sourced by bundle scripts.
# Requires: SSH_PORT, WORKDIR to be set by the calling bundle.
# Also requires lib/qemu-helpers.sh to be sourced first.
# Do not execute directly.

# Run a command on the guest over SSH.
ssh_vm() {
    timeout "${SSH_VM_TIMEOUT:-30}" \
        ssh -o StrictHostKeyChecking=no \
            -o BatchMode=yes \
            -o ConnectionAttempts=1 \
            -o ConnectTimeout=5 \
            -o ServerAliveInterval=2 \
            -o ServerAliveCountMax=2 \
            -p "${SSH_PORT}" testuser@localhost "$@"
}

# Register a drive pair and print the pair id to stdout.
# Usage: PAIR_ID=$(make_pair NAME PRIMARY_ROOT MIRROR_ROOT)
make_pair() {
    local name="$1"
    local primary="$2"
    local mirror="$3"
    ssh_vm "bitprotector --db \"\${BP_DB:-/mnt/bitprotector-db/db/bp-test.db}\" drives add \"${name}\" \"${primary}\" \"${mirror}\"" \
        | sed -nE 's/.*[Dd]rive pair #([0-9]+).*/\1/p' | head -1
}

# Create a file of a given size filled with random data on the guest.
# Usage: seed_file GUEST_PATH SIZE_BYTES
seed_file() {
    local path="$1"
    local size="$2"
    ssh_vm "dd if=/dev/urandom bs=1 count=${size} of='${path}' 2>/dev/null"
}

# Flip one byte at OFFSET in PATH on the guest (bit-rot simulation).
corrupt_byte() {
    local path="$1"
    local offset="$2"
    ssh_vm "python3 -c \"
import sys
with open('${path}', 'r+b') as f:
    f.seek(${offset})
    b = f.read(1)
    f.seek(${offset})
    f.write(bytes([b[0] ^ 0xFF]))\""
}

# Delete the DB file on the guest so the next command re-initialises it.
reset_db() {
    local db_path="$1"
    ssh_vm "rm -f '${db_path}'"
}

# Poll 'sync queue list' until the queue is drained or TIMEOUT_SECS is reached.
wait_for_sync_queue_empty() {
    local db_path="$1"
    local timeout="$2"
    local i count
    for i in $(seq 1 "${timeout}"); do
        count=$(ssh_vm "bitprotector --db '${db_path}' sync queue list 2>/dev/null \
            | grep -c 'pending\|in_progress' || true")
        if [[ "${count}" -eq 0 ]]; then
            return 0
        fi
        sleep 1
    done
    log ERROR "sync queue not empty after ${timeout}s"
    return 1
}

# Wait for a reboot cycle to complete and SSH to become available again.
# Usage: wait_for_reboot_and_ssh [TIMEOUT_SECS]
wait_for_reboot_and_ssh() {
    local timeout="${1:-180}"
    local saw_disconnect=0
    local i

    for i in $(seq 1 "${timeout}"); do
        if timeout 6 ssh -o StrictHostKeyChecking=no \
            -o BatchMode=yes \
            -o ConnectionAttempts=1 \
            -o ConnectTimeout=2 \
            -o ServerAliveInterval=2 \
            -o ServerAliveCountMax=1 \
            -p "${SSH_PORT}" testuser@localhost "echo ok" >/dev/null 2>&1; then
            if [[ "${saw_disconnect}" -eq 1 ]]; then
                return 0
            fi
        else
            saw_disconnect=1
        fi
        sleep 1
    done

    log ERROR "VM did not come back after reboot within ${timeout}s"
    return 1
}

# Fail if journalctl shows error-level messages from bitprotector since SINCE_ISO8601.
# Usage: assert_no_journal_errors "2026-01-01 00:00:00"
assert_no_journal_errors() {
    local since="$1"
    local errors filtered
    errors=$(ssh_vm "journalctl -p err -u bitprotector --since '${since}' --no-pager -q 2>/dev/null || true")
    filtered="${errors}"

    if [[ -s "${WORKDIR:-}/expected-journal-patterns.txt" && -n "${filtered}" ]]; then
        while IFS= read -r pattern; do
            [[ -z "${pattern}" ]] && continue
            filtered=$(printf '%s\n' "${filtered}" | grep -Fv -- "${pattern}" || true)
        done < "${WORKDIR}/expected-journal-patterns.txt"
    fi

    if [[ -n "${filtered}" ]]; then
        log ERROR "Unexpected journal errors from bitprotector since ${since}:"
        echo "${filtered}" >&2
        return 1
    fi
}

# Expect a specific pattern in the journal since SINCE and suppress it from assert_no_journal_errors.
# Usage: expect_journal_error SINCE PATTERN
expect_journal_error() {
    local since="$1"
    local pattern="$2"
    ssh_vm "journalctl -p err -u bitprotector --since '${since}' --no-pager -q 2>/dev/null \
        | grep -q '${pattern}' || { echo 'Expected journal error not found: ${pattern}' >&2; exit 1; }"
    mkdir -p "${WORKDIR:-.}"
    printf '%s\n' "${pattern}" >> "${WORKDIR}/expected-journal-patterns.txt"
}

# Final-scenario wrapper: assert no error-level journal entries since $BUNDLE_START_TIME.
# Bundles record BUNDLE_START_TIME="$(date -Iseconds)" before running any scenarios.
# Resilience scenarios that intentionally trigger errors must call expect_journal_error first.
journal_error_scraper() {
    assert_no_journal_errors "${BUNDLE_START_TIME:?BUNDLE_START_TIME not set by bundle}"
}

# Run a named scenario function, printing PASS/FAIL and aborting the bundle on failure.
# Usage: run_scenario "scenario-name" function_name
run_scenario() {
    local name="$1"
    local fn="$2"
    log GROUP "Scenario: ${name}"
    if "${fn}"; then
        echo "PASS: ${name}"
        log ENDGROUP
    else
        local exit_code=$?
        log ENDGROUP
        echo "FAIL: ${name}"
        log ERROR "Scenario ${name} failed with exit code ${exit_code}"
        if [[ -f "${WORKDIR:-}/serial.log" ]]; then
            log ERROR "Last serial output:"
            tail -20 "${WORKDIR}/serial.log" >&2 || true
        fi
        exit "${exit_code}"
    fi
}
