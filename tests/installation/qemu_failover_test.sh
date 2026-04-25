#!/bin/bash
exec "$(dirname "${BASH_SOURCE[0]}")/bundles/failover.sh" "$@"
