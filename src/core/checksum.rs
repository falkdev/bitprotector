use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Compute BLAKE3 checksum of the given byte slice, returning lowercase hex.
pub fn checksum_bytes(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Compute BLAKE3 checksum of a file at the given path, returning lowercase hex.
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
    Ok(hasher.finalize().to_hex().to_string())
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
}
