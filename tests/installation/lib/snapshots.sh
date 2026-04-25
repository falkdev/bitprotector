#!/bin/bash
# tests/installation/lib/snapshots.sh
# QMP savevm/loadvm and device hot-plug helpers for bundles that need full VM-state snapshots.
# Requires: QMP_SOCKET to be set by the calling bundle.
# Also requires lib/qemu-helpers.sh to be sourced first.
# Do not execute directly.

_qmp_send() {
    if [[ ! -S "${QMP_SOCKET:-}" ]]; then
        log ERROR "QMP socket not available at ${QMP_SOCKET:-<unset>}"
        return 1
    fi
    printf '%s\n' "$@" | socat - UNIX-CONNECT:"${QMP_SOCKET}" >/dev/null
}

qmp_savevm() {
    local name="$1"
    _qmp_send \
        '{ "execute": "qmp_capabilities" }' \
        "{ \"execute\": \"savevm\", \"arguments\": { \"name\": \"${name}\" } }"
}

qmp_loadvm() {
    local name="$1"
    _qmp_send \
        '{ "execute": "qmp_capabilities" }' \
        "{ \"execute\": \"loadvm\", \"arguments\": { \"name\": \"${name}\" } }"
}

qmp_delvm() {
    local name="$1"
    _qmp_send \
        '{ "execute": "qmp_capabilities" }' \
        "{ \"execute\": \"delvm\", \"arguments\": { \"name\": \"${name}\" } }"
}

qmp_device_add() {
    local json="$1"
    _qmp_send \
        '{ "execute": "qmp_capabilities" }' \
        "{ \"execute\": \"device_add\", \"arguments\": ${json} }"
}

qmp_device_del() {
    local device_id="$1"
    _qmp_send \
        '{ "execute": "qmp_capabilities" }' \
        "{ \"execute\": \"device_del\", \"arguments\": { \"id\": \"${device_id}\" } }"
}
