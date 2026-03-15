# Configuration Reference

The configuration file is a TOML document read at startup. The installed default lives at `/etc/bitprotector/config.toml`. Pass `--config <path>` on the command line to use a different file.

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

| Key | Type | Default | Description |
|---|---|---|---|
| `host` | string | `"127.0.0.1"` | IP address to bind. Use `"0.0.0.0"` to listen on all interfaces. |
| `port` | integer | `8443` | TCP port for the HTTPS API. |
| `rate_limit_rps` | integer | `100` | Maximum requests per second per IP address. |

```toml
[server]
host = "127.0.0.1"
port = 8443
rate_limit_rps = 100
```

---

## Section: [tls]

TLS certificate and private key for the HTTPS server. Both files must be present before the service can start.

| Key | Type | Default | Description |
|---|---|---|---|
| `cert` | string | `"/etc/bitprotector/tls/cert.pem"` | Path to the PEM-encoded TLS certificate (or full chain). |
| `key` | string | `"/etc/bitprotector/tls/key.pem"` | Path to the PEM-encoded private key. |

```toml
[tls]
cert = "/etc/bitprotector/tls/cert.pem"
key  = "/etc/bitprotector/tls/key.pem"
```

See [Generating a Self-Signed Certificate](#generating-a-self-signed-certificate) below.

---

## Section: [database]

| Key | Type | Default | Description |
|---|---|---|---|
| `path` | string | `"/var/lib/bitprotector/bitprotector.db"` | Absolute path to the SQLite database file. The directory must be writable by the service user. |

```toml
[database]
path = "/var/lib/bitprotector/bitprotector.db"
```

---

## Section: [auth]

Controls JWT token issuance. PAM is used for credential verification; no additional configuration is required for PAM itself.

| Key | Type | Default | Description |
|---|---|---|---|
| `jwt_secret` | string | `"change-me-in-production"` | **Must be changed before deploying.** Secret used to sign and verify JWT tokens. Use a randomly generated string of at least 32 characters. |
| `token_ttl` | integer | `28800` | Token lifetime in seconds. Default is 8 hours. |

```toml
[auth]
jwt_secret = "change-me-in-production"
token_ttl  = 28800
```

> **Security note:** `jwt_secret` must be kept confidential. Anyone with this value can forge valid tokens for any user. Generate a strong secret with:
> ```bash
> openssl rand -hex 32
> ```

---

## Section: [virtual_paths]

| Key | Type | Default | Description |
|---|---|---|---|
| `symlink_base` | string | `"/var/lib/bitprotector/virtual/"` | Directory where virtual-path symlinks are created. The service must have write access. The directory is created on first use. |

```toml
[virtual_paths]
symlink_base = "/var/lib/bitprotector/virtual/"
```

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

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `true` | Master switch for background scheduled tasks (sync and integrity check). Individual task schedules are managed via the API or CLI; this key suspends all of them at once without deleting their configuration. |

```toml
[scheduler]
enabled = true
```

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

The `--db` flag overrides `[database].path` at the command line without modifying the config file. This is primarily useful for testing or running ephemeral one-shot commands:

```bash
bitprotector --db /tmp/scratch.db drives list
# uses /tmp/scratch.db instead of the configured database path
```

---

## Example — complete file

```toml
# /etc/bitprotector/config.toml

[server]
host = "0.0.0.0"
port = 8443
rate_limit_rps = 100

[tls]
cert = "/etc/bitprotector/tls/cert.pem"
key  = "/etc/bitprotector/tls/key.pem"

[database]
path = "/var/lib/bitprotector/bitprotector.db"

[auth]
jwt_secret = "replace-with-a-random-64-char-string"
token_ttl  = 28800

[virtual_paths]
symlink_base = "/var/lib/bitprotector/virtual/"

[logging]
level = "info"
file  = "/var/log/bitprotector/bitprotector.log"

[scheduler]
enabled = true
```
