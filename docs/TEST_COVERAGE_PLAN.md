# Test Coverage & QEMU Test Improvement Plan

> **Purpose:** self-contained work brief for an engineer (human or agent) picking up test-coverage improvements on this repo with no prior context from the originating conversation. Read §§1-4 before touching anything; §§5-8 describe concrete changes; §§9-10 describe the ordered PR sequence; §§11-12 describe verification and open questions.

---

## 1. Project context

BitProtector is a Rust daemon (`bitprotector_lib` crate + `bitprotector` binary) that mirrors files across redundant storage, detects corruption via BLAKE3 checksums, and manages failover between a primary and a mirror drive. It ships as a Debian package on Ubuntu 24.04 (primary target) and Ubuntu 26.04 (provisional). The product includes:

- Actix-web HTTPS REST API with JWT + PAM authentication
- A clap-based CLI that shares the library crate
- A React/TypeScript frontend served by the daemon
- A SQLite DB managed via `rusqlite` + `r2d2`
- systemd integration + profile.d login hook

### Key source paths

| Path | Contents |
|---|---|
| [src/lib.rs](../src/lib.rs) | Library entry point |
| [src/main.rs](../src/main.rs) | CLI entry, config-precedence logic |
| [src/api/](../src/api/) | API server, auth, path resolution |
| [src/api/routes/](../src/api/routes/) | 14 route modules (auth/drives/files/folders/etc.) |
| [src/cli/commands/](../src/cli/commands/) | clap subcommand handlers |
| [src/core/](../src/core/) | Mirror, integrity, sync queue, scheduler, drive, virtual_path, change_detection, tracker, checksum |
| [src/db/repository.rs](../src/db/repository.rs) | 2299-line R2D2+rusqlite DB layer |
| [src/db/schema.rs](../src/db/schema.rs) | Schema init + migrations |
| [src/logging/event_logger.rs](../src/logging/event_logger.rs) | Event-log abstraction |
| [tests/integration/](../tests/integration/) | Rust integration tests (17 files) |
| [tests/installation/](../tests/installation/) | QEMU shell test scripts |
| [frontend/src/](../frontend/src/) | React app (Vite + Vitest + Playwright) |
| [packaging/](../packaging/) | systemd unit, PAM config, default config.toml, maintainer scripts |
| [.github/workflows/](../.github/workflows/) | CI, nightly, release |
| [scripts/run-tests.sh](../scripts/run-tests.sh) | Native test runner |
| [scripts/ci-local.sh](../scripts/ci-local.sh) | `act`-based local CI reproduction |

### Conventions to follow

- Inline unit tests in `#[cfg(test)] mod tests { … }` at the bottom of each `src/` file
- Integration tests use `assert_cmd::Command::cargo_bin("bitprotector")` + `predicates` + `tempfile::{NamedTempFile, TempDir}`
- API/actix tests use `#[actix_rt::test]` (not `#[tokio::test]`) and `actix_web::test::{init_service, TestRequest, call_service}`
- Each integration test owns its DB via `NamedTempFile` → `--db <path>`
- See [docs/TESTING.md](TESTING.md) for examples
- QEMU helpers in [tests/installation/lib/qemu-helpers.sh](../tests/installation/lib/qemu-helpers.sh): `log`, `require_commands`, `resolve_ssh_key`, `resolve_guest_image`, `wait_for_vm`, `wait_for_api`
- CI docs in [docs/CI.md](CI.md)
- Project uses `anyhow` for top-level, `thiserror` for typed errors

---

## 2. Current state snapshot

As of branch `improve-test-coverage` (base v1.0.0-alpha2):

| Layer | Count | Notes |
|---|---|---|
| Rust inline unit tests (`src/**/*.rs`) | ~155 tests across 18 files | Strong on `core/`, `db/schema`, `logging/`, `api/auth`. Zero on `core/integrity_runs.rs` and `api/path_resolution.rs`. |
| Rust integration tests | ~187 tests across 17 files | [tests/integration/api_routes.rs](../tests/integration/api_routes.rs) alone is 2589 lines / 81 tests — too large. |
| Frontend unit tests (Vitest) | 22 `.test.{ts,tsx}` files | Only 2 of 13 `frontend/src/api/*.ts` modules have tests. |
| Frontend E2E (Playwright) | 7 specs | Missing: logs, database-backups, sync-queue, virtual-paths. |
| QEMU install tests | 3 scripts (smoke / failover / uninstall) | Daemon never exercised end-to-end; all assertions use CLI + `--db`. |
| CI pipeline | 8 layers, 2-guest matrix | See [.github/workflows/ci.yml](../.github/workflows/ci.yml). |

### Known coverage gaps

- `src/core/integrity_runs.rs` — 4 public fns (`start_run_async`, `run_sync`, `process_run`, `status_str`), **zero inline tests**
- `src/api/path_resolution.rs` — security-critical helper `resolve_path_within_drive_root`, **zero inline tests**; only indirectly tested through `api_routes.rs`
- `src/db/repository.rs` — 2299 lines but only 11 unit tests; filter combinations, transaction rollback, and concurrent-pool behaviour untested
- `src/main.rs` — config-precedence logic at [src/main.rs:170-184](../src/main.rs#L170-L184) (CLI flag > config file > default) is untested
- `tests/integration/api_routes.rs` needs splitting by route module
- No coverage reporting (Rust or frontend); [docs/CI.md:196](CI.md#L196) flags this as out-of-scope today
- No property-based tests (`proptest`) or fuzzing
- QEMU daemon is never tested end-to-end (no TLS, no login flow, no live API), no bit-flip recovery, no upgrade path, no filesystem-error scenarios

---

## 3. Goals and scope

### In scope

1. Fill Rust unit-test gaps in `core/integrity_runs.rs`, `api/path_resolution.rs`, `main.rs` config resolution, and selected `repository.rs` paths.
2. Split `tests/integration/api_routes.rs` by route module.
3. Add property-based tests for two path-handling helpers.
4. Add 33 QEMU scenarios, grouped into 8 bundles that each reuse a single VM (versus naïvely booting 33 VMs).
5. Refactor the QEMU harness to support bundle-based scenario runs, including snapshot/revert primitives.
6. Upload VM logs on CI failure.
7. Introduce non-gating coverage reporting (`cargo llvm-cov` + `vitest --coverage`).
8. Add Vitest tests for the 11 untested frontend API client modules.

### Out of scope

- Visual-regression testing
- Load / chaos testing infrastructure beyond what is in §6 (scale bundle)
- Moving CI runner class (`runs-on:`) — assume `ubuntu-24.04` is fine unless noted
- Codecov integration (coverage is uploaded as artifact only; wiring Codecov is a follow-up)
- Any product-behaviour changes; this plan adds tests, not features. The **only** production code changes allowed are: (a) extracting a pure `resolve_db_path` helper in `src/main.rs` to make it unit-testable, and (b) if the agent discovers a regression surfaced by a new test, fixing it is expected.

---

## 4. QEMU architecture

### 4.1 Bundle concept

Booting a VM costs 30-60 s (cloud-init + apt + service start). A test that takes 10 s on a running VM costs 90+ s in its own fresh VM. So: **bundle related scenarios into a single VM boot; reset app state between scenarios rather than rebooting**.

A VM is boot-shared only when scenarios need the same kernel cmdline, memory size, disk layout at boot, and install history.

### 4.2 Three reset techniques

Ordered cheapest-first; use the cheapest that works for each scenario.

1. **App-state reset** (~5 s) — fresh `--db <path>` + `rm -rf /mnt/primary/* /mnt/mirror/*`. Covers ~80% of scenarios.
2. **qcow2 overlay swap** (~1 s) — each scenario attaches a thin qcow2 overlay backed by a clean base image; overlays are discarded after. Use for disk-content scenarios (bit-flip, ENOSPC, read-only, cross-FS).
3. **QMP savevm/loadvm** (~1-3 s) — full VM-state snapshot/revert. Use between destructive scenarios (SIGKILL, panic, reboot).

### 4.3 Bundles

| Bundle | VM profile | Scenarios (see §6) |
|---|---|---|
| `smoke` | standard + TLS cert + PAM user | #10, #11, #13-16, #28, #29 + the 4 existing smoke checks + reboot (last) |
| `failover` | 4 extra virtio disks (ext4+xfs) + QMP socket | #17-20, #27 + 4 existing failover scenarios + large-file/both-corrupted/auto-recovery/folder-retarget |
| `resilience` | standard + QMP for savevm/loadvm | #1-3, #6-9 |
| `scale` | 100 GB extra disk, 8 GB RAM | #21, #22 |
| `scale-lowmem` | 1 GB RAM | #23 |
| `degraded-boot` | boot-time disk surgery | #4, #5 |
| `upgrade` | installs v1.0.0-alpha1 first | #24, #25 |
| `uninstall` | existing | existing 3 scenarios + #26 |

**Total VMs per matrix cell: 8.** Naïve (1 VM per scenario): 33.

### 4.4 Directory layout after refactor

```
tests/installation/
├── lib/
│   ├── qemu-helpers.sh            # existing — keep as-is
│   ├── scenarios.sh               # NEW — shared scenario primitives
│   ├── snapshots.sh               # NEW — QMP savevm/loadvm helpers
│   └── cloud-init/
│       ├── smoke.yaml             # NEW — TLS cert + PAM user + .deb install
│       ├── failover.yaml          # NEW — 4 disks, mixed FS, QMP socket
│       ├── resilience.yaml        # NEW — smoke + knobs for fault injection
│       ├── degraded-boot.yaml     # NEW — fstab with nofail, simulated missing disk
│       └── upgrade.yaml           # NEW — installs alpha1 before current
├── bundles/
│   ├── smoke.sh                   # replaces qemu_test.sh
│   ├── failover.sh                # replaces qemu_failover_test.sh
│   ├── resilience.sh              # NEW
│   ├── scale.sh                   # NEW
│   ├── scale_lowmem.sh            # NEW
│   ├── degraded_boot.sh           # NEW
│   ├── upgrade.sh                 # NEW
│   └── uninstall.sh               # replaces qemu_uninstall_test.sh
├── scenarios/
│   ├── smoke/         # 12 files (see §6.1)
│   ├── failover/      # 12 files (see §6.2)
│   ├── resilience/    # 8 files  (see §6.3)
│   ├── scale/         # 2 files  (see §6.4)
│   ├── scale-lowmem/  # 1 file   (see §6.5)
│   ├── degraded-boot/ # 2 files  (see §6.6)
│   ├── upgrade/       # 2 files  (see §6.7)
│   └── uninstall/     # 4 files  (see §6.8)
├── qemu_test.sh            # becomes 1-line wrapper: exec bundles/smoke.sh "$@"
├── qemu_failover_test.sh   # becomes 1-line wrapper: exec bundles/failover.sh "$@"
└── qemu_uninstall_test.sh  # becomes 1-line wrapper: exec bundles/uninstall.sh "$@"
```

Wrappers preserve backward compatibility with anything (docs, dev scripts, CI history) that references the old script names.

### 4.5 `lib/scenarios.sh` primitives

Implement as pure bash functions that call into the VM over the already-established SSH connection. Each scenario script sources `scenarios.sh` and uses these:

```bash
ssh_vm CMD                                # run CMD in the guest over SSH
make_pair NAME PRIMARY_ROOT MIRROR_ROOT   # register a drive pair, returns pair id on stdout
seed_file PATH SIZE_BYTES                 # create a file of given size, filled with /dev/urandom
corrupt_byte PATH OFFSET                  # flip one byte at OFFSET (bit-rot simulation)
reset_db DB_PATH                          # delete DB file so next command re-initialises
wait_for_sync_queue_empty DB_PATH SECS    # poll `sync queue list`, fail if not drained in SECS
assert_no_journal_errors SINCE_ISO8601    # fail if `journalctl -p err -u bitprotector --since SINCE` non-empty
run_scenario NAME FN                      # runs FN; on success prints 'PASS: NAME'; on failure prints 'FAIL: NAME' + return stream of serial log lines + aborts bundle
```

### 4.6 `lib/snapshots.sh` primitives (QMP)

Requires starting QEMU with `-qmp unix:$QMP_SOCKET,server=on,wait=off` (already the case for the failover script; add to all bundles that need savevm).

```bash
qmp_savevm SNAPSHOT_NAME     # sends savevm over QMP
qmp_loadvm SNAPSHOT_NAME     # sends loadvm over QMP
qmp_delvm SNAPSHOT_NAME      # sends delvm over QMP
qmp_device_add JSON          # hot-add device
qmp_device_del DEVICE_ID     # hot-remove (same primitive as current failover test)
```

Use `socat - UNIX-CONNECT:$QMP_SOCKET` for transport, matching the existing pattern in [qemu_failover_test.sh:65-72](../tests/installation/qemu_failover_test.sh#L65-L72).

### 4.7 Example scenario script structure

```bash
#!/bin/bash
# tests/installation/scenarios/smoke/smoke-08-pam-login.sh
# Scenario #13 — real PAM login via the API.
# Bundle: smoke. Assumes: TLS active, service running, PAM user `testauth` exists.

set -euo pipefail

ssh_vm '
set -euo pipefail
resp=$(curl -sk -X POST https://localhost:8443/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"testauth\",\"password\":\"hunter2\"}")
echo "$resp" | jq -e ".token" >/dev/null

bad=$(curl -sk -o /dev/null -w "%{http_code}" -X POST https://localhost:8443/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"testauth\",\"password\":\"WRONG\"}")
[[ "$bad" == "401" ]] || { echo "bad creds should return 401 but got $bad" >&2; exit 1; }
'
```

Each scenario script is independently `chmod +x`, uses `ssh_vm` from the sourced library, and leaves the VM in a state usable by the next scenario.

---

## 5. CI workflow changes

### 5.1 Existing jobs — no behavioural change, name alignment only

Keep `qemu-smoke` as the PR gate. Keep `qemu-failover` / `qemu-uninstall` as main + nightly + `workflow_dispatch` with `run_heavy_qemu=true` (matches current gating — see [.github/workflows/ci.yml:236-244](../.github/workflows/ci.yml#L236-L244)).

### 5.2 Add new jobs

All matrix'd on `[ubuntu-24.04, ubuntu-26.04]`, all gated identically to `qemu-failover`:

- `qemu-resilience` — runs `./tests/installation/bundles/resilience.sh`
- `qemu-upgrade` — runs `./tests/installation/bundles/upgrade.sh`
- `qemu-degraded-boot` — runs `./tests/installation/bundles/degraded_boot.sh`

These run in parallel after `qemu-smoke` passes.

### 5.3 Scale bundles move to nightly-only

In [.github/workflows/nightly.yml](../.github/workflows/nightly.yml), add:

- `qemu-scale` — `./tests/installation/bundles/scale.sh`
- `qemu-scale-lowmem` — `./tests/installation/bundles/scale_lowmem.sh`

Do **not** run these on PR — high runtime, low per-run signal delta.

### 5.4 Log upload on failure

Add to **every** QEMU job:

```yaml
- name: Upload VM logs on failure
  if: failure()
  uses: actions/upload-artifact@v5
  with:
    name: qemu-logs-${{ github.job }}-${{ matrix.guest }}
    path: |
      /tmp/**/serial.log
      /tmp/**/qemu.log
      /tmp/**/cloud-init.log
    retention-days: 7
    if-no-files-found: warn
```

### 5.5 Coverage reporting (non-gating)

New job `coverage`, runs after `unit`, never blocks:

```yaml
coverage:
  name: Coverage
  needs: unit
  runs-on: ubuntu-24.04
  continue-on-error: true
  steps:
    - uses: actions/checkout@v5
    - uses: ./.github/actions/setup-rust
    - uses: ./.github/actions/setup-frontend
    - name: Install llvm-cov
      run: cargo install cargo-llvm-cov --locked || true
    - name: Rust coverage
      run: cargo llvm-cov --lib --workspace --lcov --output-path rust.lcov
    - name: Rust coverage summary
      run: cargo llvm-cov report --summary-only
    - working-directory: frontend
      run: npm ci && npm test -- --coverage
    - uses: actions/upload-artifact@v5
      with:
        name: coverage-reports
        path: |
          rust.lcov
          frontend/coverage/
        retention-days: 14
```

### 5.6 Split-integration-test job updates

After phase 11 below (splitting `api_routes.rs`), update `rust-integration-fast` to run each new binary (see §7.3).

---

## 6. Scenario catalogue — all 33

Numbering matches the originating conversation. Each scenario becomes one script under `tests/installation/scenarios/<bundle>/<name>.sh`.

### 6.1 Smoke bundle

Cloud-init (`lib/cloud-init/smoke.yaml`) additionally provisions:
- Self-signed TLS cert at `/etc/bitprotector/tls/cert.pem` + `/etc/bitprotector/tls/key.pem` (use `openssl req -x509 -nodes -newkey rsa:2048 -days 365 -subj '/CN=localhost' ...`)
- `config.toml` pointing at those paths
- Local user `testauth` with password `hunter2` (`useradd -m testauth && echo 'testauth:hunter2' | chpasswd`)

| # | Scenario file | Purpose |
|---|---|---|
| E1 | smoke-01-package-installed.sh | (existing) `which bitprotector`, `bitprotector --version` |
| #— | smoke-02-service-active-with-tls.sh | `systemctl is-active bitprotector` = `active`; `curl -sk https://localhost:8443/api/v1/auth/login` reachable (expect 400 for empty body, **not** connection-refused) |
| E2 | smoke-03-cli-smoke.sh | (existing) `bitprotector --db /tmp/smoke.db drives list && status` |
| E3 | smoke-04-profile-d-installed.sh | (existing) file exists |
| #29 | smoke-05-profile-d-execution.sh | `bash /etc/profile.d/bitprotector-status.sh` runs and emits lines matching the sections produced in [src/cli/ssh_status.rs](../src/cli/ssh_status.rs) |
| #28 | smoke-06-ldd-version-sanity.sh | `ldd /usr/bin/bitprotector` contains no `not found`; PAM linked; `bitprotector --version` substring matches `dpkg -s bitprotector` Version |
| #11 | smoke-07-journald-integration.sh | Run a CLI action; `journalctl -u bitprotector --since "1 minute ago"` contains tracing output |
| #13 | smoke-08-pam-login.sh | POST `/api/v1/auth/login` with correct creds → 200 + token; wrong password → 401 |
| #14 | smoke-09-jwt-persists-across-restart.sh | Issue token; `systemctl restart bitprotector`; same token still accepted on `/api/v1/drives` |
| #15 | smoke-10-tls-cert-rotation.sh | Capture cert fingerprint; regenerate cert/key in place; reload service; new fingerprint differs |
| #16 | smoke-11-path-traversal-rejected.sh | POST `/files/track` with `../../etc/passwd`, `/etc/shadow`, symlink payloads escaping drive root — all must 400/403 |
| #10 | smoke-12-reboot-persistence.sh | **Last in bundle.** Register pair + track file; `sudo reboot`; re-wait for VM; service auto-active; DB state intact |

### 6.2 Failover bundle

Cloud-init (`lib/cloud-init/failover.yaml`) keeps the 4 extra disks from the current failover script but:
- `bpprimary` formatted ext4 (current)
- `bpmirror` formatted ext4 (current)
- `bpreplprimary` formatted **xfs** (new — exercises cross-FS path)
- `bpreplsecondary` formatted ext4 (current)
- Install `xfsprogs` in `runcmd`

| # | Scenario file | Purpose |
|---|---|---|
| E | failover-01-planned.sh | (existing) scenario 1 from current script |
| E | failover-02-emergency-qmp.sh | (existing) scenario 2 from current script (QMP hot-remove) |
| #17 | failover-03-bit-flip-auto-repair.sh | Track file → mirror → `corrupt_byte /mnt/mirror/file 512` → `integrity check all` → `mirror_corrupted` → `sync process` → clean |
| #— | failover-04-both-corrupted.sh | Corrupt different bytes on BOTH sides → integrity flags, no auto-recovery, event log contains `both_corrupted` |
| #— | failover-05-large-file-streaming.sh | Track 500 MB file; track+mirror+integrity+recovery; poll `/proc/$(pidof bitprotector)/status` VmRSS peak < 100 MB |
| #— | failover-06-integrity-triggered-auto-recovery.sh | Corrupt primary → integrity → `primary_corrupted` → sync process → auto-repair from mirror |
| #— | failover-07-virtual-path-folder-retarget.sh | Tracked folder with 10 files; virtual path set; planned failover; all 10 symlinks under `/mnt/mirror/...`; revert; all under `/mnt/primary/...` |
| #18 | failover-08-unicode-whitespace-long-paths.sh | Track files with Japanese names, double spaces, 240-char base names; mirror+integrity succeed |
| #19 | failover-09-two-pairs-one-disk.sh | Pairs A + B under `/mnt/primary/{pairA,pairB}` and `/mnt/mirror/{pairA,pairB}`; fail A, B stays primary-active |
| #20 | failover-10-cross-fs-matrix.sh | Pair with ext4 primary + xfs mirror (`bpreplprimary`); mirror + integrity + recovery all succeed |
| #27 | failover-11-device-add-hot-insert.sh | Start with replacement-primary `device_del`'d; `drives replace assign` fails; `qmp_device_add`; re-assign succeeds; sync rebuilds |
| #— | failover-12-qmp-hot-remove-secondary.sh | Mirror of scenario 02 but active-secondary role; emergency-fails to primary |

### 6.3 Resilience bundle

Cloud-init (`lib/cloud-init/resilience.yaml`) = smoke setup + QMP socket for savevm/loadvm. Take `savevm baseline` after cloud-init completes. `loadvm baseline` between destructive scenarios.

| # | Scenario file | Purpose |
|---|---|---|
| #1 | resilience-01-enospc.sh | Fill `/mnt/mirror` via `fallocate -l <size> filler`; mirror request → sync item `failed`; remove filler; retry succeeds |
| #2 | resilience-02-readonly-mirror.sh | `mount -o remount,ro /mnt/mirror`; sync fails cleanly (no panic); remount rw; retry succeeds |
| #3 | resilience-03-eacces-tracked-file.sh | `chmod 000` tracked file; integrity reports unreadable; restore perms; clean |
| #6 | resilience-04-symlink-loop.sh | `ln -s . loop` in tracked folder; `folders scan` terminates cleanly within 10 s |
| #7 | resilience-05-sigterm-mid-sync.sh | Queue 500 files; start `sync process`; `SIGTERM` after 1 s; no partial writes; queue consistent |
| #8 | resilience-06-sigkill-recovery.sh | Queue 500 files; start `sync process`; `kill -9` after 1 s; restart service; queue self-heals (no `in_progress` stuck rows); resumed sync completes without duplicates |
| #9 | resilience-07-auto-restart-after-panic.sh | [packaging/bitprotector.service](../packaging/bitprotector.service) already has `Restart=on-failure RestartSec=5s`; trigger abnormal exit via `sudo kill -SEGV $(pidof bitprotector)`; assert systemd brings service back within 15 s (`systemctl is-active` = `active`) |
| #12 | resilience-08-journal-error-scraper.sh | **Last in bundle.** `assert_no_journal_errors <bundle_start>`. Also runs at end of smoke, failover, upgrade, uninstall as a final scenario — implemented once in `lib/scenarios.sh` |

### 6.4 Scale bundle

Cloud-init: 100 GB extra disk at `/mnt/scale`, 8 GB RAM, standard install. **Nightly only.**

| # | Scenario file | Purpose |
|---|---|---|
| #21 | scale-01-100k-real-files.sh | Generate 100k tiny files under `/mnt/scale/docs`; `folders scan` < 10 min; `sync process` + `integrity check all` measured; timings written to `/tmp/scale-results.txt` |
| #22 | scale-02-inotify-saturation.sh | Record `cat /proc/sys/fs/inotify/max_user_watches`; track 5000 folders; service stays healthy OR logs a clear warning; fail on silent truncation |

### 6.5 Scale-lowmem bundle

Cloud-init: standard, 1 GB RAM. **Nightly only.**

| # | Scenario file | Purpose |
|---|---|---|
| #23 | scale-lowmem-01-4gb-dataset.sh | Generate 4 GB across 8 × 512 MB files; track + mirror + integrity; `dmesg | grep 'Killed process'` empty; bitprotector RSS peak < 300 MB |

### 6.6 Degraded-boot bundle

Cloud-init: `fstab` entry for `/mnt/primary` uses `nofail` but the disk is absent (or the mount point is a plain directory with no root-marker file).

| # | Scenario file | Purpose |
|---|---|---|
| #4 | degraded-boot-01-fake-mount-point.sh | `/mnt/primary` exists as plain dir (never mounted); service starts; `GET /drives/<id>` shows degraded; integrity returns `primary_unavailable` |
| #5 | degraded-boot-02-device-absent-at-boot.sh | fstab `nofail` entry with disk not attached; VM boots; service starts; pair shows degraded |

### 6.7 Upgrade bundle

Cloud-init: install `bitprotector_1.0.0~alpha1_amd64.deb` (built from tag `v1.0.0-alpha1` at CI time — see §12 Q1). Scenarios run after install.

| # | Scenario file | Purpose |
|---|---|---|
| #24 | upgrade-01-alpha1-to-current-with-live-data.sh | Install alpha1 → register 2 pairs + 100 files + integrity; `dpkg -i <current>.deb`; service restarts; schema migrates; 100 files still tracked; integrity clean |
| #25 | upgrade-02-reinstall-preserves-config.sh | Edit `/etc/bitprotector/config.toml`; `apt-get install --reinstall -o Dpkg::Options::="--force-confdef" -o Dpkg::Options::="--force-confold" bitprotector`; user edits preserved |

### 6.8 Uninstall bundle

Cloud-init: standard (matches current `qemu_uninstall_test.sh`).

| # | Scenario file | Purpose |
|---|---|---|
| E | uninstall-01-package-installed.sh | (existing) |
| E | uninstall-02-create-data.sh | (existing) creates package-owned DB + backup dir |
| E | uninstall-03-purge.sh | (existing) `apt-get purge`, verify all package paths removed |
| #26 | uninstall-04-purge-preserves-user-drive-data.sh | Before purge: write distinctive content to `/mnt/primary/docs/keeper.txt`; after purge: file still exists |

### 6.9 Cross-cutting (not scenarios — implemented in §§4-5)

- **#30** Upload logs on failure — CI change, §5.4
- **#31** Shared helpers — `lib/scenarios.sh`, §4.5
- **#32** Scenario runner — `run_scenario` function, §4.5
- **#33** Baseline VM reuse via savevm/loadvm — `lib/snapshots.sh`, §4.6

---

## 7. Rust / Frontend coverage items (non-QEMU)

### 7.1 New inline unit tests

#### `src/core/integrity_runs.rs`

Add `#[cfg(test)] mod tests` at bottom. Cover:
- `status_str` returns stable strings for each `IntegrityStatus` variant (trivial)
- `start_run_async` creates a persisted run row with expected initial status
- `process_run` advances status, persists per-file results, marks run complete
- `run_sync` is equivalent to the async path for tiny datasets
- Cancellation / stop path leaves the run row in `stopped` state with partial results

Use in-memory `Repository` via the existing pattern (`tempfile::NamedTempFile` for SQLite file or `rusqlite::Connection::open_in_memory` if the repo layer supports it).

#### `src/api/path_resolution.rs`

Add `#[cfg(test)] mod tests`. Cover `resolve_path_within_drive_root`:
- Absolute path inside the drive root → returns canonicalised path
- Absolute path outside the drive root → error
- Relative path inside root → ok
- `../` escape attempt → error
- Symlink inside root pointing outside root → error
- Path with unicode/whitespace → ok
- Non-existent path → error (specific error variant if one exists)
- Root itself → ok

Use `tempfile::TempDir` for drive-root fixtures.

#### `src/main.rs` → extract `resolve_db_path`

Currently [src/main.rs:176-184](../src/main.rs#L176-L184) inlines the precedence logic. Extract:

```rust
fn resolve_db_path(cli_db: &str, cli_default: &str, file_cfg_db: Option<&str>) -> String {
    if cli_db != cli_default {
        cli_db.to_string()
    } else if let Some(p) = file_cfg_db {
        p.to_string()
    } else {
        cli_db.to_string()
    }
}
```

Then add `#[cfg(test)] mod tests` covering all three precedence cases. This is the only production-code change this plan allows besides the bitprotector.service update (already in place).

#### `src/db/repository.rs` — new tests to add

- `test_list_tracking_items_filter_combinations` — exhaustive combos of `source`, `virtual_prefix`, `has_virtual_path`, pagination. Ensure result sets match expectations when multiple filters apply.
- `test_transaction_rollback_on_fk_violation` — start tx, insert tracked file against bogus `drive_pair_id`, ensure FK violation rolls back cleanly.
- `test_clear_completed_sync_queue_preserves_in_flight` — insert a mix of `pending`, `in_progress`, `completed`, `failed`; call clearer; verify only `completed` rows are removed.
- `test_connection_pool_behaviour_under_concurrent_use` — spawn N threads, each opens a connection from the pool, performs a small read, returns. No deadlocks, no panics.

### 7.2 Property-based tests

Add to `[dev-dependencies]` in [Cargo.toml](../Cargo.toml):

```toml
proptest = "1"
```

Add `proptest!` blocks:

#### `src/core/virtual_path.rs`
- `normalize_virtual_path` is idempotent: `normalize(normalize(x)) == normalize(x)` for all inputs
- Normalised path never contains `..` segments
- Normalised path never starts with anything but `/`

#### `src/api/path_resolution.rs`
- For any input path, the result of `resolve_path_within_drive_root(input, root)` is either an error OR a path that starts with (the canonicalised) `root`

Use `proptest::strategy` for path generation with both `/`-containing strings and arbitrary UTF-8. Constrain lengths to avoid test timeouts.

### 7.3 Split `tests/integration/api_routes.rs`

Current file: 2589 lines / 81 tests. Target split:

| New file | Tests covering |
|---|---|
| `tests/integration/api_drives.rs` | `/api/v1/drives/*` (incl. replace mark/cancel/confirm/assign) |
| `tests/integration/api_files.rs` | `/api/v1/files/*` |
| `tests/integration/api_folders.rs` | `/api/v1/folders/*` |
| `tests/integration/api_tracking.rs` | `/api/v1/tracking/items` (scaling + filters) |
| `tests/integration/api_integrity.rs` | `/api/v1/integrity/*` (runs, results, active, stop) |
| `tests/integration/api_sync.rs` | `/api/v1/sync/*` |
| `tests/integration/api_virtual_paths.rs` | `/api/v1/virtual-paths/*` |

Every split file needs:
- An entry under `[[test]]` in [Cargo.toml](../Cargo.toml) matching the existing pattern around line 128
- A matching `cargo test --test <name>` step in [.github/workflows/ci.yml](../.github/workflows/ci.yml) job `rust-integration-fast`
- A matching line in `run_rust_integration_fast()` in [scripts/run-tests.sh](../scripts/run-tests.sh)

Keep `tests/integration/api_routes.rs` as a thin file that only covers cross-module concerns that don't belong in any single split (404 handling, auth middleware, CORS, rate limiting). It should end up ~100-200 lines.

### 7.4 Frontend API client unit tests

Following the existing pattern in [frontend/src/api/client.test.ts](../frontend/src/api/client.test.ts) and [frontend/src/api/filesystem.test.ts](../frontend/src/api/filesystem.test.ts), add tests for:

- `auth.ts`, `database.ts`, `drives.ts`, `files.ts`, `folders.ts`, `integrity.ts`, `logs.ts`, `scheduler.ts`, `status.ts`, `sync.ts`, `tracking.ts`, `virtual-paths.ts`

Minimum per file:
- One happy-path test per exported function
- One error-path test per function that has error handling
- Mock HTTP via `msw` or whatever the existing tests use — check `frontend/src/test/` for helpers

### 7.5 Coverage reporting

See §5.5 for the CI job. Additionally:

- Add `"test:coverage": "vitest run --coverage"` script to [frontend/package.json](../frontend/package.json)
- Ensure `@vitest/coverage-v8` is in `devDependencies`
- Update [docs/TESTING.md](TESTING.md) §Running Tests with a coverage section

---

## 8. Implementation phases (separate PRs)

Each phase is a self-contained PR. Order matters — earlier phases unlock later ones.

| # | Phase | Scope | Unlocks |
|---|---|---|---|
| 1 | QEMU harness refactor | `lib/scenarios.sh`, `lib/snapshots.sh`, `bundles/*.sh`, `scenarios/*/` directory structure, backward-compat wrappers. **Port existing scenarios as-is** — no new coverage. CI remains green. | All later QEMU phases |
| 2 | Log upload on failure | CI workflow change (§5.4). | Easier debugging for all later phases |
| 3 | Journal error scraper | `assert_no_journal_errors` helper + hook into every existing bundle as final scenario. | Immediate signal from existing scenarios |
| 4 | Smoke bundle expansion | Add smoke-02 through smoke-12 (§6.1). Update `cloud-init/smoke.yaml` (TLS cert + PAM user). | — |
| 5 | Failover bundle expansion | Add failover-03 through failover-12 (§6.2). Update `cloud-init/failover.yaml` (xfs on replacement-primary). | — |
| 6 | Resilience bundle | New bundle + 8 scenarios (§6.3) + new CI job `qemu-resilience`. | — |
| 7 | Upgrade bundle | New bundle + 2 scenarios (§6.7) + new CI job `qemu-upgrade`. Building alpha1 at CI time (see §12 Q1). | — |
| 8 | Uninstall +1 | Add scenario #26 to uninstall bundle (§6.8). | — |
| 9 | Degraded-boot bundle | New bundle + 2 scenarios (§6.6) + new CI job `qemu-degraded-boot`. | — |
| 10 | Scale bundles | Two new bundles + 3 scenarios (§§6.4-6.5) + nightly-only CI jobs. | — |
| 11 | Rust inline unit tests | §7.1: integrity_runs, path_resolution, main.rs resolve_db_path extraction, repository.rs additions. | — |
| 12 | Split `api_routes.rs` | §7.3: split by module, update Cargo.toml + CI + run-tests.sh. Atomic single PR. | — |
| 13 | Property tests | §7.2: add `proptest` dep + suites on virtual_path + path_resolution. | — |
| 14 | Frontend API client tests | §7.4: 11 new test files. | — |
| 15 | Coverage reporting | §5.5 + §7.5: CI job + package.json script + docs update. | — |

Phases 1-3 must land first and in order. Phases 4-10 can land in any order and in parallel once 1-3 are in. Phases 11-15 are independent of QEMU work and can run in parallel.

---

## 9. Verification checklist

For each PR:

- [ ] `cargo fmt --check`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test --lib` (local)
- [ ] For phases touching Rust integration: `cargo test` (all integration binaries)
- [ ] For phases touching smoke: `./scripts/run-tests.sh smoke` (local)
- [ ] For phases touching failover or later: `./scripts/run-tests.sh full` (local)
- [ ] For frontend phases: `cd frontend && npm run lint && npm test`
- [ ] Branch CI all green (all PR-gate jobs; heavy jobs where applicable)

For QEMU phases, additionally:

- [ ] Force a failure locally (inject a deliberate `exit 1` in one new scenario) and confirm the log-upload step captures `serial.log` + `qemu.log` as a GH Actions artifact
- [ ] Inspect `journalctl` output from the serial log to ensure new scenarios actually exercise the daemon when they claim to

End-state after phase 15:

- [ ] `cargo llvm-cov report --summary-only` shows `src/core/` ≥ 85 % line coverage and `src/api/` ≥ 80 % line coverage
- [ ] `./scripts/run-tests.sh full` passes end-to-end on a clean workstation with QEMU prerequisites installed
- [ ] All bundles complete under 15 min per guest in CI
- [ ] Nightly CI includes scale + scale-lowmem and passes

---

## 10. Non-regression rules for the implementing agent

- **Do not delete existing tests** — only refactor locations (e.g. moving scenarios into `scenarios/<bundle>/` as part of phase 1).
- **Do not skip or `continue-on-error` any test to make CI green.** If a new scenario reveals a real bug, fix the bug as part of that phase's PR.
- **Do not change product behaviour** beyond the two explicit allowances in §3 (`resolve_db_path` extraction; any bug fixes surfaced by new tests). Cross-check with the PR description.
- **Do not remove the backward-compat wrappers** at `tests/installation/qemu_*.sh` — external references (`docs/TESTING.md`, dev habits) depend on them.
- **Do not hard-code SSH ports** in new bundles. Reuse the env-var pattern (`SSH_PORT`, `API_PORT`) already in the existing scripts, with unique defaults per bundle (next free range after 2226).
- **Scenario scripts must be idempotent** within their bundle — a re-run after pass or fail must not leave the VM in an unrecoverable state before the next scenario (use `reset_db` / `loadvm baseline` as needed).
- **Every new scenario must include `assert_no_journal_errors` implicitly** by virtue of the `run_scenario NAME FN` wrapper calling it before emitting PASS. If a scenario expects errors in the journal (e.g. resilience-01 ENOSPC), it must scrape them out or tag them before the assertion — implement an `expect_journal_error PATTERN` helper.

---

## 11. File-by-file change summary (for phase planning)

| File | Phase | Change |
|---|---|---|
| [tests/installation/lib/qemu-helpers.sh](../tests/installation/lib/qemu-helpers.sh) | 1 | Keep; consumed by `bundles/*.sh` |
| `tests/installation/lib/scenarios.sh` | 1 | NEW |
| `tests/installation/lib/snapshots.sh` | 1 | NEW |
| `tests/installation/lib/cloud-init/*.yaml` | 1 (skeleton), 4/5/6/7/9 (per-bundle) | NEW |
| `tests/installation/bundles/*.sh` | 1 (existing 3), 6/7/9/10 (new 4) | NEW |
| `tests/installation/scenarios/*/*.sh` | 1 (port existing), 4-10 (new) | NEW |
| [tests/installation/qemu_test.sh](../tests/installation/qemu_test.sh) | 1 | Becomes `exec bundles/smoke.sh "$@"` |
| [tests/installation/qemu_failover_test.sh](../tests/installation/qemu_failover_test.sh) | 1 | Becomes `exec bundles/failover.sh "$@"` |
| [tests/installation/qemu_uninstall_test.sh](../tests/installation/qemu_uninstall_test.sh) | 1 | Becomes `exec bundles/uninstall.sh "$@"` |
| [.github/workflows/ci.yml](../.github/workflows/ci.yml) | 2 (upload), 6/7/9 (new jobs), 12 (integration splits), 15 (coverage) | Edits |
| [.github/workflows/nightly.yml](../.github/workflows/nightly.yml) | 10 (scale jobs) | Edits |
| [scripts/run-tests.sh](../scripts/run-tests.sh) | 6/7/9/10 (new bundles), 12 (integration splits) | Edits |
| [Cargo.toml](../Cargo.toml) | 12 (test binaries), 13 (proptest) | Edits |
| [src/main.rs](../src/main.rs) | 11 | Extract `resolve_db_path` |
| [src/core/integrity_runs.rs](../src/core/integrity_runs.rs) | 11 | Add `#[cfg(test)] mod tests` |
| [src/api/path_resolution.rs](../src/api/path_resolution.rs) | 11 | Add `#[cfg(test)] mod tests` |
| [src/db/repository.rs](../src/db/repository.rs) | 11 | Add 4 new tests |
| [src/core/virtual_path.rs](../src/core/virtual_path.rs) | 13 | Add `proptest!` block |
| [tests/integration/api_routes.rs](../tests/integration/api_routes.rs) | 12 | Split into 7 files; keep as thin cross-cutting file |
| [tests/integration/api_{drives,files,folders,tracking,integrity,sync,virtual_paths}.rs](../tests/integration/) | 12 | NEW |
| [frontend/src/api/*.test.ts](../frontend/src/api/) | 14 | NEW (11 files) |
| [frontend/package.json](../frontend/package.json) | 15 | Add `test:coverage` script |
| [docs/TESTING.md](TESTING.md) | 15 | Document coverage command |
| [docs/CI.md](CI.md) | 2/6/7/9/10/15 | Update pipeline description as jobs are added |

---

---

## 13. When this plan is done

Delete or archive this file (`docs/TEST_COVERAGE_PLAN.md`). Update [docs/TESTING.md](TESTING.md) and [docs/CI.md](CI.md) to reflect the new bundle architecture. The resulting test suite should be:

- ~210 Rust inline unit tests (up from ~155)
- ~260 Rust integration tests across ~23 binaries (up from ~187 across 17)
- Full coverage of frontend API client modules
- 8 QEMU bundles exercising 45 scenarios across 16 VM boots per CI run (up from 3 bundles / 9 scenarios / 6 VM boots)
- Non-gating coverage report on every PR
