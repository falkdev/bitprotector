#!/bin/bash
# tests/installation/scenarios/failover/failover-02-emergency-qmp.sh
# Scenario E — Emergency failover after hot-removing the active replacement-primary disk via QMP.
# Bundle: failover. Assumes: failover-01 completed (DB at /tmp/failover.db, active role = replacement-primary).
# Requires: QMP_SOCKET exported by the bundle.

failover_02_emergency_qmp() {
    log INFO "Hot-removing replacement-primary disk through QMP..."
    qmp_device_del "dev-replacement-primary"
    sleep 5

    ssh_vm '
set -euo pipefail
DB=/tmp/failover.db
VIRTUAL_FILE=/tmp/bitprotector-virtual/docs/report.txt

# Existing open file handles may fail after sudden device loss.
# We assert the supported contract: a follow-up operation triggers failover,
# then new opens through the virtual path work from the surviving mirror.
bitprotector --db "${DB}" integrity check 1
bitprotector --db "${DB}" drives show 1 | grep -q "Active Role:     secondary"
readlink -f "${VIRTUAL_FILE}" | grep -q "^/mnt/mirror/"
cat "${VIRTUAL_FILE}" | grep -q "after planned failover"
'
}
