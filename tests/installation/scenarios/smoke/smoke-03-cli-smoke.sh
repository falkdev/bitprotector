#!/bin/bash
# tests/installation/scenarios/smoke/smoke-03-cli-smoke.sh
# Scenario E2 — CLI smoke: drives list and status subcommands work.
# Bundle: smoke. Assumes: package installed via cloud-init.

smoke_03_cli_smoke() {
    ssh_vm 'bitprotector --db /tmp/smoke.db drives list && bitprotector --db /tmp/smoke.db status'
}
