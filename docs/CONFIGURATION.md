# Configuration Reference

BitProtector reads `/etc/bitprotector/config.toml` at startup. Settings are resolved in this order:

1. CLI flag (highest priority)
2. Config file value
3. Hardcoded default

Use the global `--config` flag to specify a custom config file path:

```bash
bitprotector --config /path/to/custom.toml serve ...
```

The file is optional — a missing or unreadable config silently falls back to defaults.

---

## Table of Contents

- [Section: \[server\]](#section-server)
- [Section: \[database\]](#section-database)
- [Logging](#logging)
- [Scheduler](#scheduler)
- [Virtual Paths](#virtual-paths)
- [Generating a Self-Signed Certificate](#generating-a-self-signed-certificate)
- [CLI global flags](#cli-global-flags)
- [Example — complete serve invocation](#example--complete-serve-invocation)

---

## Section: [server]

Controls the HTTP listener, TLS, and authentication. All keys in this section are supported in the config file **and** as CLI flags to `bitprotector serve`.

| Key / CLI flag | Type | Default | Description |
| --- | --- | --- | --- |
| `host` / `--host` | string | `"0.0.0.0"` | IP address to bind. |
| `port` / `--port` | integer | `8443` | TCP port for the HTTPS API. |
| `rate_limit_rps` / `--rate-limit-rps` | integer | `100` | Maximum requests per second per IP address. |
| `jwt_secret` / `--jwt-secret` | string | `"change-me-in-production"` | **Must be changed before deploying.** Secret used to sign and verify JWT tokens. Use a randomly generated string of at least 32 characters. |
| `tls_cert` / `--tls-cert` | string | *(none)* | Path to the PEM-encoded TLS certificate (or full chain). |
| `tls_key` / `--tls-key` | string | *(none)* | Path to the PEM-encoded private key. |

Both files must be present before the service can start. See [Generating a Self-Signed Certificate](#generating-a-self-signed-certificate).

```toml
[server]
host          = "0.0.0.0"
port          = 8443
rate_limit_rps = 100
jwt_secret    = "replace-with-a-random-64-char-string"
tls_cert      = "/etc/bitprotector/tls/cert.pem"
tls_key       = "/etc/bitprotector/tls/key.pem"
```

> **Security note:** The JWT secret must be kept confidential. Anyone with this value can forge valid tokens for any user. Generate a strong secret with:
>
> ```bash
> openssl rand -hex 32
> ```

The JWT token lifetime is fixed at **86400 seconds (24 hours)** and is not configurable.

---

## Section: [database]

Supported in the config file and overridden by the global `--db` flag.

| Key / CLI flag | Type | Default | Description |
| --- | --- | --- | --- |
| `path` / `--db` | string | `"/var/lib/bitprotector/bitprotector.db"` | Absolute path to the SQLite database file. The directory must be writable by the service user. |

```toml
[database]
path = "/var/lib/bitprotector/bitprotector.db"
```

---

## Logging

Log level and output file are **not** currently read from the config file. Control them via environment variable:

| Environment variable | Default | Description |
| --- | --- | --- |
| `RUST_LOG` | `info` | Minimum log level. One of: `trace`, `debug`, `info`, `warn`, `error`. |

```bash
RUST_LOG=debug bitprotector serve ...
```

Structured output goes to the journal when running under systemd.

---

## Scheduler

Scheduled tasks (periodic sync and integrity checks) are managed via the API or CLI — not the config file.

Use `bitprotector scheduler` subcommands or `POST /api/v1/scheduler/schedules` to create and manage schedules. See [API.md](API.md) and the scheduler CLI help:

```bash
bitprotector scheduler --help
```

---

## Virtual Paths

BitProtector no longer uses a global virtual root such as `symlink_base`. Each file or folder virtual path is the exact absolute filesystem path where BitProtector will create a symlink.

Make sure the service user has permission to create parent directories and symlinks at whatever virtual paths you assign through the CLI, API, or web UI.

---

## Generating a Self-Signed Certificate

For development or an internal network where a certificate authority is not available:

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

For production, use a certificate from a trusted CA (e.g., Let's Encrypt via `certbot`) and update `tls_cert` and `tls_key` accordingly.

---

## CLI global flags

These flags are available to all subcommands and override the corresponding config file values.

| Flag | Default | Description |
| --- | --- | --- |
| `--db <path>` | `/var/lib/bitprotector/bitprotector.db` | Path to the SQLite database file. |
| `--config <path>` | `/etc/bitprotector/config.toml` | Path to the TOML configuration file. |

```bash
bitprotector --db /tmp/scratch.db drives list
bitprotector --config /etc/bitprotector/staging.toml serve ...
```

---

## Example — complete serve invocation

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

When deployed via the systemd unit, these flags are set in the `ExecStart` line of the service file at `/lib/systemd/system/bitprotector.service`. Alternatively, set them in `/etc/bitprotector/config.toml` so the service file only needs `bitprotector serve`.
