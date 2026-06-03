# Configuration Reference

This document is the single source of truth for configuring BitProtector.
It covers how settings are loaded and resolved, every supported config key,
and common setup patterns for development and production.

## How configuration works

BitProtector reads `/etc/bitprotector/config.toml` at startup and resolves
each setting in the following order of priority:

```text
CLI flag  >  config file value  >  hardcoded default
```

This means you can always override any file setting with a CLI flag on the
command line, which is handy for quick experiments or CI scripts.

> **Config file is optional.** A missing or unreadable file is silently
> ignored — BitProtector falls back to hardcoded defaults and any CLI flags
> you provide.

Use the global `--config` flag to point at a non-default file:

```bash
bitprotector --config /etc/bitprotector/staging.toml serve
```

### Which sections are actually parsed?

Only three sections in the config file are read by the application:
`[server]`, `[database]`, and `[checksum]`.

The `[logging]` and `[scheduler]` sections that appear in
`config/default.toml` are **silently ignored** — see [Logging](#logging)
and [Scheduler](#scheduler) below for how to control those features.

---

## Table of Contents

- [Quick-start configurations](#quick-start-configurations)
- [Section: \[server\]](#section-server)
- [Section: \[database\]](#section-database)
- [Section: \[checksum\]](#section-checksum)
- [Logging](#logging)
- [Scheduler](#scheduler)
- [Virtual paths](#virtual-paths)
- [Generating a self-signed certificate](#generating-a-self-signed-certificate)
- [CLI global flags](#cli-global-flags)
- [Full serve invocation example](#full-serve-invocation-example)

---

## Quick-start configurations

### Minimal development setup (no TLS)

When both `tls_cert` and `tls_key` are omitted the server starts on plain
HTTP. This is useful for local development or testing behind a TLS-terminating
reverse proxy.

```bash
bitprotector serve \
  --host 127.0.0.1 \
  --port 8080 \
  --jwt-secret "dev-only-secret"
```

### Production setup via config file

Create `/etc/bitprotector/config.toml`, then just run `bitprotector serve`.

```toml
[server]
host          = "0.0.0.0"
port          = 8443
rate_limit_rps = 100
jwt_secret    = "replace-with-a-random-64-char-string"
tls_cert      = "/etc/bitprotector/tls/cert.pem"
tls_key       = "/etc/bitprotector/tls/key.pem"

[database]
path = "/var/lib/bitprotector/bitprotector.db"
```

Generate a strong JWT secret with:

```bash
openssl rand -hex 32
```

---

## Section: [server]

Controls the HTTP listener, TLS, and authentication. Every key in this section
can also be passed as a CLI flag to `bitprotector serve`.

| Key / CLI flag | Type | Default | Description |
| --- | --- | --- | --- |
| `host` / `--host` | string | `"0.0.0.0"` | IP address to bind. The Debian package default config sets `"127.0.0.1"` (loopback-only); `"0.0.0.0"` is the hardcoded fallback used when no config file is present. |
| `port` / `--port` | integer | `8443` | TCP port for the API. |
| `rate_limit_rps` / `--rate-limit-rps` | integer | `100` | Maximum requests per second per IP address (sliding 1-second window). |
| `jwt_secret` / `--jwt-secret` | string | `"change-me-in-production"` | Secret used to sign and verify JWT tokens. **Must be changed before deploying.** |
| `tls_cert` / `--tls-cert` | string | *(none)* | Path to the PEM-encoded TLS certificate (or full chain). |
| `tls_key` / `--tls-key` | string | *(none)* | Path to the PEM-encoded private key. |

**TLS is optional.** When both `tls_cert` and `tls_key` are provided the server
uses HTTPS; if either is absent the server starts on plain HTTP. For production
you should always enable TLS — see
[Generating a self-signed certificate](#generating-a-self-signed-certificate).

> **Security note:** Anyone who knows the JWT secret can forge tokens for any
> user. Keep this value confidential and rotate it if it is ever exposed.

The JWT token lifetime is fixed at **86 400 seconds (24 hours)** and is not
configurable.

---

## Section: [database]

| Key / CLI flag | Type | Default | Description |
| --- | --- | --- | --- |
| `path` / `--db` | string | `"/var/lib/bitprotector/bitprotector.db"` | Absolute path to the SQLite database file. The directory must be writable by the service user. |

The `--db` flag is global (available to every subcommand, not just `serve`):

```bash
# Use a separate database for testing
bitprotector --db /tmp/test.db drives list
```

```toml
[database]
path = "/var/lib/bitprotector/bitprotector.db"
```

---

## Section: [checksum]

Tunes parallelism during integrity checks. These keys are **config-file only** —
there are no equivalent CLI flags.

The defaults work well for most hardware. Adjust them only if you observe I/O
contention or want to cap resource usage.

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `hdd_max_parallel` | integer | `2` | Maximum files checked simultaneously on drive pairs that include at least one HDD. High parallelism hurts throughput on spinning disks because of seek latency; keep this low. |
| `ssd_max_parallel` | integer | `0` | Maximum files checked simultaneously on SSD-only drive pairs. `0` = auto: `num_logical_cpus / 2`, minimum 2. |

```toml
[checksum]
hdd_max_parallel = 2
ssd_max_parallel = 0   # 0 = auto (num_logical_cpus / 2, min 2)
```

---

## Logging

Log level is controlled by an environment variable, **not** the config file.

| Variable | Default | Values |
| --- | --- | --- |
| `RUST_LOG` | `info` | `trace`, `debug`, `info`, `warn`, `error` |

```bash
RUST_LOG=debug bitprotector serve
```

Under systemd, output is forwarded to the journal automatically.

> **Note:** The `[logging]` section (`level`, `file`) in `config/default.toml`
> is **silently ignored** at runtime. Only `RUST_LOG` is effective.

---

## Scheduler

Scheduled tasks (periodic sync and integrity checks) are **not** configured
in the config file. Manage them through the API or CLI:

```bash
bitprotector scheduler --help
POST /api/v1/scheduler/schedules
```

See [API.md](API.md) for the full scheduler API reference.

> **Note:** The `[scheduler]` section (`enabled`, `sync_interval_seconds`,
> `integrity_interval_seconds`) in `config/default.toml` is **silently ignored**
> at runtime.

**Database backup schedules** are separate. Configure them through the web UI
(Database Backups page), `bitprotector database settings`, or
`/api/v1/database/backups/settings`. Automatic backups and integrity checks are
disabled by default; in the web UI the manual *Run Backup Now* and
*Check Integrity Now* actions require at least one backup destination to be
enabled.

---

## Virtual paths

Each virtual path is the exact absolute filesystem path where BitProtector
will create a symlink. There is no global virtual root — every file and folder
gets its own explicit path.

The service is granted `CAP_DAC_OVERRIDE` so it can create symlinks at any
absolute path without requiring you to manually set permissions on the parent
directories.

---

## Generating a self-signed certificate

Use this for development or an internal network where a CA is not available.
For production prefer a trusted CA certificate (e.g. via Let's Encrypt /
`certbot`).

```bash
sudo mkdir -p /etc/bitprotector/tls

sudo openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 \
  -nodes \
  -keyout /etc/bitprotector/tls/key.pem \
  -out    /etc/bitprotector/tls/cert.pem \
  -subj   "/CN=bitprotector" \
  -addext "subjectAltName=IP:127.0.0.1,DNS:localhost"

sudo chmod 600 /etc/bitprotector/tls/key.pem
```

Then set `tls_cert` and `tls_key` in your config file or pass them as CLI flags.

---

## CLI global flags

These flags are accepted by every subcommand and take precedence over the
config file.

| Flag | Default | Description |
| --- | --- | --- |
| `--db <path>` | `/var/lib/bitprotector/bitprotector.db` | Path to the SQLite database file. |
| `--config <path>` | `/etc/bitprotector/config.toml` | Path to the TOML configuration file. |

```bash
bitprotector --db /tmp/scratch.db drives list
bitprotector --config /etc/bitprotector/staging.toml serve
```

---

## Full serve invocation example

CLI flags for every setting — useful when you do not want a config file (e.g.
in a container or CI environment):

```bash
bitprotector \
  --db /var/lib/bitprotector/bitprotector.db \
  serve \
  --host 0.0.0.0 \
  --port 8443 \
  --jwt-secret "replace-with-a-random-64-char-string" \
  --tls-cert /etc/bitprotector/tls/cert.pem \
  --tls-key  /etc/bitprotector/tls/key.pem \
  --rate-limit-rps 100
```

When deployed via the systemd unit, these flags live in the `ExecStart` line of
`/lib/systemd/system/bitprotector.service`. Alternatively, put everything in
`/etc/bitprotector/config.toml` so the service file only needs
`bitprotector serve`.
