use anyhow::Context;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SIDECAR_BYTES: u64 = 4096;

fn sidecar_path_for(file_path: &Path) -> anyhow::Result<PathBuf> {
    let filename = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("File path has no terminal filename"))?;
    let mut sidecar_name = OsString::from(filename);
    sidecar_name.push(".b3");
    Ok(file_path.with_file_name(sidecar_name))
}

fn normalize_b3_hex(candidate: &str) -> anyhow::Result<String> {
    let lowered = candidate.to_ascii_lowercase();
    if lowered.len() != 64 {
        anyhow::bail!("Invalid BLAKE3 hash length in sidecar: expected 64 hex chars")
    }
    if !lowered.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Invalid BLAKE3 hash in sidecar: expected hex characters only")
    }
    Ok(lowered)
}

/// Read and parse a `<file>.b3` sidecar hash.
///
/// Returns:
/// - `Ok(None)` when no sidecar exists.
/// - `Ok(Some(hash))` when a valid 64-char hex hash is parsed.
/// - `Err` when a sidecar exists but is malformed.
pub fn read_b3_sidecar(file_path: &Path) -> anyhow::Result<Option<String>> {
    let sidecar_path = sidecar_path_for(file_path)?;
    if !sidecar_path.exists() {
        return Ok(None);
    }

    let metadata = fs::metadata(&sidecar_path)
        .with_context(|| format!("Failed to inspect sidecar {}", sidecar_path.display()))?;
    if metadata.len() > MAX_SIDECAR_BYTES {
        anyhow::bail!(
            "Sidecar is too large ({} bytes, max {}): {}",
            metadata.len(),
            MAX_SIDECAR_BYTES,
            sidecar_path.display()
        );
    }

    let content = fs::read_to_string(&sidecar_path)
        .with_context(|| format!("Failed to read sidecar {}", sidecar_path.display()))?;

    let first_non_empty = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Sidecar is empty: {}", sidecar_path.display()))?;

    let token = first_non_empty
        .split_whitespace()
        .next()
        .unwrap_or(first_non_empty);

    Ok(Some(normalize_b3_hex(token)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn valid_hex() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
    }

    #[test]
    fn test_read_b3_sidecar_missing_returns_none() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("doc.txt");
        fs::write(&file_path, b"content").unwrap();

        let parsed = read_b3_sidecar(&file_path).unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn test_read_b3_sidecar_parses_b3sum_format() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("report.bin");
        fs::write(&file_path, b"content").unwrap();

        let hash = valid_hex();
        fs::write(
            file_path.with_file_name("report.bin.b3"),
            format!("{}  report.bin\n", hash),
        )
        .unwrap();

        let parsed = read_b3_sidecar(&file_path).unwrap();
        assert_eq!(parsed, Some(hash));
    }

    #[test]
    fn test_read_b3_sidecar_parses_raw_hex_and_lowercases() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("movie.mkv");
        fs::write(&file_path, b"content").unwrap();

        let hash_upper =
            "ABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCD".to_string();
        fs::write(
            file_path.with_file_name("movie.mkv.b3"),
            format!("{}\n", hash_upper),
        )
        .unwrap();

        let parsed = read_b3_sidecar(&file_path).unwrap();
        assert_eq!(parsed, Some(hash_upper.to_ascii_lowercase()));
    }

    #[test]
    fn test_read_b3_sidecar_rejects_non_hex() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("a.txt");
        fs::write(&file_path, b"content").unwrap();

        fs::write(
            file_path.with_file_name("a.txt.b3"),
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        )
        .unwrap();

        let err = read_b3_sidecar(&file_path).unwrap_err();
        assert!(err.to_string().contains("expected hex"));
    }

    #[test]
    fn test_read_b3_sidecar_rejects_wrong_length() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("a.txt");
        fs::write(&file_path, b"content").unwrap();

        fs::write(file_path.with_file_name("a.txt.b3"), "abc123").unwrap();

        let err = read_b3_sidecar(&file_path).unwrap_err();
        assert!(err.to_string().contains("expected 64"));
    }

    #[test]
    fn test_read_b3_sidecar_rejects_empty_file() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("a.txt");
        fs::write(&file_path, b"content").unwrap();

        fs::write(file_path.with_file_name("a.txt.b3"), "\n\n").unwrap();

        let err = read_b3_sidecar(&file_path).unwrap_err();
        assert!(err.to_string().contains("Sidecar is empty"));
    }
}
