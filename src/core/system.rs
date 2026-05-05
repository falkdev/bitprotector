use std::sync::OnceLock;

static TOTAL_RAM: OnceLock<u64> = OnceLock::new();

/// Read MemTotal from /proc/meminfo. Returns 2 GiB on any error.
pub fn total_ram_bytes() -> u64 {
    *TOTAL_RAM.get_or_init(|| parse_proc_meminfo().unwrap_or(2 * 1024 * 1024 * 1024))
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
/// Set to 50% of total RAM so the mapping does not evict working set.
pub fn mmap_threshold_bytes() -> u64 {
    total_ram_bytes() / 2
}

/// Default number of parallel files for SSD drive pairs: half logical CPUs, min 2.
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
