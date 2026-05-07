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
    assert_eq!(
        hash_stream, hash_mmap,
        "Streaming and MmapRayon must produce identical hashes"
    );
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
    assert_eq!(
        pool_size_for_pair(DriveMediaType::Hdd, DriveMediaType::Hdd, &cfg),
        2
    );
}

#[test]
fn test_pool_size_ssd_ssd() {
    let cfg = ChecksumConfig::resolve(2, 4);
    assert_eq!(
        pool_size_for_pair(DriveMediaType::Ssd, DriveMediaType::Ssd, &cfg),
        4
    );
}

#[test]
fn test_pool_size_mixed() {
    let cfg = ChecksumConfig::resolve(2, 8);
    // Mixed pair: limited by HDD
    assert_eq!(
        pool_size_for_pair(DriveMediaType::Hdd, DriveMediaType::Ssd, &cfg),
        2
    );
    assert_eq!(
        pool_size_for_pair(DriveMediaType::Ssd, DriveMediaType::Hdd, &cfg),
        2
    );
}
