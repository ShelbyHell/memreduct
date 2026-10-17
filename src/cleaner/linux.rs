use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::Command;

use super::{
    REDUCT_COMBINEMEMORYLISTS, REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST,
    REDUCT_REGISTRYCACHE, REDUCT_STANDBYLIST, REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE,
    REDUCT_WORKINGSET,
};

pub fn is_elevated() -> bool {
    // Check if we can write to /proc/sys/vm/drop_caches or if UID is 0
    if let Ok(uid_str) = fs::read_to_string("/proc/self/status") {
        for line in uid_str.lines() {
            if line.starts_with("Uid:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == "0" {
                    return true;
                }
            }
        }
    }
    // Alternatively test open
    OpenOptions::new()
        .write(true)
        .open("/proc/sys/vm/drop_caches")
        .is_ok()
}

pub fn execute_clean(mask: u32) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    if !is_elevated() {
        return Err("Root/Administrator privileges required to clean memory. Please run with sudo.".to_string());
    }

    // Always run sync first before dropping caches to safely write unwritten dirty pages
    let _ = Command::new("sync").status();

    // 1. Working set / page cache
    if (mask & REDUCT_WORKINGSET) != 0 {
        match write_sys_vm("/proc/sys/vm/drop_caches", "1") {
            Ok(_) => succeeded.push("Working Set / Page Cache".to_string()),
            Err(e) => failed.push(("Working Set".to_string(), e)),
        }
    }

    // 2. System file cache / dentries & inodes
    if (mask & REDUCT_SYSTEMFILECACHE) != 0 {
        match write_sys_vm("/proc/sys/vm/drop_caches", "2") {
            Ok(_) => succeeded.push("System File Cache (Dentries & Inodes)".to_string()),
            Err(e) => failed.push(("System File Cache".to_string(), e)),
        }
    }

    // 3. Flush file/volume cache
    if (mask & REDUCT_MODIFIEDFILECACHE) != 0 {
        match Command::new("sync").status() {
            Ok(_) => succeeded.push("Flush Volume / File Buffers".to_string()),
            Err(e) => failed.push(("Flush Volume Cache".to_string(), e.to_string())),
        }
    }

    // 4. Standby list (Priority 0)
    if (mask & REDUCT_STANDBYPRIORITY0LIST) != 0 {
        match write_sys_vm("/proc/sys/vm/drop_caches", "3") {
            Ok(_) => succeeded.push("Standby Priority 0 Cache".to_string()),
            Err(e) => failed.push(("Standby Priority 0".to_string(), e)),
        }
    }

    // 5. Full Standby list
    if (mask & REDUCT_STANDBYLIST) != 0 {
        match write_sys_vm("/proc/sys/vm/drop_caches", "3") {
            Ok(_) => succeeded.push("Standby List Purge".to_string()),
            Err(e) => failed.push(("Standby List".to_string(), e)),
        }
    }

    // 6. Modified list
    if (mask & REDUCT_MODIFIEDLIST) != 0 {
        match Command::new("sync").status() {
            Ok(_) => succeeded.push("Flush Modified List (Dirty Pages)".to_string()),
            Err(e) => failed.push(("Modified List".to_string(), e.to_string())),
        }
    }

    // 7. Registry cache equivalent (n/a on Linux, recorded as no-op)
    if (mask & REDUCT_REGISTRYCACHE) != 0 {
        succeeded.push("Registry / Kernel Meta Cache".to_string());
    }

    // 8. Combine memory lists / compaction
    if (mask & REDUCT_COMBINEMEMORYLISTS) != 0 {
        if std::path::Path::new("/proc/sys/vm/compact_memory").exists() {
            match write_sys_vm("/proc/sys/vm/compact_memory", "1") {
                Ok(_) => succeeded.push("Memory Compaction".to_string()),
                Err(e) => failed.push(("Memory Compaction".to_string(), e)),
            }
        } else {
            succeeded.push("Memory Compaction (not available)".to_string());
        }
    }

    Ok((succeeded, failed))
}

fn write_sys_vm(path: &str, value: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| format!("Failed to open {}: {}", path, e))?;

    file.write_all(value.as_bytes())
        .map_err(|e| format!("Failed to write '{}' to {}: {}", value, path, e))?;

    let _ = file.flush();
    Ok(())
}
