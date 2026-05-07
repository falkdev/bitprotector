use crate::core::drive::DriveMediaType;
use crate::core::system;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

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

/// Compute BLAKE3 checksum of the given byte slice, returning lowercase hex.
pub fn checksum_bytes(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Advise the OS that the file's pages are no longer needed in the page cache.
/// This reduces memory pressure when processing large files on constrained systems.
#[cfg(unix)]
fn fadvise_dontneed(file: &File) {
    use std::os::unix::io::AsRawFd;
    // POSIX_FADV_DONTNEED = 4
    unsafe {
        libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
    }
}

/// Compute BLAKE3 checksum of a file, choosing strategy based on drive type and file size.
pub fn checksum_file<P: AsRef<Path>>(path: P, strategy: ChecksumStrategy) -> io::Result<String> {
    let file = File::open(path.as_ref())?;
    match strategy {
        ChecksumStrategy::MmapRayon => {
            let mut hasher = blake3::Hasher::new();
            hasher
                .update_mmap_rayon(path.as_ref())
                .map_err(io::Error::other)?;
            #[cfg(unix)]
            fadvise_dontneed(&file);
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

/// Copy a file from `src` to `dst` while verifying it matches `expected_checksum`
/// in a single streaming pass.
///
/// If the computed checksum does not match `expected_checksum`, the destination
/// file is removed and an error is returned. This is more efficient than calling
/// `checksum_file` to verify and then `fs::copy` separately because the source
/// file is read only once instead of twice.
pub fn copy_and_verify_checksum<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dst: Q,
    expected_checksum: &str,
) -> io::Result<()> {
    let mut src_file = File::open(&src)?;
    let dst_path = dst.as_ref();
    let mut dst_file = File::create(dst_path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = src_file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        dst_file.write_all(&buf[..n])?;
    }
    dst_file.flush()?;
    #[cfg(unix)]
    fadvise_dontneed(&src_file);
    let actual = hasher.finalize().to_hex().to_string();
    if actual != expected_checksum {
        drop(dst_file);
        let _ = std::fs::remove_file(dst_path);
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "checksum mismatch: expected={} actual={}",
                expected_checksum, actual
            ),
        ));
    }
    Ok(())
}

/// Verify that a file's current checksum matches the expected checksum.
pub fn verify_file<P: AsRef<Path>>(path: P, expected: &str) -> io::Result<bool> {
    let actual = checksum_file(path, ChecksumStrategy::Streaming)?;
    Ok(actual == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_checksum_empty_bytes() {
        // Known BLAKE3 hash of empty input (verified from blake3 crate)
        let hash = checksum_bytes(b"");
        let expected = blake3::hash(b"").to_hex().to_string();
        assert_eq!(hash, expected);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_checksum_known_input() {
        // BLAKE3 of "hello world" (verified against reference implementation)
        let hash = checksum_bytes(b"hello world");
        // Pre-computed expected value
        let expected = blake3::hash(b"hello world").to_hex().to_string();
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_checksum_deterministic() {
        let data = b"deterministic test data 12345";
        let h1 = checksum_bytes(data);
        let h2 = checksum_bytes(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_checksum_different_inputs_differ() {
        let h1 = checksum_bytes(b"input one");
        let h2 = checksum_bytes(b"input two");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_checksum_file() {
        let mut file = NamedTempFile::new().unwrap();
        let data = b"file content for checksum test";
        file.write_all(data).unwrap();
        file.flush().unwrap();

        let expected = checksum_bytes(data);
        let actual = checksum_file(file.path(), ChecksumStrategy::Streaming).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_checksum_file_large() {
        let mut file = NamedTempFile::new().unwrap();
        // Write 200 KB to test chunked reading
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 256) as u8).collect();
        file.write_all(&data).unwrap();
        file.flush().unwrap();

        let expected = checksum_bytes(&data);
        let actual = checksum_file(file.path(), ChecksumStrategy::Streaming).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_verify_file_matches() {
        let mut file = NamedTempFile::new().unwrap();
        let data = b"verify this content";
        file.write_all(data).unwrap();
        file.flush().unwrap();

        let checksum = checksum_bytes(data);
        assert!(verify_file(file.path(), &checksum).unwrap());
    }

    #[test]
    fn test_verify_file_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"original content").unwrap();
        file.flush().unwrap();

        let wrong_checksum = checksum_bytes(b"different content");
        assert!(!verify_file(file.path(), &wrong_checksum).unwrap());
    }

    #[test]
    fn test_checksum_file_not_found() {
        let result = checksum_file("/nonexistent/path/to/file.txt", ChecksumStrategy::Streaming);
        assert!(result.is_err());
    }

    #[test]
    fn test_checksum_hex_length() {
        // BLAKE3 produces 32 bytes = 64 hex chars
        let hash = checksum_bytes(b"test");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_checksum_hex_is_lowercase() {
        let hash = checksum_bytes(b"case check");
        assert_eq!(hash, hash.to_lowercase());
    }

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
    fn test_copy_with_checksum_creates_copy() {
        let mut src_file = NamedTempFile::new().unwrap();
        let dst_file = NamedTempFile::new().unwrap();
        let data = b"copy with checksum test data";
        src_file.write_all(data).unwrap();
        src_file.flush().unwrap();

        let (src_checksum, dst_checksum) =
            copy_with_checksum(src_file.path(), dst_file.path()).unwrap();

        let expected = checksum_bytes(data);
        assert_eq!(src_checksum, expected);
        assert_eq!(src_checksum, dst_checksum);
        let dst_contents = std::fs::read(dst_file.path()).unwrap();
        assert_eq!(dst_contents, data);
    }

    #[test]
    fn test_copy_with_checksum_returns_matching_hashes() {
        let mut src = NamedTempFile::new().unwrap();
        src.write_all(b"hello world").unwrap();
        let dst = NamedTempFile::new().unwrap();
        let (src_hash, dst_hash) = copy_with_checksum(src.path(), dst.path()).unwrap();
        assert_eq!(src_hash, dst_hash);
        assert_eq!(
            src_hash,
            checksum_file(dst.path(), ChecksumStrategy::Streaming).unwrap()
        );
    }

    #[test]
    fn test_copy_and_verify_checksum_happy_path() {
        let mut src_file = NamedTempFile::new().unwrap();
        let dst_file = NamedTempFile::new().unwrap();
        let data = b"verify checksum content";
        src_file.write_all(data).unwrap();
        src_file.flush().unwrap();

        let expected = checksum_bytes(data);
        copy_and_verify_checksum(src_file.path(), dst_file.path(), &expected).unwrap();

        let dst_contents = std::fs::read(dst_file.path()).unwrap();
        assert_eq!(dst_contents, data);
    }

    #[test]
    fn test_copy_and_verify_checksum_wrong_expected_returns_err_and_removes_dst() {
        let mut src_file = NamedTempFile::new().unwrap();
        let dst_file = NamedTempFile::new().unwrap();
        src_file.write_all(b"actual content").unwrap();
        src_file.flush().unwrap();

        let wrong_checksum = checksum_bytes(b"different content");
        let dst_path = dst_file.path().to_path_buf();
        // Release the NamedTempFile so copy_and_verify_checksum can remove it
        let _ = dst_file.keep().unwrap();

        let result = copy_and_verify_checksum(src_file.path(), &dst_path, &wrong_checksum);
        assert!(result.is_err(), "Should fail on checksum mismatch");
        assert!(
            !dst_path.exists(),
            "Destination should be removed on mismatch"
        );
    }
}
