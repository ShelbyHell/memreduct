use crate::system::{self, MemoryInfo};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
use self::windows as sys;

#[cfg(not(target_os = "windows"))]
pub mod linux;
#[cfg(not(target_os = "windows"))]
use self::linux as sys;

pub const REDUCT_WORKINGSET: u32 = 0x01;
pub const REDUCT_SYSTEMFILECACHE: u32 = 0x02;
pub const REDUCT_STANDBYPRIORITY0LIST: u32 = 0x04;
pub const REDUCT_STANDBYLIST: u32 = 0x08;
pub const REDUCT_MODIFIEDLIST: u32 = 0x10;
pub const REDUCT_COMBINEMEMORYLISTS: u32 = 0x20;
pub const REDUCT_REGISTRYCACHE: u32 = 0x40;
pub const REDUCT_MODIFIEDFILECACHE: u32 = 0x80;

pub const REDUCT_MASK_ALL: u32 = REDUCT_WORKINGSET
    | REDUCT_SYSTEMFILECACHE
    | REDUCT_STANDBYPRIORITY0LIST
    | REDUCT_STANDBYLIST
    | REDUCT_MODIFIEDLIST
    | REDUCT_COMBINEMEMORYLISTS
    | REDUCT_REGISTRYCACHE
    | REDUCT_MODIFIEDFILECACHE;

pub const REDUCT_MASK_DEFAULT: u32 = REDUCT_WORKINGSET
    | REDUCT_SYSTEMFILECACHE
    | REDUCT_STANDBYPRIORITY0LIST
    | REDUCT_REGISTRYCACHE
    | REDUCT_COMBINEMEMORYLISTS
    | REDUCT_MODIFIEDFILECACHE;

#[allow(dead_code)]
pub const REDUCT_MASK_FREEZES: u32 = REDUCT_STANDBYLIST | REDUCT_MODIFIEDLIST;

#[derive(Debug, Clone)]
pub struct CleanReport {
    #[allow(dead_code)]
    pub regions_attempted: Vec<String>,
    pub regions_succeeded: Vec<String>,
    pub regions_failed: Vec<(String, String)>,
    pub mem_before: MemoryInfo,
    pub mem_after: MemoryInfo,
    pub bytes_freed: u64,
    pub duration: Duration,
}

pub fn get_region_names(mask: u32) -> Vec<&'static str> {
    let mut names = Vec::new();
    if (mask & REDUCT_WORKINGSET) != 0 {
        names.push("Working Set");
    }
    if (mask & REDUCT_SYSTEMFILECACHE) != 0 {
        names.push("System File Cache");
    }
    if (mask & REDUCT_STANDBYPRIORITY0LIST) != 0 {
        names.push("Standby (Priority 0)");
    }
    if (mask & REDUCT_STANDBYLIST) != 0 {
        names.push("Standby List");
    }
    if (mask & REDUCT_MODIFIEDLIST) != 0 {
        names.push("Modified Page List");
    }
    if (mask & REDUCT_COMBINEMEMORYLISTS) != 0 {
        names.push("Combine Memory Lists");
    }
    if (mask & REDUCT_REGISTRYCACHE) != 0 {
        names.push("Registry Cache");
    }
    if (mask & REDUCT_MODIFIEDFILECACHE) != 0 {
        names.push("Flush Volume Cache");
    }
    names
}

pub fn parse_region_name(name: &str) -> Option<u32> {
    let n = name.to_lowercase().replace(['-', '_', ' '], "");
    match n.as_str() {
        "workingset" | "ws" => Some(REDUCT_WORKINGSET),
        "systemfilecache" | "filecache" | "cache" | "sfc" => Some(REDUCT_SYSTEMFILECACHE),
        "standbypriority0" | "standbyprio0" | "prio0" => Some(REDUCT_STANDBYPRIORITY0LIST),
        "standbylist" | "standby" => Some(REDUCT_STANDBYLIST),
        "modifiedlist" | "modified" => Some(REDUCT_MODIFIEDLIST),
        "combinememorylists" | "combine" => Some(REDUCT_COMBINEMEMORYLISTS),
        "registrycache" | "registry" | "reg" => Some(REDUCT_REGISTRYCACHE),
        "modifiedfilecache" | "volumecache" | "volume" | "flush" => Some(REDUCT_MODIFIEDFILECACHE),
        "all" | "full" => Some(REDUCT_MASK_ALL),
        "default" => Some(REDUCT_MASK_DEFAULT),
        _ => None,
    }
}

pub fn is_elevated() -> bool {
    sys::is_elevated()
}

pub fn clean_memory(mask: u32) -> Result<CleanReport, String> {
    let start_time = Instant::now();
    let mem_before = system::get_memory_info()?;

    let (succeeded, failed) = sys::execute_clean(mask)?;

    let mem_after = system::get_memory_info().unwrap_or_else(|_| mem_before.clone());
    let bytes_freed = if mem_before.used_physical > mem_after.used_physical {
        mem_before.used_physical - mem_after.used_physical
    } else {
        0
    };

    let mut attempted = Vec::new();
    for name in get_region_names(mask) {
        attempted.push(name.to_string());
    }

    Ok(CleanReport {
        regions_attempted: attempted,
        regions_succeeded: succeeded,
        regions_failed: failed,
        mem_before,
        mem_after,
        bytes_freed,
        duration: start_time.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_regions() {
        let regions = get_region_names(REDUCT_MASK_DEFAULT);
        assert!(regions.contains(&"Working Set"));
        assert!(regions.contains(&"System File Cache"));
        assert!(regions.contains(&"Standby (Priority 0)"));
        assert!(!regions.contains(&"Standby List")); // Default doesn't include freezing lists
        assert!(!regions.contains(&"Modified Page List"));

        let all_regions = get_region_names(REDUCT_MASK_ALL);
        assert_eq!(all_regions.len(), 8);
    }

    #[test]
    fn test_parse_region_name() {
        assert_eq!(parse_region_name("workingset"), Some(REDUCT_WORKINGSET));
        assert_eq!(parse_region_name("ws"), Some(REDUCT_WORKINGSET));
        assert_eq!(parse_region_name("cache"), Some(REDUCT_SYSTEMFILECACHE));
        assert_eq!(parse_region_name("standby"), Some(REDUCT_STANDBYLIST));
        assert_eq!(parse_region_name("modified"), Some(REDUCT_MODIFIEDLIST));
        assert_eq!(parse_region_name("all"), Some(REDUCT_MASK_ALL));
        assert_eq!(parse_region_name("default"), Some(REDUCT_MASK_DEFAULT));
        assert_eq!(parse_region_name("nonexistent"), None);
    }
}
