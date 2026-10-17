use super::MemoryInfo;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn get_memory_info() -> Result<MemoryInfo, String> {
    let file = File::open("/proc/meminfo")
        .map_err(|e| format!("Failed to open /proc/meminfo: {}", e))?;
    let reader = BufReader::new(file);

    let mut mem_total_kb: u64 = 0;
    let mut mem_free_kb: u64 = 0;
    let mut mem_avail_kb: Option<u64> = None;
    let mut buffers_kb: u64 = 0;
    let mut cached_kb: u64 = 0;
    let mut swap_total_kb: u64 = 0;
    let mut swap_free_kb: u64 = 0;
    let mut sreclaimable_kb: u64 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let mut parts = line.split_whitespace();
        let key = match parts.next() {
            Some(k) => k.trim_end_matches(':'),
            None => continue,
        };
        let val = match parts.next().and_then(|v| v.parse::<u64>().ok()) {
            Some(v) => v,
            None => continue,
        };

        match key {
            "MemTotal" => mem_total_kb = val,
            "MemFree" => mem_free_kb = val,
            "MemAvailable" => mem_avail_kb = Some(val),
            "Buffers" => buffers_kb = val,
            "Cached" => cached_kb = val,
            "SwapTotal" => swap_total_kb = val,
            "SwapFree" => swap_free_kb = val,
            "SReclaimable" => sreclaimable_kb = val,
            _ => {}
        }
    }

    let total_physical = mem_total_kb * 1024;
    let avail_physical = match mem_avail_kb {
        Some(avail) => avail * 1024,
        None => (mem_free_kb + buffers_kb + cached_kb + sreclaimable_kb) * 1024,
    };
    let used_physical = total_physical.saturating_sub(avail_physical);
    let percent_physical = if total_physical > 0 {
        ((used_physical as f64 / total_physical as f64) * 100.0).round().min(100.0) as u8
    } else {
        0
    };

    let total_pagefile = swap_total_kb * 1024;
    let avail_pagefile = swap_free_kb * 1024;
    let used_pagefile = total_pagefile.saturating_sub(avail_pagefile);
    let percent_pagefile = if total_pagefile > 0 {
        ((used_pagefile as f64 / total_pagefile as f64) * 100.0).round().min(100.0) as u8
    } else {
        0
    };

    Ok(MemoryInfo {
        total_physical,
        avail_physical,
        used_physical,
        percent_physical,
        total_pagefile,
        avail_pagefile,
        used_pagefile,
        percent_pagefile,
        cached: Some(cached_kb * 1024),
        buffers: Some(buffers_kb * 1024),
        standby: None,
        modified: None,
    })
}
