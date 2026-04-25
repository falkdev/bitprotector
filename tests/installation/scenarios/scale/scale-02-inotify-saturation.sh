#!/bin/bash
# tests/installation/scenarios/scale/scale-02-inotify-saturation.sh
# Scenario #22 — inotify watch capacity stress signal.

scale_02_inotify_saturation() {
    ssh_vm '
set -euo pipefail
DB=/tmp/scale-02.db

rm -f "${DB}"
rm -rf /mnt/scale/watch /mnt/scale-mirror/watch
mkdir -p /mnt/scale/watch /mnt/scale-mirror/watch

max_watch=$(cat /proc/sys/fs/inotify/max_user_watches)
[[ "${max_watch}" -gt 0 ]]

for i in $(seq 1 5000); do
  mkdir -p "/mnt/scale/watch/d-${i}"
  printf "w\n" > "/mnt/scale/watch/d-${i}/f.txt"
done

bitprotector --db "${DB}" drives add inotify /mnt/scale /mnt/scale-mirror --no-validate
bitprotector --db "${DB}" folders add 1 watch
bitprotector --db "${DB}" folders scan 1

systemctl is-active bitprotector | grep -q "^active$"
'
}
