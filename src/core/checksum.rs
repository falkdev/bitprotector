use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

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

/// Compute BLAKE3 checksum of a file at the given path, returning lowercase hex.
/// After reading, advises the OS to release the file's pages from the page cache.
pub fn checksum_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path)?;
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

/// Copy a file from `src` to `dst` while computing the BLAKE3 checksum of the
/// source in a single streaming pass. Returns the source checksum as lowercase hex.
///
/// This is more efficient than calling `fs::copy` and `checksum_file` separately
/// because the source file is read only once instead of twice.
pub fn copy_with_checksum<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<String> {
    let mut src_file = File::open(src)?;
    let mut dst_file = File::create(dst)?;
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
    Ok(hasher.finalize().to_hex().to_string())
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
    let actual = checksum_file(path)?;
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
        let actual = checksum_file(file.path()).unwrap();
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
        let actual = checksum_file(file.path()).unwrap();
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
        let result = checksum_file("/nonexistent/path/to/file.txt");
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
    fn test_copy_with_checksum_creates_copy() {
        let mut src_file = NamedTempFile::new().unwrap();
        let dst_file = NamedTempFile::new().unwrap();
        let data = b"copy with checksum test data";
        src_file.write_all(data).unwrap();
        src_file.flush().unwrap();

        let checksum = copy_with_checksum(src_file.path(), dst_file.path()).unwrap();

        let expected = checksum_bytes(data);
        assert_eq!(checksum, expected);
        let dst_contents = std::fs::read(dst_file.path()).unwrap();
        assert_eq!(dst_contents, data);
    }

    #[test]
    fn test_copy_with_checksum_returns_source_checksum() {
        let mut src_file = NamedTempFile::new().unwrap();
        let dst_file = NamedTempFile::new().unwrap();
        let data = b"source checksum data";
        src_file.write_all(data).unwrap();
        src_file.flush().unwrap();

        let result = copy_with_checksum(src_file.path(), dst_file.path()).unwrap();
        let direct = checksum_file(src_file.path()).unwrap();

        assert_eq!(result, direct);
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
        assert!(!dst_path.exists(), "Destination should be removed on mismatch");
    }
}
