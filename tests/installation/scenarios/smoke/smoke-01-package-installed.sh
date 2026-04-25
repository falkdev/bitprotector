#!/bin/bash
# tests/installation/scenarios/smoke/smoke-01-package-installed.sh
# Scenario E1 — Package installed: verify binary is in PATH and --version works.
# Bundle: smoke. Assumes: package installed via cloud-init.

smoke_01_package_installed() {
    ssh_vm 'which bitprotector && bitprotector --version'
}
