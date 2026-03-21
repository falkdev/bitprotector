#!/bin/sh
# /etc/profile.d/bitprotector-status.sh
# Installed by the bitprotector .deb package.
# Displays a brief BitProtector system health summary at SSH login.

DB_PATH="${BITPROTECTOR_DB:-/var/lib/bitprotector/bitprotector.db}"

if [ -f "$DB_PATH" ] && command -v bitprotector >/dev/null 2>&1; then
    bitprotector --db "$DB_PATH" status 2>/dev/null || true
fi
