#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
use self::windows as sys;

#[cfg(not(target_os = "windows"))]
pub mod linux;
#[cfg(not(target_os = "windows"))]
use self::linux as sys;

#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub total_physical: u64,
    pub avail_physical: u64,
    pub used_physical: u64,
    pub percent_physical: u8,

    pub total_pagefile: u64,
    pub avail_pagefile: u64,
    pub used_pagefile: u64,
    pub percent_pagefile: u8,

    pub cached: Option<u64>,
    pub buffers: Option<u64>,
    pub standby: Option<u64>,
    pub modified: Option<u64>,
}

pub fn get_memory_info() -> Result<MemoryInfo, String> {
    sys::get_memory_info()
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1.00 TB");
    }

    #[test]
    fn test_get_memory_info() {
        let info = get_memory_info().expect("get_memory_info failed");
        assert!(info.total_physical > 0);
        assert!(info.percent_physical <= 100);
    }
}
