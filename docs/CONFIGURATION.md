# Configuration Reference

BitProtector is configured via command-line flags passed to `bitprotector serve`. The file `/etc/bitprotector/config.toml` is installed as a reference template but **is not currently read by the binary** — all settings must be passed as CLI flags.

```bash
bitprotector serve \
  --host 0.0.0.0 \
  --port 8443 \
  --jwt-secret "$(openssl rand -hex 32)" \
  --tls-cert /etc/bitprotector/tls/cert.pem \
  --tls-key  /etc/bitprotector/tls/key.pem \
  --rate-limit-rps 100
```

The database path is a global flag available to all subcommands:

```bash
bitprotector --db /var/lib/bitprotector/bitprotector.db serve ...
```

---

## Table of Contents

- [Section: \[server\]](#section-server)
- [Section: \[tls\]](#section-tls)
- [Section: \[database\]](#section-database)
- [Section: \[auth\]](#section-auth)
- [Section: \[virtual\_paths\]](#section-virtual_paths)
- [Section: \[logging\]](#section-logging)
- [Section: \[scheduler\]](#section-scheduler)
- [Generating a Self-Signed Certificate](#generating-a-self-signed-certificate)
- [CLI --db flag](#cli---db-flag)
- [Example — complete file](#example--complete-file)

---

## Section: [server]

Controls the HTTP listener.

| Key / CLI flag | Type | Default | Description |
|---|---|---|---|
| `host` / `--host` | string | `"0.0.0.0"` | IP address to bind. |
| `port` / `--port` | integer | `8443` | TCP port for the HTTPS API. |
| `rate_limit_rps` / `--rate-limit-rps` | integer | `100` | Maximum requests per second per IP address. |

```bash
bitprotector serve --host 0.0.0.0 --port 8443 --rate-limit-rps 100
```

---

## Section: [tls]

TLS certificate and private key for the HTTPS server. Both files must be present before the service can start.

| Key / CLI flag | Type | Default | Description |
|---|---|---|---|
| `tls_cert` / `--tls-cert` | string | *(none)* | Path to the PEM-encoded TLS certificate (or full chain). |
| `tls_key` / `--tls-key` | string | *(none)* | Path to the PEM-encoded private key. |

```bash
bitprotector serve --tls-cert /etc/bitprotector/tls/cert.pem \
                   --tls-key  /etc/bitprotector/tls/key.pem
```

See [Generating a Self-Signed Certificate](#generating-a-self-signed-certificate) below.

---

## Section: [database]

| Key / CLI flag | Type | Default | Description |
|---|---|---|---|
| `path` / `--db` | string | `"/var/lib/bitprotector/bitprotector.db"` | Absolute path to the SQLite database file. The directory must be writable by the service user. |

```bash
bitprotector --db /var/lib/bitprotector/bitprotector.db serve ...
```

---

## Section: [auth]

PAM is used for credential verification; no additional configuration is required for PAM itself.

| Key / CLI flag | Type | Default | Description |
|---|---|---|---|
| `jwt_secret` / `--jwt-secret` | string | `"change-me-in-production"` | **Must be changed before deploying.** Secret used to sign and verify JWT tokens. Use a randomly generated string of at least 32 characters. |

The JWT token lifetime is fixed at **86400 seconds (24 hours)** and is not configurable.

```bash
bitprotector serve --jwt-secret "$(openssl rand -hex 32)"
```

> **Security note:** The JWT secret must be kept confidential. Anyone with this value can forge valid tokens for any user. Generate a strong secret with:
> ```bash
> openssl rand -hex 32
> ```

---

## Section: [virtual_paths]

| Setting | Type | Default | Description |
|---|---|---|---|
| `BITPROTECTOR_SYMLINK_BASE` env var | string | `"/var/lib/bitprotector/virtual"` | Directory where virtual-path symlinks are created. The service must have write access. The directory is created on first use. |

```bash
export BITPROTECTOR_SYMLINK_BASE=/var/lib/bitprotector/virtual
```

This can also be overridden per-request via the optional `symlink_base` field in API calls and the `--symlink-base` flag in CLI commands.

---

## Section: [logging]

| Key | Type | Default | Description |
|---|---|---|---|
| `level` | string | `"info"` | Minimum log level for `tracing` output. One of: `trace`, `debug`, `info`, `warn`, `error`. |
| `file` | string | `"/var/log/bitprotector/bitprotector.log"` | Path to the log file. The directory must be writable by the service user. |

```toml
[logging]
level = "info"
file  = "/var/log/bitprotector/bitprotector.log"
```

The `RUST_LOG` environment variable overrides `level` if set.

---

## Section: [scheduler]

The scheduler API is not yet implemented. Background tasks can be run on demand via the CLI (`bitprotector sync process`) or the API (`POST /sync/process`, `POST /sync/run/{task}`). The `schedule_config` table is reserved for future scheduled-task management.

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

For production, use a certificate from a trusted CA (e.g., Let's Encrypt via `certbot`) and update `cert` and `key` paths accordingly.

---

## CLI --db flag

The `--db` flag is a global flag available to all subcommands. It overrides the default database path without modifying any config file. This is primarily useful for testing or running ephemeral one-shot commands:

```bash
bitprotector --db /tmp/scratch.db drives list
# uses /tmp/scratch.db instead of the configured database path
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

When deployed via the systemd unit, these flags are set in the `ExecStart` line of the service file at `/lib/systemd/system/bitprotector.service`.
