# BitProtector

[![CI](https://github.com/falkdev/bitprotector/actions/workflows/ci.yml/badge.svg)](https://github.com/falkdev/bitprotector/actions/workflows/ci.yml) [![Nightly](https://github.com/falkdev/bitprotector/actions/workflows/nightly.yml/badge.svg)](https://github.com/falkdev/bitprotector/actions/workflows/nightly.yml)

## Distributed File Mirror and Integrity Protection System

Monitors files across redundant storage, detects bit-decay and silent corruption using BLAKE3 checksums, automatically recovers from mirror copies, and supports live drive failover plus replacement rebuilds. Operates as a background daemon with both a CLI tool and a HTTPS REST API.

---

## Table of Contents

- [How It Works](#how-it-works)
- [Core Concepts](#core-concepts)
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

## Core Concepts

Before diving into commands, it helps to understand the five building blocks that everything else is built on.

| Concept | What it is |
| --- | --- |
| **Drive pair** | A named binding of a primary directory to a secondary (mirror) directory. All tracked files are scoped to a pair; the pair tracks which side is currently active and whether the system is mirroring, quiescing for replacement, or rebuilding. |
| **Tracked file / folder** | A file or folder whose BLAKE3 checksum is recorded in the database. Tracking queues a mirror copy by default. A tracked folder can be scanned to discover the files inside it, and watched for live changes. |
| **Sync queue** | A persistent work queue of pending `mirror`, `restore`, and `verify` actions. Processing the queue copies or restores files in the background. The queue can be paused and resumed at any time. |
| **Integrity run** | An async pass that re-hashes every tracked file against the stored baseline and classifies each result (`ok`, `mirror_corrupted`, `master_corrupted`, `both_corrupted`, etc.). With recovery enabled, BitProtector automatically restores from the healthy counterpart wherever one exists. |
| **Virtual path** | A user-defined absolute symlink that always resolves to the current active copy of a tracked file or folder. Symlinks are retargeted automatically when the active drive changes (e.g. after failover). |

---

## Prerequisites

| Requirement | Notes |
| --- | --- |
| Rust stable toolchain | Install via [rustup](https://rustup.rs) |
| Node.js 24+ | Required to build, lint, and test the React frontend |
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

The minimal path to protected, mirrored storage is three steps: register a drive pair, track your files, and process the mirror queue. Everything else — integrity checks, virtual paths, scheduling, and drive replacement — builds on that foundation.

### 1. Register a drive pair and start tracking

```bash
# Register a drive pair. Specifying the media type lets BitProtector tune
# BLAKE3 checksum parallelism (HDDs benefit from lower concurrency than SSDs).
bitprotector drives add mybackup /mnt/primary /mnt/mirror \
  --primary-media-type hdd --secondary-media-type hdd

# Track a single file. This records a BLAKE3 checksum and queues a mirror copy.
bitprotector files track <drive-pair-id> documents/report.pdf

# Or track a whole folder: add it, then scan to discover all files inside.
# The scan queues mirror work for every discovered file.
bitprotector folders add <drive-pair-id> documents
bitprotector folders scan <folder-id>

# Optional: watch a folder for live filesystem changes (runs until Ctrl+C).
# When a tracked file changes, the checksum is updated and re-mirroring is queued.
bitprotector folders watch <folder-id>
```

### 2. Mirror and verify

```bash
# Process the queue: copies every pending file from the active drive to the standby.
bitprotector sync process

# Optional: pause or resume the automatic queue processing at any time.
bitprotector sync pause
bitprotector sync resume

# Optional: bypass the queue and mirror a single file immediately.
bitprotector files mirror <file-id>

# Optional: mirror all unmirrored files under a folder right now.
bitprotector folders mirror <folder-id>

# Run an integrity check. Re-hashes all tracked files against stored baselines.
# Results are persisted and can be polled while the run is in progress.
bitprotector integrity check all

# Show a one-screen summary of drive health, mirror coverage, and recent events.
bitprotector status
```

### 3. Optional: virtual paths, drive replacement, backups, and scheduling

```bash
# Assign a virtual path: BitProtector creates a symlink exactly at this
# absolute location. The symlink retargets automatically after a failover.
bitprotector virtual-paths set <file-id> /docs/report.pdf

# Recreate all virtual path symlinks (e.g. after manual cleanup).
bitprotector virtual-paths refresh

# Planned primary replacement workflow.
# Use mark→confirm so you can quiesce external I/O before BitProtector switches over.
bitprotector drives replace mark <drive-pair-id> --role primary
# (optional) cancel if you change your mind:
bitprotector drives replace cancel <drive-pair-id> --role primary
bitprotector drives replace confirm <drive-pair-id> --role primary
bitprotector drives replace assign <drive-pair-id> --role primary /mnt/new-primary
bitprotector sync process   # rebuilds tracked files onto the replacement drive

# Register backup destinations for the SQLite database itself.
bitprotector database add /mnt/mirror/bitprotector.db --drive-label "mirror-drive"
bitprotector database run               # back up now to all enabled destinations
bitprotector database check-integrity   # verify backup copies, repair where possible

# Automate recurring work with the scheduler.
# Use a fixed interval (seconds) or a cron expression; --max-duration caps run time.
bitprotector scheduler add --task-type sync --interval 3600
bitprotector scheduler add --task-type integrity_check --cron "0 2 * * *" --max-duration 3600
bitprotector scheduler list
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
host           = "0.0.0.0"
port           = 8443
jwt_secret     = "replace-with-a-random-64-char-string"  # MUST be changed
tls_cert       = "/etc/bitprotector/tls/cert.pem"
tls_key        = "/etc/bitprotector/tls/key.pem"
rate_limit_rps = 100

[database]
path = "/var/lib/bitprotector/bitprotector.db"

[logging]
level = "info"
file  = "/var/log/bitprotector/bitprotector.log"

[scheduler]
enabled                    = true
sync_interval_seconds      = 3600
integrity_interval_seconds = 86400

[checksum]
# Lower parallelism for HDDs; set ssd_max_parallel = 0 to auto-detect CPU count
hdd_max_parallel = 2
ssd_max_parallel = 0
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

- **Dashboard** (`/`) shows a live summary of drive pair health, mirror status, and recent event log entries
- `/files` is the unified **Tracking Workspace** for both tracked files and tracked folders (`/folders` redirects to `/files`)
- tracked file and tracked folder forms use a filesystem browser dialog powered by the server
- drive pair and replacement-drive forms can also fill directory paths from the same browser
- tracked file/folder submissions are validated against the selected drive pair's active root before they are stored
- tracking and folder scans queue mirror work by default; use explicit mirror actions or sync processing for immediate copies
- **Sync Queue** page (`/sync`) lists pending, in-progress, and completed queue items and exposes pause/resume controls
- Integrity page starts/stops async runs, shows a running progress banner, and only lists issue rows (`needs_attention=true`)
- the latest run is loaded from DB automatically on page open; results are fetched in pages for responsiveness
- the Integrity page intro shows `Last integrity check` as a date/time timestamp (instead of a run ID)
- **Database Backups** page (`/database`) manages backup destinations, triggers manual backups, and verifies backup integrity
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
| [docs/testing/README.md](docs/testing/README.md) | How to run tests, test categories, QEMU suites, and the dedicated guest DB-disk layout used by installation scenarios |
| [docs/CI.md](docs/CI.md) | CI pipeline layers, local debugging with `act`, QEMU guest storage model, and nightly failure reproduction |

---

## AI assistance

Parts of this project were developed with assistance from AI tools
(GitHub Copilot, Claude). All AI-generated code has been reviewed,
tested, and modified by the maintainer before publication.

---

## License

MIT — see [LICENSE](LICENSE).
