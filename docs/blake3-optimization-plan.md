# BLAKE3 Checksum Optimization — Implementation Plan

> **Alpha-stage feature.** No DB migrations needed — the app recreates the DB fresh from the schema.
> **Scope:** drive media-type flag, RAM-aware mmap threshold, per-pair parallelism limits, active-worker visibility in CLI + frontend + API, elimination of mirror double-read.

---

## Background

BLAKE3 supports three parallelism layers:
1. **SIMD (intra-thread)** — AVX2/AVX-512/NEON, active by default, no code change needed.
2. **`rayon` multi-thread** — splits data across threads; enabled via `blake3 = { features = ["rayon"] }`. Use `Hasher::update_mmap_rayon(&file)` after mapping the file.
3. **`mmap`** — zero-copy via OS page cache; enabled via `blake3 = { features = ["mmap"] }`.

On HDDs, disk seek is the bottleneck; multi-threading multiple files adds contention. On SSDs, CPU can be the bottleneck for large files, so `mmap_rayon` wins. The application therefore:

- Keeps a per-role `ssd|hdd` flag on every drive pair.
- Computes a per-process mmap threshold from system RAM (`total_RAM / 2`).
- Builds a per-pair `rayon::ThreadPool` sized by the pair's media types.
- Serialises processing across drive pairs (avoids cross-pair HDD seek contention).

---

## Scope exclusions

- No DB migration framework (alpha stage).
- QEMU upgrade test (`bundles/upgrade.sh`) is **not** updated — schema changes break the alpha1→current path by design.
- No per-file media-type override (media type is a drive-level property).
- No configurable mmap threshold in TOML (the `[checksum]` section only exposes parallelism limits).

---

## Files to change or create

### Rust

| File | Action |
|---|---|
| `Cargo.toml` | Add `rayon`, `num_cpus`; update `blake3` features |
| `src/db/schema.rs` | Add two columns to `drive_pairs`, one to `integrity_runs` |
| `src/db/repository.rs` | Add fields to structs; update CRUD functions; add `set_integrity_run_active_workers` |
| `src/core/drive.rs` | Add `DriveMediaType` enum; add `media_type_for_role()` method on `DrivePair` |
| `src/core/system.rs` | **New file** — RAM detection, mmap threshold, default SSD parallelism |
| `src/core/mod.rs` | Expose `system` module |
| `src/core/checksum.rs` | Add `ChecksumStrategy`; update `checksum_file`; dual-hash `copy_with_checksum`; add `pool_size_for_pair` |
| `src/core/integrity.rs` | `check_file_integrity` takes strategies; use `rayon::join` |
| `src/core/integrity_runs.rs` | Serial pair loop; per-pair `ThreadPool`; `active_workers` updates; serial post-batch DB writes |
| `src/core/mirror.rs` | Consume tuple from `copy_with_checksum`; remove `checksum_file(&dst)` call |
| `src/main.rs` | Add `ChecksumConfig` to `AppConfig`; pass to `run_server` |
| `src/api/server.rs` | Accept `ChecksumConfig`; store as `web::Data`; thread to `process_run` |
| `src/cli/commands/drives.rs` | Add `--primary-media-type`, `--secondary-media-type` to `Add` and `Update` |
| `src/cli/commands/integrity.rs` | Show `Parallelism used:` in `check_all` completion output |
| `src/api/routes/drives.rs` | Add media-type fields to request structs; validate; pass to repository |

### Frontend

| File | Action |
|---|---|
| `frontend/src/types/drive.ts` | Add `primary_media_type`, `secondary_media_type` to `DrivePair` and request types |
| `frontend/src/types/integrity.ts` | Add `active_workers: number` to `IntegrityRun` |
| `frontend/src/components/drives/DriveForm.tsx` | Add media-type `<select>` fields |
| `frontend/src/components/drives/DriveCard.tsx` | Add HDD/SSD badges |
| `frontend/src/pages/IntegrityPage.tsx` | Add "Active Workers" summary card |

### Config

| File | Action |
|---|---|
| `config/default.toml` | Add `[checksum]` section |
| `packaging/config.toml` | Add `[checksum]` section |

### Tests

| File | Action |
|---|---|
| `tests/integration/api_drives.rs` | Update existing assertions; add new test cases |
| `tests/integration/cli_drives.rs` | Update existing assertions; add new test cases |
| `tests/integration/core_mirror.rs` | Update callers of `copy_with_checksum` (now returns tuple) |
| `tests/integration/core_checksum_strategy.rs` | **New file** |
| `frontend/src/pages/IntegrityPage.test.tsx` | Add `active_workers` to mock `IntegrityRun`; add worker card test |
| (vitest drive tests) | Add `primary_media_type: 'hdd'` to all mock `DrivePair` objects |

### CI/CD

| File | Action |
|---|---|
| `.github/workflows/ci.yml` | Add `core_checksum_strategy` step; add optional `qemu-drive-media-type` job |
| `.github/workflows/nightly.yml` | Add `qemu-drive-media-type` job |
| `tests/installation/scenarios/smoke/smoke-13-drive-media-type.sh` | **New file** |
| `tests/installation/scenarios/smoke/smoke-14-parallel-integrity-progress.sh` | **New file** |
| `tests/installation/bundles/drive_media_type.sh` | **New file** |

---

## Phase A — Database schema

### `src/db/schema.rs`

In `initialize_schema`, find the `CREATE TABLE IF NOT EXISTS drive_pairs` statement and add two columns before `created_at`:

```sql
primary_media_type  TEXT NOT NULL DEFAULT 'hdd' CHECK(primary_media_type IN ('hdd','ssd')),
secondary_media_type TEXT NOT NULL DEFAULT 'hdd' CHECK(secondary_media_type IN ('hdd','ssd')),
```

In the same function, find the `CREATE TABLE IF NOT EXISTS integrity_runs` statement and add one column before `started_at`:

```sql
active_workers      INTEGER NOT NULL DEFAULT 0,
```

The existing "idempotent migrations" block at the bottom of `initialize_schema` already handles old DBs with `ALTER TABLE … ADD COLUMN` calls wrapped in `let _ = conn.execute(…)`. Since we are alpha-stage and recreating DBs fresh, **no new ALTER TABLE entries are needed**. Just edit the `CREATE TABLE IF NOT EXISTS` statements directly.

---

## Phase B — Core: drive type, RAM, config

### `Cargo.toml`

Replace:
```toml
blake3 = "1"
```
With:
```toml
blake3 = { version = "1", features = ["rayon", "mmap"] }
rayon = "1"
num_cpus = "1"
```

---

### `src/core/system.rs` (new file)

```rust
use std::sync::OnceLock;

static TOTAL_RAM: OnceLock<u64> = OnceLock::new();

/// Read MemTotal from /proc/meminfo. Returns 2 GiB on any error.
pub fn total_ram_bytes() -> u64 {
    *TOTAL_RAM.get_or_init(|| {
        parse_proc_meminfo().unwrap_or(2 * 1024 * 1024 * 1024)
    })
}

fn parse_proc_meminfo() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// Files up to this size (bytes) may be hashed with mmap+rayon on SSD drives.
/// Set to 50% of total RAM so the mapping doesn't evict working set.
pub fn mmap_threshold_bytes() -> u64 {
    total_ram_bytes() / 2
}

/// Default number of parallel files for SSD drive pairs: half the logical CPU count, min 2.
pub fn default_ssd_parallel() -> usize {
    (num_cpus::get() / 2).max(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_ram_positive() {
        assert!(total_ram_bytes() > 0);
    }

    #[test]
    fn test_mmap_threshold_lte_total_ram() {
        assert!(mmap_threshold_bytes() <= total_ram_bytes());
    }

    #[test]
    fn test_parse_proc_meminfo_sample() {
        // 8 GiB = 8388608 KiB
        let sample = "MemTotal:       8388608 kB\nMemFree:        1234 kB\n";
        let result: Option<u64> = {
            let mut found = None;
            for line in sample.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    let kb: u64 = rest.split_whitespace().next().unwrap().parse().unwrap();
                    found = Some(kb * 1024);
                    break;
                }
            }
            found
        };
        assert_eq!(result, Some(8 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_default_ssd_parallel_gte_2() {
        assert!(default_ssd_parallel() >= 2);
    }
}
```

---

### `src/core/mod.rs`

Add `pub mod system;` to the existing list of module declarations.

---

### `src/core/drive.rs`

Add after the `DriveState` enum and its `impl` block:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveMediaType {
    Hdd,
    Ssd,
}

impl DriveMediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            DriveMediaType::Hdd => "hdd",
            DriveMediaType::Ssd => "ssd",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(value: &str) -> DriveMediaType {
        match value {
            "ssd" => DriveMediaType::Ssd,
            _ => DriveMediaType::Hdd,
        }
    }
}
```

Add the following method inside the existing `impl DrivePair` block (after `standby_accepts_sync`):

```rust
pub fn media_type_for_role(&self, role: DriveRole) -> DriveMediaType {
    match role {
        DriveRole::Primary => DriveMediaType::from_str(&self.primary_media_type),
        DriveRole::Secondary => DriveMediaType::from_str(&self.secondary_media_type),
    }
}
```

---

### `src/db/repository.rs`

**Update `DrivePair` struct** — add two fields after `active_role`:

```rust
pub primary_media_type: String,
pub secondary_media_type: String,
```

**Update `IntegrityRun` struct** — add one field after `recovered_files`:

```rust
pub active_workers: i64,
```

**Update `create_drive_pair`** — change signature and INSERT:

```rust
pub fn create_drive_pair(
    &self,
    name: &str,
    primary: &str,
    secondary: &str,
    primary_media_type: &str,
    secondary_media_type: &str,
) -> anyhow::Result<DrivePair> {
    let id = {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO drive_pairs (
                name, primary_path, secondary_path,
                primary_state, secondary_state, active_role,
                primary_media_type, secondary_media_type
             ) VALUES (?1, ?2, ?3, 'active', 'active', 'primary', ?4, ?5)",
            rusqlite::params![name, primary, secondary,
                              primary_media_type, secondary_media_type],
        )?;
        conn.last_insert_rowid()
    };
    self.get_drive_pair(id)
}
```

**Update `update_drive_pair`** — add two optional parameters:

```rust
pub fn update_drive_pair(
    &self,
    id: i64,
    name: Option<&str>,
    primary: Option<&str>,
    secondary: Option<&str>,
    primary_media_type: Option<&str>,
    secondary_media_type: Option<&str>,
) -> anyhow::Result<DrivePair> {
    {
        let conn = self.conn()?;
        if let Some(n) = name {
            conn.execute(
                "UPDATE drive_pairs SET name=?1, updated_at=datetime('now') WHERE id=?2",
                rusqlite::params![n, id],
            )?;
        }
        if let Some(p) = primary {
            conn.execute(
                "UPDATE drive_pairs SET primary_path=?1, updated_at=datetime('now') WHERE id=?2",
                rusqlite::params![p, id],
            )?;
        }
        if let Some(s) = secondary {
            conn.execute(
                "UPDATE drive_pairs SET secondary_path=?1, updated_at=datetime('now') WHERE id=?2",
                rusqlite::params![s, id],
            )?;
        }
        if let Some(pmt) = primary_media_type {
            conn.execute(
                "UPDATE drive_pairs SET primary_media_type=?1, updated_at=datetime('now') WHERE id=?2",
                rusqlite::params![pmt, id],
            )?;
        }
        if let Some(smt) = secondary_media_type {
            conn.execute(
                "UPDATE drive_pairs SET secondary_media_type=?1, updated_at=datetime('now') WHERE id=?2",
                rusqlite::params![smt, id],
            )?;
        }
    }
    self.get_drive_pair(id)
}
```

**Update `get_drive_pair` and all other functions that SELECT `drive_pairs`** — the SELECT queries must include `primary_media_type, secondary_media_type` and the row mapper must populate both new fields. Every function that builds a `DrivePair` from a row must be updated consistently.

**Update `get_integrity_run` and all functions that SELECT `integrity_runs`** — the SELECT must include `active_workers` and the row mapper must populate it.

**Add `set_integrity_run_active_workers`** — new function (does not exist yet):

```rust
pub fn set_integrity_run_active_workers(&self, run_id: i64, count: i64) -> anyhow::Result<()> {
    let conn = self.conn()?;
    conn.execute(
        "UPDATE integrity_runs SET active_workers=?1 WHERE id=?2",
        rusqlite::params![count, run_id],
    )?;
    Ok(())
}
```

**All existing `create_drive_pair` callers** (in tests and in CLI/API) pass only `(name, primary, secondary)` today. They must all be updated to pass `"hdd", "hdd"` as the new last two arguments unless the call site already has media-type information.

**All existing `update_drive_pair` callers** pass `(id, name, primary, secondary)`. They must be updated to pass two additional `None` values unless they know the media types.

---

## Phase C — Checksum strategy

### `src/core/checksum.rs`

**Add at the top (after existing imports):**

```rust
use crate::core::drive::DriveMediaType;
use crate::core::system;

/// How a single file should be hashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumStrategy {
    /// 64 KiB streaming loop + fadvise_dontneed (safe for all hardware).
    Streaming,
    /// Memory-map the entire file and hash all chunks with rayon (SSD + file fits in RAM/2).
    MmapRayon,
}

impl ChecksumStrategy {
    /// Choose the best strategy for a file on a drive of the given media type.
    pub fn for_drive(media_type: DriveMediaType, file_size: u64) -> ChecksumStrategy {
        if media_type == DriveMediaType::Ssd && file_size <= system::mmap_threshold_bytes() {
            ChecksumStrategy::MmapRayon
        } else {
            ChecksumStrategy::Streaming
        }
    }
}
```

**Replace `checksum_file` signature and body:**

```rust
/// Compute BLAKE3 checksum of a file, choosing strategy based on drive type and file size.
pub fn checksum_file<P: AsRef<Path>>(path: P, strategy: ChecksumStrategy) -> io::Result<String> {
    let file = File::open(path.as_ref())?;
    match strategy {
        ChecksumStrategy::MmapRayon => {
            let mut hasher = blake3::Hasher::new();
            hasher
                .update_mmap_rayon(path.as_ref())
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            Ok(hasher.finalize().to_hex().to_string())
        }
        ChecksumStrategy::Streaming => {
            let mut file = file;
            let mut hasher = blake3::Hasher::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
            #[cfg(unix)]
            fadvise_dontneed(&file);
            Ok(hasher.finalize().to_hex().to_string())
        }
    }
}
```

Note: `update_mmap_rayon` takes a `&Path`, so pass `path.as_ref()` not `&file`.

**Replace `copy_with_checksum` to return `(String, String)`:**

```rust
/// Copy src to dst while computing BLAKE3 checksums of both in a single pass.
/// Returns `(src_checksum, dst_checksum)`.
pub fn copy_with_checksum<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dst: Q,
) -> io::Result<(String, String)> {
    let mut src_file = File::open(src)?;
    let mut dst_file = File::create(dst)?;
    let mut src_hasher = blake3::Hasher::new();
    let mut dst_hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = src_file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        src_hasher.update(&buf[..n]);
        dst_file.write_all(&buf[..n])?;
        dst_hasher.update(&buf[..n]);
    }
    dst_file.flush()?;
    #[cfg(unix)]
    fadvise_dontneed(&src_file);
    Ok((
        src_hasher.finalize().to_hex().to_string(),
        dst_hasher.finalize().to_hex().to_string(),
    ))
}
```

**Add `ChecksumConfig` struct** (also in `src/core/checksum.rs`):

```rust
/// Parallelism limits read from the [checksum] config section.
#[derive(Debug, Clone)]
pub struct ChecksumConfig {
    /// Max simultaneous files for HDD pairs. Default 2.
    pub hdd_max_parallel: usize,
    /// Max simultaneous files for SSD pairs. 0 = auto (num_cpus/2, min 2).
    pub ssd_max_parallel: usize,
    /// Resolved ssd_max_parallel (never 0 after startup).
    pub resolved_ssd_parallel: usize,
}

impl Default for ChecksumConfig {
    fn default() -> Self {
        let resolved = system::default_ssd_parallel();
        ChecksumConfig {
            hdd_max_parallel: 2,
            ssd_max_parallel: 0,
            resolved_ssd_parallel: resolved,
        }
    }
}

impl ChecksumConfig {
    pub fn resolve(hdd_max_parallel: usize, ssd_max_parallel: usize) -> Self {
        let resolved = if ssd_max_parallel == 0 {
            system::default_ssd_parallel()
        } else {
            ssd_max_parallel
        };
        ChecksumConfig {
            hdd_max_parallel,
            ssd_max_parallel,
            resolved_ssd_parallel: resolved,
        }
    }
}
```

**Add `pool_size_for_pair` function:**

```rust
/// Number of files to process in parallel for a given drive pair and config.
/// Mixed pairs are limited by the HDD side to avoid seek contention.
pub fn pool_size_for_pair(
    primary_media: DriveMediaType,
    secondary_media: DriveMediaType,
    cfg: &ChecksumConfig,
) -> usize {
    match (primary_media, secondary_media) {
        (DriveMediaType::Ssd, DriveMediaType::Ssd) => cfg.resolved_ssd_parallel,
        _ => cfg.hdd_max_parallel,
    }
}
```

**Update `verify_file`** — update the `checksum_file` call to pass `ChecksumStrategy::Streaming`:

```rust
pub fn verify_file<P: AsRef<Path>>(path: P, expected: &str) -> io::Result<bool> {
    let actual = checksum_file(path, ChecksumStrategy::Streaming)?;
    Ok(actual == expected)
}
```

**Update unit tests in `checksum.rs`** — add strategy tests:

```rust
#[test]
fn test_strategy_hdd_always_streaming() {
    assert_eq!(
        ChecksumStrategy::for_drive(DriveMediaType::Hdd, 0),
        ChecksumStrategy::Streaming
    );
    assert_eq!(
        ChecksumStrategy::for_drive(DriveMediaType::Hdd, u64::MAX),
        ChecksumStrategy::Streaming
    );
}

#[test]
fn test_strategy_ssd_small_file_mmap() {
    // A 1-byte file is always <= threshold
    assert_eq!(
        ChecksumStrategy::for_drive(DriveMediaType::Ssd, 1),
        ChecksumStrategy::MmapRayon
    );
}

#[test]
fn test_strategy_ssd_huge_file_streaming() {
    // u64::MAX is always > threshold
    assert_eq!(
        ChecksumStrategy::for_drive(DriveMediaType::Ssd, u64::MAX),
        ChecksumStrategy::Streaming
    );
}

#[test]
fn test_copy_with_checksum_returns_matching_hashes() {
    let mut src = NamedTempFile::new().unwrap();
    src.write_all(b"hello world").unwrap();
    let dst = NamedTempFile::new().unwrap();
    let (src_hash, dst_hash) = copy_with_checksum(src.path(), dst.path()).unwrap();
    assert_eq!(src_hash, dst_hash);
    assert_eq!(src_hash, checksum_file(dst.path(), ChecksumStrategy::Streaming).unwrap());
}
```

---

### `src/core/tracker.rs` and `src/core/change_detection.rs`

These call `checksum_file` without drive context. Update both call sites to pass `ChecksumStrategy::Streaming`:

- In `tracker.rs`: `checksum::checksum_file(&master_path, checksum::ChecksumStrategy::Streaming)`
- In `change_detection.rs`: `checksum::checksum_file(&full_path, checksum::ChecksumStrategy::Streaming)`

---

### `src/core/mirror.rs`

**`mirror_file` function** — update the `copy_with_checksum` call and remove the separate `checksum_file(&dst)` call:

Replace:
```rust
let src_checksum =
    checksum::copy_with_checksum(&src, &dst).context("Failed to copy file to mirror")?;
let dst_checksum = checksum::checksum_file(&dst)?;

if src_checksum != dst_checksum {
    anyhow::bail!(
        "Mirror verification failed: src={} dst={}",
        src_checksum,
        dst_checksum
    );
}

Ok(src_checksum)
```

With:
```rust
let (src_checksum, dst_checksum) =
    checksum::copy_with_checksum(&src, &dst).context("Failed to copy file to mirror")?;

if src_checksum != dst_checksum {
    anyhow::bail!(
        "Mirror verification failed: src={} dst={}",
        src_checksum,
        dst_checksum
    );
}

Ok(src_checksum)
```

---

### `src/core/integrity.rs`

**Update `check_file_integrity` signature** to accept strategies:

```rust
pub fn check_file_integrity(
    drive_pair: &DrivePair,
    file: &TrackedFile,
    master_strategy: checksum::ChecksumStrategy,
    mirror_strategy: checksum::ChecksumStrategy,
) -> anyhow::Result<IntegrityCheckResult> {
```

Inside the function, replace the two sequential `checksum::checksum_file` calls with `rayon::join`:

```rust
let (master_result, mirror_result) = rayon::join(
    || {
        if primary_root_available && master_path.exists() {
            Some(checksum::checksum_file(&master_path, master_strategy))
        } else {
            None
        }
    },
    || {
        if secondary_root_available && mirror_path.exists() {
            Some(checksum::checksum_file(&mirror_path, mirror_strategy))
        } else {
            None
        }
    },
);

let master_checksum = master_result.transpose()?;
let mirror_checksum = mirror_result.transpose()?;
```

The rest of the function (computing `master_valid`, `mirror_valid`, `status`) is unchanged.

**Update all callers of `check_file_integrity`** to pass two `ChecksumStrategy` arguments. In `integrity_runs.rs` they will be derived from the pair's media types. In any single-file API endpoints, pass `ChecksumStrategy::Streaming` for both since those callers have no `ChecksumConfig`.

---

### `src/core/integrity_runs.rs`

**Add imports:**

```rust
use crate::core::checksum::{self, ChecksumConfig, ChecksumStrategy};
use crate::core::drive::DriveMediaType;
use rayon::prelude::*;
```

**Update `start_run_async`, `run_sync`, and `process_run` signatures** to accept `cfg: ChecksumConfig`:

```rust
pub fn start_run_async(
    repo: &Repository,
    scope_drive_pair_id: Option<i64>,
    recover: bool,
    trigger: &str,
    deadline: Option<std::time::Instant>,
    cfg: ChecksumConfig,
) -> anyhow::Result<IntegrityRun> { … }

pub fn run_sync(
    repo: &Repository,
    scope_drive_pair_id: Option<i64>,
    recover: bool,
    trigger: &str,
    deadline: Option<std::time::Instant>,
    cfg: ChecksumConfig,
) -> anyhow::Result<IntegrityRun> { … }

pub fn process_run(
    repo: &Repository,
    run_id: i64,
    deadline: Option<std::time::Instant>,
    cfg: ChecksumConfig,
) -> anyhow::Result<()> { … }
```

In `start_run_async`, clone `cfg` and move it into the spawned thread:

```rust
std::thread::spawn(move || {
    if let Err(error) = process_run(&repo_clone, run_id, deadline, cfg) {
        // …
    }
});
```

**Rewrite `process_run` body** — the existing structure is a single paged loop over all files. The new structure is:

```
for each drive pair (scoped by scope_drive_pair_id):
    pool_size = pool_size_for_pair(pair media types, cfg)
    build rayon::ThreadPool with pool_size threads
    repo.set_integrity_run_active_workers(run_id, pool_size as i64)

    paginate over pair's files in batches of 100:
        check stop/deadline at top of each batch

        parallel_results: Vec<(file, IntegrityCheckResult_or_error)>
            = pool.install(|| batch.par_iter().map(|file| {
                master_strategy = ChecksumStrategy::for_drive(primary_media, file.file_size as u64)
                mirror_strategy = ChecksumStrategy::for_drive(secondary_media, file.file_size as u64)
                result = check_file_integrity(pair, file, master_strategy, mirror_strategy)
                (file.clone(), result)
              }).collect())

        serial post-batch loop over parallel_results:
            for each (file, result):
                if error: record internal_error, increment progress, continue
                if recover && status != Ok: attempt recovery (serial, touches files + DB)
                update DB: last_integrity_check_at, append_integrity_run_result, increment_progress

    repo.set_integrity_run_active_workers(run_id, 0)

repo.finish_integrity_run(…)
```

The `par_iter` must run **inside the pair's thread pool**, not the global rayon pool:

```rust
let pool = rayon::ThreadPoolBuilder::new()
    .num_threads(pool_size)
    .build()
    .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap());

let parallel_results: Vec<_> = pool.install(|| {
    files.par_iter().map(|file| {
        let master_strategy = ChecksumStrategy::for_drive(
            DriveMediaType::from_str(&pair.primary_media_type),
            file.file_size as u64,
        );
        let mirror_strategy = ChecksumStrategy::for_drive(
            DriveMediaType::from_str(&pair.secondary_media_type),
            file.file_size as u64,
        );
        let result = integrity::check_file_integrity(&pair, file, master_strategy, mirror_strategy);
        (file.clone(), result)
    }).collect()
});
```

**Important:** `rayon::join` inside `check_file_integrity` will use the thread pool it is called from (the pair's pool), not the global rayon pool. This is correct behaviour.

**Stop/deadline check** moves to the top of each batch page iteration (before `par_iter`), not inside the file loop:

```rust
if current_run.stop_requested { /* return stopped */ }
if let Some(dl) = deadline { if Instant::now() >= dl { /* return stopped */ } }
```

---

## Phase D — Config

### `config/default.toml` and `packaging/config.toml`

Add this section to both files:

```toml
[checksum]
# Maximum files checked in parallel for HDD-backed drive pairs.
# HDD seek time makes high parallelism counter-productive; keep this low.
# Default: 2 (allows master+mirror of one file to be read concurrently on separate drives)
hdd_max_parallel = 2

# Maximum files checked in parallel for SSD-backed drive pairs.
# 0 = auto (num_logical_cpus / 2, minimum 2).
ssd_max_parallel = 0
```

### `src/main.rs`

Add a `ChecksumFileConfig` struct for TOML deserialization and wire it through:

```rust
#[derive(serde::Deserialize, Default)]
struct ChecksumFileConfig {
    hdd_max_parallel: Option<usize>,
    ssd_max_parallel: Option<usize>,
}
```

Add to `AppConfig`:

```rust
#[serde(default)]
checksum: ChecksumFileConfig,
```

In the `Commands::Serve` branch, before calling `run_server`, resolve the config:

```rust
let checksum_cfg = bitprotector_lib::core::checksum::ChecksumConfig::resolve(
    file_cfg.checksum.hdd_max_parallel.unwrap_or(2),
    file_cfg.checksum.ssd_max_parallel.unwrap_or(0),
);
```

Pass `checksum_cfg` to `run_server`.

For CLI commands that call `integrity_runs::run_sync` (i.e. `check_all` in `src/cli/commands/integrity.rs`), load `ChecksumConfig::default()` — CLI commands do not go through `run_server`.

### `src/api/server.rs`

Update `run_server` signature:

```rust
pub async fn run_server(
    host: &str,
    port: u16,
    db_path: &str,
    jwt_secret: Vec<u8>,
    tls_cert: Option<&str>,
    tls_key: Option<&str>,
    rate_limit_rps: usize,
    checksum_cfg: ChecksumConfig,
) -> anyhow::Result<()>
```

Store it as app data:

```rust
let checksum_cfg_data = web::Data::new(checksum_cfg);
// in App::new(): .app_data(checksum_cfg_data.clone())
```

In `src/api/routes/integrity.rs`, the `start_run_async` handler extracts it:

```rust
checksum_cfg: web::Data<ChecksumConfig>,
```

And passes `(**checksum_cfg).clone()` to `integrity_runs::start_run_async(…)`.

---

## Phase E — CLI changes

### `src/cli/commands/drives.rs`

**`AddArgs` struct** — add two new fields:

```rust
/// Media type of the primary drive (hdd or ssd)
#[arg(long, default_value = "hdd")]
pub primary_media_type: String,
/// Media type of the secondary drive (hdd or ssd)
#[arg(long, default_value = "hdd")]
pub secondary_media_type: String,
```

Validate in the handler before calling `create_drive_pair`:

```rust
if !["hdd", "ssd"].contains(&args.primary_media_type.as_str()) {
    anyhow::bail!("--primary-media-type must be 'hdd' or 'ssd'");
}
if !["hdd", "ssd"].contains(&args.secondary_media_type.as_str()) {
    anyhow::bail!("--secondary-media-type must be 'hdd' or 'ssd'");
}
```

Pass `&args.primary_media_type, &args.secondary_media_type` as new final args to `repo.create_drive_pair(…)`.

Update the `Add` success output to show media types:

```rust
println!("  Primary:   {} ({})", pair.primary_path, pair.primary_media_type);
println!("  Secondary: {} ({})", pair.secondary_path, pair.secondary_media_type);
```

**`UpdateArgs` struct** — add two optional fields:

```rust
/// New media type for the primary drive (hdd or ssd)
#[arg(long)]
pub primary_media_type: Option<String>,
/// New media type for the secondary drive (hdd or ssd)
#[arg(long)]
pub secondary_media_type: Option<String>,
```

Validate in the handler if present. Pass `.as_deref()` for each to `repo.update_drive_pair(…)`.

**`Show` / `List` output** — wherever the pair is printed, add lines:

```rust
println!("  Primary media:   {}", pair.primary_media_type);
println!("  Secondary media: {}", pair.secondary_media_type);
```

**All internal test calls to `repo.create_drive_pair`** inside the `#[cfg(test)]` block of `drives.rs` must add `"hdd", "hdd"` as last two arguments.

### `src/cli/commands/integrity.rs`

**`check_all` function** — load `ChecksumConfig::default()` and pass to `run_sync`. Print parallelism in completion output:

```rust
fn check_all(repo: &Repository, drive_id: Option<i64>, recover: bool) -> anyhow::Result<()> {
    let cfg = crate::core::checksum::ChecksumConfig::default();
    let run = integrity_runs::run_sync(repo, drive_id, recover, "cli", None, cfg.clone())?;
    let clean = run.processed_files - run.recovered_files;

    println!(
        "Integrity run complete (#{}): {} checked, {} clean, {} recovered, {} need attention",
        run.id, run.processed_files, clean, run.recovered_files, run.attention_files,
    );
    println!(
        "  Parallelism used: HDD pairs={} files/batch, SSD pairs={} files/batch",
        cfg.hdd_max_parallel,
        cfg.resolved_ssd_parallel,
    );
    Ok(())
}
```

**All `#[cfg(test)]` calls to `repo.create_drive_pair`** inside `integrity.rs` must add `"hdd", "hdd"`.

---

## Phase F — API changes

### `src/api/routes/drives.rs`

**`CreateDrivePairRequest`** — add fields:

```rust
#[serde(default = "default_media_type")]
pub primary_media_type: String,
#[serde(default = "default_media_type")]
pub secondary_media_type: String,
```

Add helper at module level: `fn default_media_type() -> String { "hdd".to_string() }`

**`UpdateDrivePairRequest`** — add fields:

```rust
pub primary_media_type: Option<String>,
pub secondary_media_type: Option<String>,
```

**`create_drive_pair` handler** — validate before calling repository:

```rust
let valid = ["hdd", "ssd"];
if !valid.contains(&body.primary_media_type.as_str())
    || !valid.contains(&body.secondary_media_type.as_str())
{
    return HttpResponse::BadRequest()
        .json(ApiError::new("VALIDATION_ERROR", "media_type must be 'hdd' or 'ssd'"));
}
```

Pass `&body.primary_media_type, &body.secondary_media_type` to `repo.create_drive_pair(…)`.

**`update_drive_pair` handler** — pass `.as_deref()` for both optional media type fields to `repo.update_drive_pair(…)`.

---

## Phase G — Frontend changes

### `frontend/src/types/drive.ts`

Add to `DrivePair`:

```typescript
primary_media_type: 'ssd' | 'hdd'
secondary_media_type: 'ssd' | 'hdd'
```

Add to `CreateDrivePairRequest`:

```typescript
primary_media_type?: 'ssd' | 'hdd'
secondary_media_type?: 'ssd' | 'hdd'
```

Add to `UpdateDrivePairRequest`:

```typescript
primary_media_type?: 'ssd' | 'hdd'
secondary_media_type?: 'ssd' | 'hdd'
```

### `frontend/src/types/integrity.ts`

Add to `IntegrityRun` interface:

```typescript
active_workers: number
```

### `frontend/src/components/drives/DriveForm.tsx`

The form currently has `name`, `primary_path`, `secondary_path`, `skip_validation`.

**Update the Zod schema** to add:

```typescript
primary_media_type: z.enum(['hdd', 'ssd']).default('hdd'),
secondary_media_type: z.enum(['hdd', 'ssd']).default('hdd'),
```

**Update `FormData` defaultValues** to add:

```typescript
primary_media_type: 'hdd',
secondary_media_type: 'hdd',
```

**Update the `useEffect` reset** for edit mode to populate from `initial.primary_media_type` and `initial.secondary_media_type`.

**Add two `<select>` fields** in the form JSX, after the `secondary_path` field and before the `skip_validation` checkbox:

```tsx
<Field label="Primary Drive Type">
  <select
    {...register('primary_media_type')}
    className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
    data-testid="drive-primary-media-type-select"
  >
    <option value="hdd">HDD — spinning disk</option>
    <option value="ssd">SSD / NVMe</option>
  </select>
</Field>
<Field label="Secondary Drive Type">
  <select
    {...register('secondary_media_type')}
    className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
    data-testid="drive-secondary-media-type-select"
  >
    <option value="hdd">HDD — spinning disk</option>
    <option value="ssd">SSD / NVMe</option>
  </select>
</Field>
```

### `frontend/src/components/drives/DriveCard.tsx`

Add media-type badges immediately after the state badges section:

```tsx
{/* Media type badges */}
<div className="flex gap-2">
  <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-600 uppercase">
    P: {drive.primary_media_type}
  </span>
  <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-600 uppercase">
    S: {drive.secondary_media_type}
  </span>
</div>
```

### `frontend/src/pages/IntegrityPage.tsx`

Add a conditional "Active Workers" card shown only while a run is live:

```tsx
{activeRun?.status === 'running' && (
  <SummaryCard label="Files checking in parallel" value={activeRun.active_workers} />
)}
```

This card appears and disappears automatically as the 2-second poll updates `activeRun`.

---

## Phase H — Tests

### Existing tests that break due to API changes

**`tests/integration/api_drives.rs`**
- Every call to `repo.create_drive_pair(name, primary, secondary)` must become `repo.create_drive_pair(name, primary, secondary, "hdd", "hdd")`.
- Every JSON assertion on a `DrivePair` response must add `"primary_media_type": "hdd", "secondary_media_type": "hdd"`.
- Add new test cases:
  - `test_create_drive_with_ssd_type`: POST with `primary_media_type: "ssd"`, assert response has `primary_media_type: "ssd"`.
  - `test_create_drive_invalid_media_type`: POST with `primary_media_type: "nvme"`, assert 400 with `VALIDATION_ERROR`.
  - `test_update_drive_media_type`: PUT updating `primary_media_type` from `"hdd"` to `"ssd"`, verify response.

**`tests/integration/cli_drives.rs`**
- Test helper `create_pair` calls must add `"hdd", "hdd"`.
- Add new test cases:
  - `test_cli_add_drive_with_ssd_flag`: run `drives add … --primary-media-type ssd`, assert output contains `"ssd"`.
  - `test_cli_show_drive_includes_media_type`: run `drives show <id>`, assert output contains `"Primary media:"`.
  - `test_cli_add_drive_invalid_media_type`: run `drives add … --primary-media-type nvme`, assert non-zero exit.

**`tests/integration/core_mirror.rs`**
- Every call to `repo.create_drive_pair` must add `"hdd", "hdd"`.
- Any test that captures the return value of `copy_with_checksum` must destructure the tuple: `let (src_hash, dst_hash) = checksum::copy_with_checksum(…)?;`

**`tests/integration/api_integrity.rs`** (if it exists)
- Any assertion on `IntegrityRun` must add `active_workers: 0` to expected values.

**All other integration tests that call `repo.create_drive_pair`**
- Add `"hdd", "hdd"` as final arguments. Search for all occurrences of `create_drive_pair(` in `tests/integration/`.

**`frontend/src/pages/IntegrityPage.test.tsx`**
- Every mock `IntegrityRun` object must add `active_workers: 0`.
- Add a test: when `status === 'running'` and `active_workers > 0`, the "Files checking in parallel" card is rendered with the correct value.

**All vitest unit tests with mock `DrivePair` objects**
- Add `primary_media_type: 'hdd'` and `secondary_media_type: 'hdd'` to every mock `DrivePair`.

---

### New integration test: `tests/integration/core_checksum_strategy.rs`

```rust
use bitprotector_lib::core::checksum::{
    checksum_file, copy_with_checksum, pool_size_for_pair, ChecksumConfig, ChecksumStrategy,
};
use bitprotector_lib::core::drive::DriveMediaType;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_strategy_parity_small_file() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"strategy parity test data").unwrap();
    f.flush().unwrap();

    let hash_stream = checksum_file(f.path(), ChecksumStrategy::Streaming).unwrap();
    let hash_mmap = checksum_file(f.path(), ChecksumStrategy::MmapRayon).unwrap();
    assert_eq!(hash_stream, hash_mmap, "Streaming and MmapRayon must produce identical hashes");
}

#[test]
fn test_copy_with_checksum_dual_hash_matches_independent() {
    let mut src = NamedTempFile::new().unwrap();
    src.write_all(b"copy verification data").unwrap();
    src.flush().unwrap();

    let dst = NamedTempFile::new().unwrap();
    let (src_hash, dst_hash) = copy_with_checksum(src.path(), dst.path()).unwrap();

    let independent_src = checksum_file(src.path(), ChecksumStrategy::Streaming).unwrap();
    let independent_dst = checksum_file(dst.path(), ChecksumStrategy::Streaming).unwrap();

    assert_eq!(src_hash, independent_src);
    assert_eq!(dst_hash, independent_dst);
    assert_eq!(src_hash, dst_hash);
}

#[test]
fn test_pool_size_hdd_hdd() {
    let cfg = ChecksumConfig::resolve(2, 0);
    assert_eq!(pool_size_for_pair(DriveMediaType::Hdd, DriveMediaType::Hdd, &cfg), 2);
}

#[test]
fn test_pool_size_ssd_ssd() {
    let cfg = ChecksumConfig::resolve(2, 4);
    assert_eq!(pool_size_for_pair(DriveMediaType::Ssd, DriveMediaType::Ssd, &cfg), 4);
}

#[test]
fn test_pool_size_mixed() {
    let cfg = ChecksumConfig::resolve(2, 8);
    // Mixed pair: limited by HDD
    assert_eq!(pool_size_for_pair(DriveMediaType::Hdd, DriveMediaType::Ssd, &cfg), 2);
    assert_eq!(pool_size_for_pair(DriveMediaType::Ssd, DriveMediaType::Hdd, &cfg), 2);
}
```

---

## Phase I — QEMU scenarios

### `tests/installation/scenarios/smoke/smoke-13-drive-media-type.sh` (new file)

```bash
#!/bin/bash
# Scenario: create SSD+HDD pair via CLI, verify via API, update via API.
# Bundle: smoke.

smoke_13_drive_media_type() {
    ssh_vm '
set -euo pipefail
DB="${BP_DB:-/mnt/bitprotector-db/db/bp-test.db}"
API="https://localhost:8443/api/v1"
TOKEN="${BP_TOKEN:-}"

PAIR_ID=$(bitprotector --db "$DB" drives add "media-type-test" /tmp/bp-primary /tmp/bp-mirror \
    --primary-media-type ssd --secondary-media-type hdd \
    | grep -oP "Drive pair #\K[0-9]+" | head -1)

[[ -n "$PAIR_ID" ]] || { echo "Failed to create drive pair" >&2; exit 1; }

RESP=$(curl -sk -H "Authorization: Bearer $TOKEN" "$API/drives/$PAIR_ID")
echo "$RESP" | jq -e ".primary_media_type == \"ssd\"" >/dev/null || {
    echo "Expected primary_media_type=ssd, got: $RESP" >&2; exit 1
}
echo "$RESP" | jq -e ".secondary_media_type == \"hdd\"" >/dev/null || {
    echo "Expected secondary_media_type=hdd, got: $RESP" >&2; exit 1
}

curl -sk -X PUT "$API/drives/$PAIR_ID" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"primary_media_type\":\"hdd\"}" | jq -e ".primary_media_type == \"hdd\"" >/dev/null || {
    echo "Update to hdd failed" >&2; exit 1
}

echo "smoke-13 passed"
'
}
```

### `tests/installation/scenarios/smoke/smoke-14-parallel-integrity-progress.sh` (new file)

```bash
#!/bin/bash
# Scenario: SSD pair runs show active_workers > 0 during run, 0 after completion.
# Bundle: smoke.

smoke_14_parallel_integrity_progress() {
    ssh_vm '
set -euo pipefail
DB="${BP_DB:-/mnt/bitprotector-db/db/bp-test.db}"
API="https://localhost:8443/api/v1"
TOKEN="${BP_TOKEN:-}"

PAIR_ID=$(bitprotector --db "$DB" drives add "parallel-test" /tmp/bp-ssd-p /tmp/bp-ssd-m \
    --primary-media-type ssd --secondary-media-type ssd \
    | grep -oP "Drive pair #\K[0-9]+" | head -1)

mkdir -p /tmp/bp-ssd-p /tmp/bp-ssd-m
for i in $(seq 1 15); do
    echo "content-$i" > /tmp/bp-ssd-p/file-$i.txt
    echo "content-$i" > /tmp/bp-ssd-m/file-$i.txt
    bitprotector --db "$DB" files track "$PAIR_ID" "file-$i.txt" >/dev/null
done

RUN=$(curl -sk -X POST "$API/integrity/runs" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"drive_id\":$PAIR_ID,\"recover\":false}")
RUN_ID=$(echo "$RUN" | jq -r ".id")
[[ "$RUN_ID" != "null" ]] || { echo "Failed to start run: $RUN" >&2; exit 1; }

SAW_WORKERS=0
for _ in $(seq 1 20); do
    ACTIVE=$(curl -sk -H "Authorization: Bearer $TOKEN" "$API/integrity/runs/active")
    STATUS=$(echo "$ACTIVE" | jq -r ".run.status // \"null\"")
    WORKERS=$(echo "$ACTIVE" | jq -r ".run.active_workers // 0")
    if [[ "$WORKERS" -gt 0 ]]; then
        SAW_WORKERS=1
        break
    fi
    [[ "$STATUS" == "running" ]] || break
    sleep 0.5
done

for _ in $(seq 1 30); do
    STATUS=$(curl -sk -H "Authorization: Bearer $TOKEN" \
        "$API/integrity/runs/$RUN_ID" 2>/dev/null | jq -r ".status // \"\"")
    [[ "$STATUS" == "completed" || "$STATUS" == "stopped" || "$STATUS" == "failed" ]] && break
    sleep 1
done

FINAL=$(curl -sk -H "Authorization: Bearer $TOKEN" "$API/integrity/runs/$RUN_ID")
FINAL_WORKERS=$(echo "$FINAL" | jq -r ".active_workers")
[[ "$FINAL_WORKERS" == "0" ]] || {
    echo "active_workers should be 0 after run, got: $FINAL_WORKERS" >&2; exit 1
}

CLI_OUT=$(bitprotector --db "$DB" integrity check-all --drive-id "$PAIR_ID" 2>&1 || true)
echo "$CLI_OUT" | grep -q "Parallelism used:" || {
    echo "CLI output missing parallelism info. Got: $CLI_OUT" >&2; exit 1
}

echo "smoke-14 passed"
'
}
```

### `tests/installation/bundles/drive_media_type.sh` (new file)

Follow the structure of `bundles/resilience.sh` exactly. Key parameters:

- `SSH_PORT` default: `2264`
- `API_PORT` default: `18845`
- Two extra virtio block devices: `primary.qcow2` (5 GiB, serial `bpprimary`) and `mirror.qcow2` (5 GiB, serial `bpmirror`)
- Four scenarios: HDD checksum, SSD checksum, SSD active-workers, HDD→SSD update via API

---

## Phase J — CI/CD

### `.github/workflows/ci.yml`

**Add `core_checksum_strategy` step** to the `rust-integration-fast` job, after the `core_scheduler` step:

```yaml
      - name: core_checksum_strategy
        run: cargo test --test core_checksum_strategy
```

**Add `qemu-drive-media-type` job** (Layer 11), after `qemu-failover`. SSH ports 2264/2265, API ports 18845/18846:

```yaml
  qemu-drive-media-type:
    name: QEMU Drive Media Type (${{ matrix.guest }})
    needs: qemu-smoke
    if: >
      github.event_name == 'push' ||
      github.event_name == 'pull_request' ||
      github.event_name == 'schedule' ||
      inputs.run_heavy_qemu == true
    runs-on: ubuntu-24.04
    timeout-minutes: 45
    strategy:
      fail-fast: true
      matrix:
        include:
          - guest: ubuntu-24.04
            ssh_port: 2264
            api_port: 18845
            deb_artifact: bitprotector-deb-ubuntu-24.04
          - guest: ubuntu-26.04
            ssh_port: 2265
            api_port: 18846
            deb_artifact: bitprotector-deb-ubuntu-26.04
    steps:
      - uses: actions/checkout@v5
      - uses: ./.github/actions/setup-qemu
        with:
          guest: ${{ matrix.guest }}
      - name: Download .deb artifact
        uses: actions/download-artifact@v5
        with:
          name: ${{ matrix.deb_artifact }}
          path: target/debian/
      - name: QEMU drive media type test
        env:
          GUEST_IMAGE: ${{ matrix.guest }}
          SSH_PORT: ${{ matrix.ssh_port }}
          API_PORT: ${{ matrix.api_port }}
          CI: '1'
        run: ./tests/installation/bundles/drive_media_type.sh
      - name: Upload VM logs on failure
        if: failure()
        uses: actions/upload-artifact@v5
        with:
          name: qemu-logs-${{ github.job }}-${{ matrix.guest }}
          path: |
            ${{ runner.temp }}/**/serial.log
            ${{ runner.temp }}/**/qemu.log
          retention-days: 7
          if-no-files-found: warn
```

### `.github/workflows/nightly.yml`

Add `qemu-drive-media-type` job after `qemu-scale-lowmem`. SSH ports 2294/2295, API ports 19243/19244:

```yaml
  qemu-drive-media-type:
    name: QEMU Drive Media Type (nightly, ${{ matrix.guest }})
    needs: build-artifacts-nightly
    runs-on: ubuntu-24.04
    timeout-minutes: 45
    strategy:
      fail-fast: false
      matrix:
        include:
          - guest: ubuntu-24.04
            ssh_port: 2294
            api_port: 19243
            deb_artifact: bitprotector-deb-nightly-ubuntu-24.04
          - guest: ubuntu-26.04
            ssh_port: 2295
            api_port: 19244
            deb_artifact: bitprotector-deb-nightly-ubuntu-26.04
    steps:
      - uses: actions/checkout@v5
      - uses: ./.github/actions/setup-qemu
        with:
          guest: ${{ matrix.guest }}
      - name: Download nightly .deb artifact
        uses: actions/download-artifact@v5
        with:
          name: ${{ matrix.deb_artifact }}
          path: target/debian/
      - name: QEMU drive media type test
        env:
          GUEST_IMAGE: ${{ matrix.guest }}
          SSH_PORT: ${{ matrix.ssh_port }}
          API_PORT: ${{ matrix.api_port }}
          CI: '1'
        run: ./tests/installation/bundles/drive_media_type.sh
      - name: Upload VM logs on failure
        if: failure()
        uses: actions/upload-artifact@v5
        with:
          name: qemu-logs-${{ github.job }}-${{ matrix.guest }}
          path: |
            ${{ runner.temp }}/**/serial.log
            ${{ runner.temp }}/**/qemu.log
          retention-days: 7
          if-no-files-found: warn
```

---

## Verification checklist

After implementation, run these in order:

1. `cargo check` — must compile with no errors
2. `cargo clippy -- -D warnings` — must be clean
3. `cargo test --lib` — all unit tests pass (system.rs, checksum.rs strategies, drive.rs)
4. `cargo test --test core_checksum_strategy` — strategy parity and dual-hash tests pass
5. `cargo test --test api_drives` — media-type fields accepted; `"nvme"` → 400
6. `cargo test --test cli_drives` — `--primary-media-type ssd` accepted; `show` includes media type
7. `cargo test --test core_mirror` — compiles with tuple return; no double-read
8. `cargo test --test api_integrity` — `active_workers` field present in all run responses
9. `cargo test --test scaling_100k` — no throughput regression
10. `npm test` (in `frontend/`) — vitest passes with updated mock objects
11. Playwright E2E — media-type dropdowns in DriveForm; HDD/SSD badges on DriveCard; "Files checking in parallel" card appears during active run
12. `./tests/installation/qemu_test.sh` locally — smoke-13 and smoke-14 pass
13. Nightly CI — `qemu-drive-media-type` passes on ubuntu-24.04 and ubuntu-26.04

---

## Key design constraints (do not violate)

- **No global rayon pool for file hashing** — always use the per-pair `ThreadPool` built with `pool.install(|| …)`.
- **DB writes are always serial** — collect results from `par_iter` into a `Vec` then apply serially.
- **Recovery is always serial** — `attempt_recovery_with_reconciliation` touches files and DB; must not run inside `par_iter`.
- **`update_mmap_rayon` takes `&Path` not `&File`** — the blake3 crate opens the file internally; pass `path.as_ref()`.
- **`copy_with_checksum` and `copy_and_verify_checksum` always stream** — they interleave hash with write; mmap cannot be used.
- **Callers without drive context always use `Streaming`** — `tracker.rs`, `change_detection.rs`, `verify_file`, single-file API endpoint.
- **`active_workers` is set to 0 after each pair's batch completes** — the frontend will see 0 when idle and a positive number only while a batch is in flight.
- **The QEMU upgrade job is intentionally NOT updated** — alpha stage; schema changes break alpha1→current upgrade by design.
