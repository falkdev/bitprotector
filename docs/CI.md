# CI Pipeline

This document explains the GitHub Actions pipeline, how each layer maps to jobs, how to reproduce a failure locally, and how to trigger the full suite manually.

---

## Table of Contents

- [Pipeline Overview](#pipeline-overview)
- [Trigger Matrix](#trigger-matrix)
- [Layer Reference](#layer-reference)
- [QEMU Guest Storage Model](#qemu-guest-storage-model)
- [Local Debugging with `act`](#local-debugging-with-act)
- [Native Local Runs (no Docker)](#native-local-runs-no-docker)
- [Reproducing a Specific Failure](#reproducing-a-specific-failure)
- [Cached Artifacts](#cached-artifacts)
- [Environment Variables and Overrides](#environment-variables-and-overrides)
- [Extending the Pipeline](#extending-the-pipeline)

---

## Pipeline Overview

The pipeline is **layered and fail-fast**. Cheaper, faster tests gate the expensive QEMU layers, and independent jobs fan out in parallel where possible (for example, coverage runs alongside Rust integration after unit tests).

```text
lint → unit → (coverage, non-gating) → rust-integration-fast → rust-integration-heavy
                                                           ↓
                                                     build-artifacts
                                                           ↓
                                                     qemu-smoke
                  ┌──────────────────────┬────────────────────────────┬───────────────┬──────────────┬──────────────┬───────────────────────┬─────────────────────────┬──────┐
                  ↓                      ↓                            ↓               ↓              ↓              ↓                       ↓                         ↓       ↓
     qemu-application-workflows    qemu-failover                qemu-uninstall   qemu-resilience  qemu-upgrade  qemu-degraded-boot   qemu-drive-media-type   e2e   (nightly) qemu-scale + qemu-scale-lowmem + qemu-scheduled-load
```

All workflow YAML lives in [.github/workflows/](.github/workflows/). QEMU jobs use composite actions in [.github/actions/](.github/actions/) for setup.

---

## Trigger Matrix

| Event | Layers run |
| --- | --- |
| `pull_request` | 0 – 12 (full CI workflow in `ci.yml`) |
| `push` to `main` | 0 – 12 |
| `workflow_dispatch` with `run_heavy_qemu=true` | 0 – 12 |
| `workflow_dispatch` with `run_heavy_qemu=false` (default) | 0 – 5 (`qemu-smoke` only for QEMU layer) |
| Nightly cron (03:00 UTC via `nightly.yml`) | Full CI (0 – 12) + nightly-only `qemu-scale` + `qemu-scale-lowmem` + `qemu-scheduled-load` + `qemu-drive-media-type` |

PR runs use `cancel-in-progress: true` so a new push automatically cancels the previous run. Pushes to `main` never cancel (a heavy failover run mid-flight must finish).

---

## Layer Reference

| # | Job name | Content | Runner | Expected time |
| --- | --- | --- | --- | --- |
| 0 | `lint` | `cargo fmt --check`, `cargo clippy -D warnings`, `npm run lint`, prettier check | ubuntu-24.04 | < 2 min |
| 1 | `unit` | `cargo test --lib` + `npm test` (vitest/jsdom) | ubuntu-24.04 | 2-4 min |
| — | `coverage` | `cargo llvm-cov` + `npm run test:coverage` artifact upload (non-gating) | ubuntu-24.04 | 4-8 min |
| 2 | `rust-integration-fast` | CLI + split API integration binaries + core integration tests (except `scaling_100k`) | ubuntu-24.04 | 4-7 min |
| 3 | `rust-integration-heavy` | `cargo test --test scaling_100k` (100k rows, 3 s/query budgets) | ubuntu-24.04 | 2-4 min |
| 4 | `build-artifacts` | Docker-based build: `scripts/build-deb.sh` runs inside `bitprotector-deb-builder:ubuntu-<VER>` → uploads `.deb` as artifact | ubuntu-24.04 | 4-6 min |
| 5 | `qemu-smoke` | Matrix: Ubuntu 24.04 + 26.04. Installs `.deb`, smoke scenarios including scheduler + DB-backup smoke coverage. | ubuntu-24.04 | 10-14 min per guest |
| 6 | `qemu-application-workflows` | Matrix: Ubuntu 24.04 + 26.04. Scheduled sync/integrity/backup workflows, backup repair, restart persistence. | ubuntu-24.04 | 20-35 min per guest |
| 7 | `qemu-resilience` | Matrix: Ubuntu 24.04 + 26.04. ENOSPC/readonly/signal/restart scenarios. | ubuntu-24.04 | 15-25 min per guest |
| 8 | `qemu-upgrade` | Matrix: Ubuntu 24.04 + 26.04. alpha1 → current upgrade scenarios. | ubuntu-24.04 | 20-30 min per guest |
| 9 | `qemu-degraded-boot` | Matrix: Ubuntu 24.04 + 26.04. Degraded boot scenarios. | ubuntu-24.04 | 10-15 min per guest |
| 10 | `qemu-failover` | Matrix: Ubuntu 24.04 + 26.04. Failover scenarios + QMP hot-remove. | ubuntu-24.04 | 15-20 min per guest |
| 11 | `qemu-drive-media-type` | Matrix: Ubuntu 24.04 + 26.04. Drive media type + `active_workers` integrity progress checks. | ubuntu-24.04 | 10-15 min per guest |
| 12 | `qemu-uninstall` | Matrix: Ubuntu 24.04 + 26.04. Purge/uninstall scenarios. | ubuntu-24.04 | 8-12 min per guest |
| 13 | `e2e` | Playwright E2E suite against a dedicated ubuntu-24.04 QEMU guest. Boots the guest via `tests/installation/e2e-guest.sh`, runs all 8 spec files (`drives`, `file-browser`, `scheduler`, `integrity`, `folders`, `auth-and-nav`, `database-backups`, `dashboard`). | ubuntu-24.04 | 10-20 min |

Nightly-only jobs in `nightly.yml` also run `qemu-scale`, `qemu-scale-lowmem`, `qemu-scheduled-load`, and `qemu-drive-media-type` (all matrixed across Ubuntu 24.04 + 26.04).

**Runner vs guest OS**: the runner is always `ubuntu-24.04` (GitHub-hosted). The *guest* running inside QEMU is controlled by the matrix (`ubuntu-24.04` noble, `ubuntu-26.04` resolute). See [.github/actions/setup-qemu/action.yml](.github/actions/setup-qemu/action.yml) for image download logic.

**`scaling_100k` timing budget**: the test enforces a 3000 ms per-query budget ([tests/integration/scaling_100k.rs](../tests/integration/scaling_100k.rs)). On slow runners this may flake. If it does, bump the budget via the `SCALING_QUERY_BUDGET_MS` env var (if wired), or move the job to a larger runner by changing its `runs-on` label — one-line change.

---

## QEMU Guest Storage Model

All QEMU installation bundles attach a dedicated guest database disk:

- `serial=bpdb` virtual disk (32G qcow2)
- mounted in-guest at `/mnt/bitprotector-db`
- scenario DB files written to `/mnt/bitprotector-db/db`

This is intentional for nightly scale stability: large scenario metadata no longer competes with guest root `/tmp` capacity.

If a nightly scale job fails with `database or disk is full`:

1. Inspect the failing step logs for the scale scenario `df -h` lines (`/`, `/mnt/scale`, `/mnt/bitprotector-db`).
2. Confirm the DB mount preflight passed (`/mnt/bitprotector-db` mounted and writable).
3. Download and inspect the uploaded `qemu-logs-*` artifact for the failing matrix cell.

---

## Local Debugging with `act`

[`act`](https://github.com/nektos/act) runs GitHub Actions workflows inside Docker containers locally. The same YAML you push to GitHub runs on your machine.

### Install act

```bash
# macOS
brew install act

# Linux (installs to /usr/local/bin)
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash
```

### Run a layer

```bash
./scripts/ci-local.sh lint
./scripts/ci-local.sh fast     # lint + unit + integration (no QEMU)
./scripts/ci-local.sh smoke    # + build + QEMU smoke (requires /dev/kvm)
./scripts/ci-local.sh full     # + full QEMU suite
```

The wrapper uses `catthehacker/ubuntu:full-latest` which includes `qemu-system-x86_64`. It also passes `--device /dev/kvm --privileged` so KVM is available inside the container.

### Run a single job

```bash
./scripts/ci-local.sh smoke --job qemu-smoke
```

### Restrict to one matrix cell

```bash
./scripts/ci-local.sh smoke --matrix guest:ubuntu-24.04
```

### act quirks

- `actions/cache` doesn't fully emulate GitHub's cache service — repeat local runs may be slower than CI on the first run.
- Matrix expansion works but you may prefer `--matrix` to iterate one cell at a time.
- Artifact paths are served by act's local server (`/tmp/act-artifacts` by default, see [.actrc](.actrc)).

---

## Host Prerequisites

For local builds and the `smoke`/`full` layers, you need:

| Tool | Used by |
| --- | --- |
| Docker | `run-tests.sh smoke` / `full` (Layer 4 — `build-artifacts`) and direct `build-deb.sh` invocations |
| QEMU + KVM | Layer 5+ QEMU tests (run `./scripts/setup-qemu.sh` first) |
| Rust toolchain | Layers 0–3 (lint, unit, integration) |
| Node.js 24+ | Layers 0–1 (lint, frontend unit) |

Layer 4 (`build-artifacts`) requires **only Docker** — no Rust or Node.js on the host. See [docs/BUILDING.md](BUILDING.md) for details.

---

## Native Local Runs (no Docker)

If you don't want the `act` overhead, run layers natively using the same layer subcommands:

```bash
./scripts/run-tests.sh lint
./scripts/run-tests.sh fast
./scripts/run-tests.sh smoke   # requires Docker (for build-deb.sh) + QEMU (run setup-qemu.sh first)
./scripts/run-tests.sh full    # includes qemu-application-workflows + Playwright E2E
GUEST_IMAGE=ubuntu-24.04 ./tests/installation/bundles/application_workflows.sh
GUEST_IMAGE=ubuntu-24.04 ./tests/installation/bundles/scheduled_load.sh   # nightly-style load
# Run Playwright E2E only (boots its own VM, runs all specs, then stops VM):
./scripts/run-tests.sh e2e
```

Layers 0–3 call cargo/npm directly. Layer 4 calls `scripts/build-deb.sh` which uses Docker. The layer definitions are shared with `ci-local.sh`.

To build a `.deb` directly without running the full test suite:

```bash
./scripts/build-deb.sh --ubuntu-version 24.04
./scripts/build-deb.sh --ubuntu-version 26.04
```

---

## Reproducing a Specific Failure

1. **Find the failing job** in the Actions tab. Note the job name (e.g., `qemu-smoke (ubuntu-26.04)`).

2. **Run locally via act**:

   ```bash
   ./scripts/ci-local.sh smoke --job qemu-smoke --matrix guest:ubuntu-26.04
   ```

3. **Run natively** (faster iteration):

   ```bash
   GUEST_IMAGE=ubuntu-26.04 ./tests/installation/qemu_test.sh
   GUEST_IMAGE=ubuntu-26.04 ./tests/installation/bundles/application_workflows.sh
   ```

4. **Inspect serial console output** — the scripts stream boot lines to your terminal. The raw log is also at `${WORKDIR}/serial.log` inside the QEMU working directory.

   For scheduled-load scenarios, inspect `timing:` lines such as:
   - `timing: scheduled-load-01 generation_seconds=...`
   - `timing: scheduled-load-01 pending_zero_seconds=...`
   - `timing: scheduled-load-02 backup_first_observed_seconds=...`
   - `timing: scheduled-load-02 backup_repair_seconds=...`

5. **Trigger the heavy suite manually from a PR branch** (without merging to main):
   - Go to Actions → CI → Run workflow → check `run_heavy_qemu` → Run.

---

## Cached Artifacts

### Rust build cache

`Swatinem/rust-cache@v2` caches `~/.cargo/registry`, `~/.cargo/git`, and the `target/` directory. Expect > 50% reduction in layer 2 time on cache hits.

### npm cache

`actions/setup-node@v5` installs Node.js 24 and caches the npm cache keyed on `frontend/package-lock.json`. After the first run, `npm ci` time is near-zero.

### Cloud images

Each Ubuntu image (~650 MB) is cached under `~/images/` via `actions/cache@v5` with a per-guest monthly key (`ubuntu-24.04-YYYYMM`, `ubuntu-26.04-YYYYMM`). The cache refreshes automatically at the start of each month. If a critical CVE lands and you need a fresh image immediately, clear the cache manually via the Actions UI (Actions → Caches).

### .deb artifact

`build-artifacts` uploads `bitprotector_*.deb` as matrix artifacts (`bitprotector-deb-ubuntu-24.04`, `bitprotector-deb-ubuntu-26.04`) with 7-day retention. QEMU jobs in `ci.yml` download those artifacts directly — the binary is never rebuilt during CI QEMU runs.

---

## Environment Variables and Overrides

These are understood by the QEMU test scripts and are passed via `env:` blocks in the workflow:

| Variable | Default | Purpose |
| --- | --- | --- |
| `GUEST_IMAGE` | `ubuntu-24.04` | Guest OS label or absolute image path |
| `UBUNTU_IMAGE` | — | Deprecated alias for `GUEST_IMAGE`; still honoured |
| `SSH_PORT` | 2222..2313 | Host-side port forwarded to guest SSH (per-bundle defaults) |
| `API_PORT` | 18443..19444 | Host-side port forwarded to guest API |
| `TIMEOUT` | 600 / 900 / 1200 | Seconds to wait for VM boot |
| `CI` | `1` | Enables `::group::` / `::error::` annotations in log output |
| `BITPROTECTOR_QEMU_SSH_KEY` | — | Public key text; set by the setup-qemu action (overrides `~/.ssh` fallback) |

---

## Extending the Pipeline

- **Add a new integration test binary**: declare it in `Cargo.toml` under `[[test]]`, add a `cargo test --test <name>` step to the `rust-integration-fast` job in `ci.yml`, and the matching call to `run_rust_integration_fast()` in `run-tests.sh`.
- **Add a new Playwright E2E spec**: create a new `*.spec.ts` file in `frontend/tests/e2e/`. It is automatically picked up by the `e2e` CI job — no changes to `ci.yml` are required.
- **Change the runner**: update `runs-on:` in the relevant job. For heavy QEMU, `ubuntu-latest-4-core` is the fallback if the default runner is too slow.
- **Adjust coverage behavior**: edit the `coverage` job in `ci.yml` (it is non-gating via `continue-on-error: true`).
- **E2E port conflicts on local runs**: the `e2e` layer in `run-tests.sh` uses SSH 2280 / API 18480 so it does not collide with the smoke layer (SSH 2222 / API 18443).
