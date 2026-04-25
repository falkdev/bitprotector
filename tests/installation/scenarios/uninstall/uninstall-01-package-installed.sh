#!/bin/bash
# tests/installation/scenarios/uninstall/uninstall-01-package-installed.sh
# Scenario E — Package installed: verify binary is in PATH and --version works.
# Bundle: uninstall. Assumes: package installed via cloud-init.

uninstall_01_package_installed() {
    ssh_vm 'which bitprotector && bitprotector --version'
}
