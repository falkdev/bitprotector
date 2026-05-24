# Building bitprotector

This document describes how to build the bitprotector Debian package locally. The only host prerequisite is **Docker**.

---

## Table of Contents

- [Host Prerequisites](#host-prerequisites)
- [Build Targets](#build-targets)
- [Building Locally](#building-locally)
- [Build Options](#build-options)
- [Docker Image Contents](#docker-image-contents)
- [CI Caching](#ci-caching)

---

## Host Prerequisites

| Requirement | Notes |
| --- | --- |
| Docker | Required to build and run the builder image |
| Git | Required for dev version computation from tags |

No Rust toolchain, Node.js, or `libpam0g-dev` is required on the host — everything is inside the Docker image.

---

## Build Targets

bitprotector produces a separate `.deb` for each Ubuntu target release:

| Target | Output version suffix | Example |
| --- | --- | --- |
| Ubuntu 24.04 (Noble) | `~24.04.1` | `bitprotector_1.0.0-0ubuntu1~24.04.1_amd64.deb` |
| Ubuntu 26.04 (Resolute) | `~26.04.1` | `bitprotector_1.0.0-0ubuntu1~26.04.1_amd64.deb` |

The Ubuntu version suffix ensures correct APT upgrade ordering: a package built for 24.04 will not accidentally upgrade a system running 26.04.

---

## Building Locally

```bash
# Ubuntu 24.04 target
./scripts/build-deb.sh --ubuntu-version 24.04

# Ubuntu 26.04 target
./scripts/build-deb.sh --ubuntu-version 26.04
```

The output `.deb` is written to `target/debian/` (via Docker bind mount).

On the first run, the script builds the `bitprotector-deb-builder:ubuntu-<VERSION>` image (~5–10 min). Subsequent runs reuse the cached image and are much faster.

---

## Build Options

| Flag | Description |
| --- | --- |
| `--ubuntu-version <ver>` | **Required.** Ubuntu version to target (`24.04` or `26.04`). |
| `--deb-version <ver>` | Override the Debian version string. Computed from git tags if omitted. |
| `--rebuild` | Force a rebuild of the Docker image even if it already exists locally. |

### Example: pin an explicit version

```bash
./scripts/build-deb.sh --ubuntu-version 24.04 --deb-version 1.2.3-0ubuntu1~24.04.1
```

### Example: force image rebuild after Dockerfile changes

```bash
./scripts/build-deb.sh --ubuntu-version 24.04 --rebuild
```

---

## Docker Image Contents

`docker/Dockerfile.deb-builder` is a single parametrized Dockerfile (`ARG UBUNTU_VERSION=24.04`):

| Layer | Why it is needed |
| --- | --- |
| `ubuntu:${UBUNTU_VERSION}` | Matches the target runtime ABI exactly |
| `build-essential`, `pkg-config` | Rust compile toolchain |
| `libpam0g-dev` | PAM headers required by `pam` crate |
| `libclang-dev`, `llvm` | Required by `bindgen` for native C bindings |
| `curl`, `git`, `ca-certificates` | Installation tooling for Rust and Node.js |
| NodeSource Node.js 24 | Matches the Node version used in CI and local dev |
| Rust stable (via `rustup`) | Compiles the Rust binary |
| `cargo-deb` | Packages the binary into a `.deb` |

The host's `~/.cargo/registry` and `~/.cargo/git` are bind-mounted into the container at `/tmp/.cargo/registry` and `/tmp/.cargo/git`. The container runs as the current host user (via `--user $(id -u):$(id -g)`) with `CARGO_HOME=/tmp/.cargo`, so all files written during the build remain owned by you — no `sudo` is needed for cleanup.

---

## CI Caching

In GitHub Actions, the `build-artifacts` and `build-and-verify` jobs use two caches:

1. **Docker layer cache** — Docker Buildx with `type=gha` cache backend, scoped per Ubuntu version (`deb-builder-24.04`, `deb-builder-26.04`). On cache hit, the image layers are restored in seconds.

2. **Cargo registry cache** — `actions/cache` on `~/.cargo/registry` and `~/.cargo/git`, keyed on `Cargo.lock`. This is the same directory bind-mounted into the container, so cached crates are available to `cargo deb` without a network fetch.

Both caches are populated on first use and refreshed whenever `Dockerfile.deb-builder` or `Cargo.lock` change.
