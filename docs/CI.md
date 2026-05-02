# CI Pipeline

This document explains the GitHub Actions pipeline, how each layer maps to jobs, how to reproduce a failure locally, and how to trigger the full suite manually.

---

## Table of Contents

- [Pipeline Overview](#pipeline-overview)
- [Trigger Matrix](#trigger-matrix)
- [Layer Reference](#layer-reference)
- [Local Debugging with `act`](#local-debugging-with-act)
- [Native Local Runs (no Docker)](#native-local-runs-no-docker)
- [Reproducing a Specific Failure](#reproducing-a-specific-failure)
- [Cached Artifacts](#cached-artifacts)
- [Environment Variables and Overrides](#environment-variables-and-overrides)
- [Extending the Pipeline](#extending-the-pipeline)

---

## Pipeline Overview

The pipeline is **layered and fail-fast**. Cheaper, faster tests gate the expensive QEMU layers, and independent jobs fan out in parallel where possible (for example, coverage runs alongside Rust integration after unit tests).

```
lint → unit → (coverage, non-gating) → rust-integration-fast → rust-integration-heavy
                                                           ↓
                                                     build-artifacts
                                                           ↓
                                                     qemu-smoke
                  ┌────────────────────────────┬───────────────┬──────────────┬──────────────┬───────────────────┐
                  ↓                            ↓               ↓              ↓              ↓                   ↓
           qemu-failover                qemu-uninstall   qemu-resilience  qemu-upgrade  qemu-degraded-boot  (nightly) qemu-scale + qemu-scale-lowmem
```

All workflow YAML lives in [.github/workflows/](.github/workflows/). QEMU jobs use composite actions in [.github/actions/](.github/actions/) for setup.

---

## Trigger Matrix

| Event | Layers run |
|---|---|
| `pull_request` | 0 – 11 (full CI workflow in `ci.yml`) |
| `push` to `main` | 0 – 11 |
| `workflow_dispatch` with `run_heavy_qemu=true` | 0 – 11 |
| `workflow_dispatch` with `run_heavy_qemu=false` (default) | 0 – 6 (`qemu-smoke` only for QEMU layer) |
| Nightly cron (03:00 UTC via `nightly.yml`) | Full CI (0 – 11) + nightly-only `qemu-scale` + `qemu-scale-lowmem` |

PR runs use `cancel-in-progress: true` so a new push automatically cancels the previous run. Pushes to `main` never cancel (a heavy failover run mid-flight must finish).

---

## Layer Reference

| # | Job name | Content | Runner | Expected time |
|---|---|---|---|---|
| 0 | `lint` | `cargo fmt --check`, `cargo clippy -D warnings`, `npm run lint`, prettier check | ubuntu-24.04 | < 2 min |
| 1 | `unit` | `cargo test --lib` + `npm test` (vitest/jsdom) | ubuntu-24.04 | 2-4 min |
| 2 | `coverage` | `cargo llvm-cov` + `npm run test:coverage` artifact upload (non-gating) | ubuntu-24.04 | 4-8 min |
| 3 | `rust-integration-fast` | CLI + split API integration binaries + core integration tests (except `scaling_100k`) | ubuntu-24.04 | 4-7 min |
| 4 | `rust-integration-heavy` | `cargo test --test scaling_100k` (100k rows, 3 s/query budgets) | ubuntu-24.04 | 2-4 min |
| 5 | `build-artifacts` | `npm ci && npm run build && cargo deb` → uploads `.deb` as artifact | ubuntu-24.04 | 4-6 min |
| 6 | `qemu-smoke` | Matrix: Ubuntu 24.04 + 26.04. Installs `.deb`, smoke scenarios including database backup repair/restore. | ubuntu-24.04 | 8-12 min per guest |
| 7 | `qemu-failover` | Matrix: Ubuntu 24.04 + 26.04. Failover scenarios + QMP hot-remove. | ubuntu-24.04 | 15-20 min per guest |
| 8 | `qemu-uninstall` | Matrix: Ubuntu 24.04 + 26.04. Purge/uninstall scenarios. | ubuntu-24.04 | 8-12 min per guest |
| 9 | `qemu-resilience` | Matrix: Ubuntu 24.04 + 26.04. ENOSPC/readonly/signal/restart scenarios. | ubuntu-24.04 | 15-25 min per guest |
| 10 | `qemu-upgrade` | Matrix: Ubuntu 24.04 + 26.04. alpha1 → current upgrade scenarios. | ubuntu-24.04 | 20-30 min per guest |
| 11 | `qemu-degraded-boot` | Matrix: Ubuntu 24.04 + 26.04. Degraded boot scenarios. | ubuntu-24.04 | 10-15 min per guest |

**Runner vs guest OS**: the runner is always `ubuntu-24.04` (GitHub-hosted). The *guest* running inside QEMU is controlled by the matrix (`ubuntu-24.04` noble, `ubuntu-26.04` plucky). See [.github/actions/setup-qemu/action.yml](.github/actions/setup-qemu/action.yml) for image download logic.

**Ubuntu 26.04 note**: until the 26.04 LTS image is fully published on `cloud-images.ubuntu.com`, the 26.04 matrix cell uses `continue-on-error: true`. Once the image is stable, remove that line from each QEMU job in [.github/workflows/ci.yml](.github/workflows/ci.yml).

**`scaling_100k` timing budget**: the test enforces a 3000 ms per-query budget ([tests/integration/scaling_100k.rs](../tests/integration/scaling_100k.rs)). On slow runners this may flake. If it does, bump the budget via the `SCALING_QUERY_BUDGET_MS` env var (if wired), or move the job to a larger runner by changing its `runs-on` label — one-line change.

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

## Native Local Runs (no Docker)

If you don't want the `act` overhead, run layers natively using the same layer subcommands:

```bash
./scripts/run-tests.sh lint
./scripts/run-tests.sh fast
./scripts/run-tests.sh smoke   # requires QEMU installed (run setup-qemu.sh first)
./scripts/run-tests.sh full
```

This script calls cargo/npm/bash directly — no containers. The layer definitions are shared with `ci-local.sh`.

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
   ```

4. **Inspect serial console output** — the scripts stream boot lines to your terminal. The raw log is also at `${WORKDIR}/serial.log` inside the QEMU working directory.

5. **Trigger the heavy suite manually from a PR branch** (without merging to main):
   - Go to Actions → CI → Run workflow → check `run_heavy_qemu` → Run.

---

## Cached Artifacts

### Rust build cache

`Swatinem/rust-cache@v2` caches `~/.cargo/registry`, `~/.cargo/git`, and the `target/` directory. Expect > 50% reduction in layer 2 time on cache hits.

### npm cache

`actions/setup-node@v4` caches `frontend/node_modules` keyed on `frontend/package-lock.json`. After the first run, `npm ci` time is near-zero.

### Cloud images

Each Ubuntu image (~650 MB) is cached under `~/images/` via `actions/cache@v4` with a per-guest monthly key (`ubuntu-24.04-YYYYMM`, `ubuntu-26.04-YYYYMM`). The cache refreshes automatically at the start of each month. If a critical CVE lands and you need a fresh image immediately, clear the cache manually via the Actions UI (Actions → Caches).

### .deb artifact

`build-artifacts` uploads `bitprotector_*.deb` as a GitHub Actions artifact named `bitprotector-deb` with 7-day retention. QEMU jobs in `ci.yml` download it — the binary is never rebuilt during CI QEMU runs.

---

## Environment Variables and Overrides

These are understood by the QEMU test scripts and are passed via `env:` blocks in the workflow:

| Variable | Default | Purpose |
|---|---|---|
| `GUEST_IMAGE` | `ubuntu-24.04` | Guest OS label or absolute image path |
| `UBUNTU_IMAGE` | — | Deprecated alias for `GUEST_IMAGE`; still honoured |
| `SSH_PORT` | 2222..2229 | Host-side port forwarded to guest SSH (per-bundle defaults) |
| `API_PORT` | 18443..18450 | Host-side port forwarded to guest API |
| `TIMEOUT` | 600 / 900 | Seconds to wait for VM boot |
| `CI` | `1` | Enables `::group::` / `::error::` annotations in log output |
| `BITPROTECTOR_QEMU_SSH_KEY` | — | Public key text; set by the setup-qemu action (overrides `~/.ssh` fallback) |

---

## Extending the Pipeline

- **Add a new integration test binary**: declare it in `Cargo.toml` under `[[test]]`, add a `cargo test --test <name>` step to the `rust-integration-fast` job in `ci.yml`, and the matching call to `run_rust_integration_fast()` in `run-tests.sh`.
- **Change the runner**: update `runs-on:` in the relevant job. For heavy QEMU, `ubuntu-latest-4-core` is the fallback if the default runner is too slow.
- **Promote 26.04 to required**: remove `continue-on-error: ${{ matrix.guest == 'ubuntu-26.04' }}` from each QEMU job once the image is stable.
- **Adjust coverage behavior**: edit the `coverage` job in `ci.yml` (it is non-gating via `continue-on-error: true`).
