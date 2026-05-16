## Plan: CI/Nightly Workflow Optimization

**TL;DR**: The implemented wins in this PR come from caching runner-side QEMU packages, eliminating the nightly duplicate `.deb` rebuild, parallelising lint+unit, faster tool installs, and pre-fetching the alpha `.deb` from a GitHub Release. Pre-baking the QEMU guest image was explored separately, but it is not part of the current implementation.

---

### Phase 1 — Pre-baked QEMU base image *(future optimization, not implemented here)*

Right now every QEMU job cloud-init still runs `apt-get update && apt-get install jq openssl curl` inside the VM on every boot. There are ~8 heavy QEMU job types × 2 guests = 16 VM boots per full CI run.

**Potential approach — `virt-customize` in `setup-qemu`:**

1. In `.github/actions/setup-qemu/action.yml`, after restoring the vanilla cloud image, run `sudo virt-customize -a image.img --install jq,openssl,curl --run-command 'apt-get clean' --run-command 'cloud-init clean --logs --seed'` to create a pre-baked base image
2. Cache the pre-baked image under a separate monthly key like `${{ inputs.guest }}-base-YYYYMM` (restore vanilla → customize → save as base)
3. Test jobs use the base image as the qcow2 backing file
4. Cloud-init `runcmd` in all bundle scripts (`smoke.sh`, `application_workflows.sh`, `resilience.sh`, `failover.sh`, `degraded_boot.sh`, `drive_media_type.sh`, `upgrade.sh`, `scale*.sh`, `scheduled_load.sh`) — remove `apt-get update` + `apt-get install jq openssl curl`, keep only `apt-get install /mnt/debpkg/bitprotector*.deb`

> **Note**: `cloud-init clean` inside the image ensures cloud-init reruns fresh for each test's own seed ISO, avoiding the "already ran for this instance-id" no-op. The `virt-customize` step requires `libguestfs-tools` on the runner (add it to the `Install QEMU packages` step in setup-qemu — ~30s, paid back many times over).

---

### Phase 2 — Cache runner-side QEMU apt packages *(~8 min/full-CI)*

All 16+ QEMU runners each install `qemu-system-x86 qemu-utils cloud-image-utils socat openssh-client` from scratch. Replace the raw `apt-get install` in `.github/actions/setup-qemu/action.yml` with `awalsh128/cache-apt-pkgs-action@v1`.

---

### Phase 3 — Eliminate nightly duplicate build *(~8–12 min/nightly)*

`.github/workflows/nightly.yml` calls `ci.yml` (which already builds `bitprotector-deb-ubuntu-*` artifacts) and then immediately rebuilds the identical `.deb` as `bitprotector-deb-nightly-ubuntu-*`. The `qemu-scale` / `qemu-scale-lowmem` / `qemu-scheduled-load` jobs only care about the binary, not the version string.

**Recommended fix**: Change those three jobs to download from `bitprotector-deb-ubuntu-*` (the CI artifacts) directly, and delete `build-artifacts-nightly` entirely.

---

### Phase 4 — Parallelize lint + unit *(~2–4 min off critical path)*

Currently `lint → unit → rust-integration-fast`. There's no real dependency between lint and unit.

1. Remove `needs: lint` from the `unit` job in `.github/workflows/ci.yml`
2. Change `rust-integration-fast` to `needs: [lint, unit]` — integration only starts when both pass

---

### Phase 5 — `cargo-binstall` for tool installs *(~1–3 min on Swatinem cache miss)*

`cargo install cargo-deb` and `cargo install cargo-llvm-cov` compile from source when the binary is absent. Replace with `cargo binstall` (downloads pre-built binaries in seconds).

- Add `cargo-bins/cargo-binstall@v1` to `.github/actions/setup-rust/action.yml` (or per-job)
- Swap `cargo install cargo-deb` → `cargo binstall cargo-deb --no-confirm` in `build-artifacts`
- Swap `cargo install cargo-llvm-cov` → `cargo binstall cargo-llvm-cov --no-confirm` in `coverage`

---

### Phase 6 — Pre-download alpha1 `.deb` for upgrade tests *(~10–15 min per upgrade run)*

The `qemu-upgrade` job runs `setup-rust + setup-frontend` and rebuilds `v1.0.0-alpha1` entirely from source. Since alpha1 is a stable tag its `.deb` can be a one-time release asset.

1. One-time: upload the alpha1 `.deb` as an asset on the `v1.0.0-alpha1` GitHub Release
2. Replace the `Build alpha1 .deb` step in the `qemu-upgrade` job with `gh release download v1.0.0-alpha1 --pattern '*.deb'`
3. Remove `setup-rust` and `setup-frontend` from `qemu-upgrade`

---

### Relevant files

- `.github/actions/setup-qemu/action.yml` — Phase 1 + Phase 2
- `.github/actions/setup-rust/action.yml` — Phase 5
- `.github/workflows/ci.yml` — Phase 4 (lint/unit parallelism), Phase 5 (tool installs), Phase 6 (upgrade job)
- `.github/workflows/nightly.yml` — Phase 3 (remove duplicate build)
- `tests/installation/bundles/*.sh` — Phase 1 (simplify cloud-init runcmd)

---

### Further Considerations

1. **Phase 4 design question**: currently lint gates unit intentionally (fail-cheap-first). Running them in parallel means a slow unit test run won't be cancelled if lint fails first — is that acceptable?
2. **Phase 1 cache size**: the pre-baked image will be slightly larger than the vanilla (~650 MB → ~800 MB). Still well within the 10 GB Actions cache limit.
3. **Phase 3 artifact scope**: cross-workflow artifact access from a reusable workflow call works in `download-artifact@v3+` if you specify `run-id`. Double-check that deleting `build-artifacts-nightly` doesn't break artifact naming expectations in other tooling.
