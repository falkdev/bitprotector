#!/bin/bash
# tests/installation/lib/cloud-init-db-disk.sh
# Shared cloud-init snippet generator for the dedicated BitProtector DB disk.
#
# The bpdb disk (virtio serial=bpdb) is attached to every QEMU test bundle as a
# 32 GB qcow2 image and mounted inside the guest at /mnt/bitprotector-db.
# Scenario DB files live under /mnt/bitprotector-db/db.
#
# Usage (inside an unquoted CLOUDINIT heredoc):
#
#   write_files:
#   $(cloudinit_bpdb_write_file)
#   runcmd:
#     - /usr/local/bin/bitprotector-db-storage.sh
#     ...

# cloudinit_bpdb_write_file
# Outputs the cloud-init write_files entry that installs
# /usr/local/bin/bitprotector-db-storage.sh on the guest.
#
# Note: the script chowns /mnt/bitprotector-db to 'testuser', which is the
# standard unprivileged test account provisioned in every QEMU bundle's
# cloud-init users block.  All bundles must include this user definition.
cloudinit_bpdb_write_file() {
    cat <<'EOF'
  - path: /usr/local/bin/bitprotector-db-storage.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -euo pipefail
      dev=/dev/disk/by-id/virtio-bpdb
      for _ in $(seq 1 30); do
        [[ -b "${dev}" ]] && break
        sleep 1
      done
      if [[ ! -b "${dev}" ]]; then
        echo "ERROR: expected block device not found: ${dev}" >&2
        echo "Contents of /dev/disk/by-id:" >&2
        ls -l /dev/disk/by-id >&2 || true
        exit 1
      fi
      mkdir -p /mnt/bitprotector-db
      if ! blkid "${dev}" >/dev/null 2>&1; then
        mkfs.ext4 -F "${dev}"
      fi
      uuid=$(blkid -s UUID -o value "${dev}")
      grep -q "${uuid}" /etc/fstab || echo "UUID=${uuid} /mnt/bitprotector-db ext4 defaults,nofail 0 2" >> /etc/fstab
      mount -a
      mkdir -p /mnt/bitprotector-db/db
      chown -R testuser:testuser /mnt/bitprotector-db
EOF
}
