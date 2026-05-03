# BitProtector

[![CI](https://github.com/falkdev/bitprotector/actions/workflows/ci.yml/badge.svg)](https://github.com/falkdev/bitprotector/actions/workflows/ci.yml) [![Nightly](https://github.com/falkdev/bitprotector/actions/workflows/nightly.yml/badge.svg)](https://github.com/falkdev/bitprotector/actions/workflows/nightly.yml)

## Distributed File Mirror and Integrity Protection System

Monitors files across redundant storage, detects bit-decay and silent corruption using BLAKE3 checksums, automatically recovers from mirror copies, and supports live drive failover plus replacement rebuilds. Operates as a background daemon with both a CLI tool and a HTTPS REST API.

---

## Table of Contents

- [How It Works](#how-it-works)
- [Prerequisites](#prerequisites)
- [Build](#build)
- [Install](#install)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Running the Service](#running-the-service)
- [Documentation](#documentation)

---

## How It Works

1. You register **drive pairs** — a primary path and a secondary (mirror) path.
2. You **track files** (individually or by folder). BitProtector computes a BLAKE3 checksum and queues mirror work in `sync_queue` by default.
   - In the web UI, tracked file/folder forms now open a real server-side path picker backed by the host filesystem.
   - Those web selections may start as absolute host paths in the UI, but BitProtector still stores tracked file/folder paths relative to the selected drive pair's active root.
3. **Integrity runs** re-hash tracked files against the stored baseline using an async run model:
   - Full runs are persisted (`integrity_runs` + `integrity_run_results`) and can be polled while processing.
   - The Integrity page auto-loads the latest persisted run on open and only renders files that need attention.
   - Large datasets stay responsive because the backend processes/persists results incrementally and the UI fetches paged issue rows.
   - Mirror corrupted → restore from primary.
   - Primary corrupted → restore from mirror.
   - Both corrupted → flag for user action.
4. If the active drive fails, BitProtector can fail over to the surviving side and retarget virtual paths to it.
5. When a replacement drive is mounted, BitProtector can queue a rebuild back onto that slot and return the pair to a fully mirrored state.
6. A **virtual path** layer exposes tracked files and folders at exact absolute filesystem paths by creating symlinks directly at those virtual locations.

---

## Prerequisites

| Requirement | Notes |
| --- | --- |
| Rust stable toolchain | Install via [rustup](https://rustup.rs) |
| Node.js 20.19+ | Required to build, lint, and test the React frontend |
| `libpam0g-dev` | PAM headers for system authentication — `sudo apt install libpam0g-dev` |
| TLS certificate and key | Required for the HTTPS API server — see [docs/CONFIGURATION.md](docs/CONFIGURATION.md#generating-a-self-signed-certificate) |
| `cargo-deb` (optional) | Required only to build the `.deb` package — `cargo install cargo-deb` |

---

## Build

```bash
# Frontend build (required for the web UI and Debian package)
cd frontend
npm ci
npm run build
cd ..

# Debug build
cargo build

# Release build
cargo build --release

# Build Debian package (Ubuntu 24 target)
cargo deb
```

The release binary is written to `target/release/bitprotector`.
The `.deb` package is written to `target/debian/`.
The production frontend bundle is written to `frontend/dist/`.

---

## Install

### From the Debian package (recommended for production)

```bash
cd frontend
npm ci
npm run build
cd ..
cargo deb
sudo dpkg -i target/debian/bitprotector_*.deb
```

The package installs:

- Binary to `/usr/bin/bitprotector`
- Default config to `/etc/bitprotector/config.toml`
- systemd unit to `/lib/systemd/system/bitprotector.service`
- SSH login status hook to `/etc/profile.d/bitprotector-status.sh`
- Frontend assets to `/var/lib/bitprotector/frontend`

### From source (development)

```bash
cd frontend
npm ci
npm run build
cd ..
cargo build --release
sudo install -m 755 target/release/bitprotector /usr/local/bin/
sudo mkdir -p /etc/bitprotector /var/lib/bitprotector/frontend /var/log/bitprotector
sudo cp packaging/config.toml /etc/bitprotector/config.toml
sudo cp -r frontend/dist/* /var/lib/bitprotector/frontend/
```

---

## Quick Start

```bash
# 1. Register a drive pair
bitprotector drives add mybackup /mnt/primary /mnt/mirror

# 2. Track a file (queues mirror work by default)
bitprotector files track <drive-pair-id> documents/report.pdf

# 3. Track a folder, then scan it to discover files (scan queues mirror work)
bitprotector folders add <drive-pair-id> documents
bitprotector folders scan <folder-id>

# 4. Process queued mirror/sync work
bitprotector sync process

# 5. Optional: mirror immediately without waiting for queue processing
bitprotector files mirror <file-id>

# 6. Optional: mirror all unmirrored tracked files under one folder now
bitprotector folders mirror <folder-id>

# 7. Run an integrity check (persisted run summary/results)
bitprotector integrity check all

# 8. Show overall status
bitprotector status

# 9. Assign a virtual path (creates a symlink exactly at this absolute path)
bitprotector virtual-paths set <file-id> /docs/report.pdf

# 10. Planned primary replacement workflow
bitprotector drives replace mark <drive-pair-id> --role primary
# (optional) cancel if you change your mind:
bitprotector drives replace cancel <drive-pair-id> --role primary
bitprotector drives replace confirm <drive-pair-id> --role primary
bitprotector drives replace assign <drive-pair-id> --role primary /mnt/new-primary
bitprotector sync process
```

During failover, virtual path symlinks automatically follow the pair's current `active_role`. Planned replacements use `mark` then `confirm` so you can quiesce external I/O before switching over.

For a full list of commands and flags, run:

```bash
bitprotector --help
bitprotector <subcommand> --help
```

---

## Configuration

The service reads `/etc/bitprotector/config.toml` at startup. CLI flags override config file values, which override hardcoded defaults.

```toml
# /etc/bitprotector/config.toml
[server]
host       = "0.0.0.0"
port       = 8443
jwt_secret = "replace-with-a-random-64-char-string"  # MUST be changed
tls_cert   = "/etc/bitprotector/tls/cert.pem"
tls_key    = "/etc/bitprotector/tls/key.pem"

[database]
path = "/var/lib/bitprotector/bitprotector.db"
```

CLI flags can override any value at runtime, and `--config` selects a different config file:

```bash
bitprotector --config /etc/bitprotector/config.toml serve --port 9443
```

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for a full reference of every option.

---

## Running the Service

```bash
# Enable and start the systemd service
sudo systemctl enable --now bitprotector

# Check service status
sudo systemctl status bitprotector

# View logs
sudo journalctl -u bitprotector -f
```

The web UI is available at `https://localhost:8443/` once the service is running.
The REST API remains available at `https://localhost:8443/api/v1`.

In the web UI:

- `/files` is the unified **Tracking Workspace** for both tracked files and tracked folders (`/folders` redirects to `/files`)
- tracked file and tracked folder forms use a filesystem browser dialog powered by the server
- drive pair and replacement-drive forms can also fill directory paths from the same browser
- tracked file/folder submissions are validated against the selected drive pair's active root before they are stored
- tracking and folder scans queue mirror work by default; use explicit mirror actions or sync processing for immediate copies
- Integrity page starts/stops async runs, shows a running progress banner, and only lists issue rows (`needs_attention=true`)
- the latest run is loaded from DB automatically on page open; results are fetched in pages for responsiveness
- the Integrity page intro shows `Last integrity check` as a date/time timestamp (instead of a run ID)
- user/logout controls are pinned to the bottom of the left sidebar (top header chrome removed across authenticated pages)

For a CLI-only workflow without the daemon, pass `--db <path>` to use a custom database file:

```bash
bitprotector --db /tmp/test.db drives list
```

---

## Documentation

| Document | Description |
| --- | --- |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, module breakdown, and database schema |
| [docs/API.md](docs/API.md) | Full REST API reference with request/response examples |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Every configuration key explained |
| [docs/TESTING.md](docs/TESTING.md) | How to run tests, test categories, QEMU suites, and the dedicated guest DB-disk layout used by installation scenarios |
| [docs/CI.md](docs/CI.md) | CI pipeline layers, local debugging with `act`, QEMU guest storage model, and nightly failure reproduction |

---

## AI assistance

Parts of this project were developed with assistance from AI tools
(GitHub Copilot, Claude). All AI-generated code has been reviewed,
tested, and modified by the maintainer before publication.

---

## License

MIT — see [LICENSE](LICENSE).
