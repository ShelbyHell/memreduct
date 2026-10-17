use super::MemoryInfo;
use std::mem::size_of;

#[repr(C)]
#[allow(non_snake_case)]
struct MEMORYSTATUSEX {
    dwLength: u32,
    dwMemoryLoad: u32,
    ullTotalPhys: u64,
    ullAvailPhys: u64,
    ullTotalPageFile: u64,
    ullAvailPageFile: u64,
    ullTotalVirtual: u64,
    ullAvailVirtual: u64,
    ullAvailExtendedVirtual: u64,
}

#[repr(C)]
#[allow(non_snake_case)]
struct SYSTEM_FILECACHE_INFORMATION {
    CurrentSize: usize,
    PeakSize: usize,
    PageFaultCount: u32,
    MinimumWorkingSet: usize,
    MaximumWorkingSet: usize,
    CurrentSizeIncludingTransitionInPages: usize,
    PeakSizeIncludingTransitionInPages: usize,
    TransitionRePurposeCount: u32,
    Flags: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalMemoryStatusEx(lpBuffer: *mut MEMORYSTATUSEX) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQuerySystemInformation(
        SystemInformationClass: u32,
        SystemInformation: *mut std::ffi::c_void,
        SystemInformationLength: u32,
        ReturnLength: *mut u32,
    ) -> i32;
}

const SYSTEM_FILECACHE_INFORMATION_CLASS: u32 = 21;

pub fn get_memory_info() -> Result<MemoryInfo, String> {
    unsafe {
        let mut mem_status = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            dwMemoryLoad: 0,
            ullTotalPhys: 0,
            ullAvailPhys: 0,
            ullTotalPageFile: 0,
            ullAvailPageFile: 0,
            ullTotalVirtual: 0,
            ullAvailVirtual: 0,
            ullAvailExtendedVirtual: 0,
        };

        if GlobalMemoryStatusEx(&mut mem_status) == 0 {
            return Err("GlobalMemoryStatusEx failed".to_string());
        }

        let total_physical = mem_status.ullTotalPhys;
        let avail_physical = mem_status.ullAvailPhys;
        let used_physical = total_physical.saturating_sub(avail_physical);
        let percent_physical = mem_status.dwMemoryLoad.min(100) as u8;

        let total_pagefile = mem_status.ullTotalPageFile;
        let avail_pagefile = mem_status.ullAvailPageFile;
        let used_pagefile = total_pagefile.saturating_sub(avail_pagefile);
        let percent_pagefile = if total_pagefile > 0 {
            ((used_pagefile as f64 / total_pagefile as f64) * 100.0).round().min(100.0) as u8
        } else {
            0
        };

        // Attempt to query system file cache info
        let mut sfci: SYSTEM_FILECACHE_INFORMATION = std::mem::zeroed();
        let mut ret_len: u32 = 0;
        let status = NtQuerySystemInformation(
            SYSTEM_FILECACHE_INFORMATION_CLASS,
            &mut sfci as *mut _ as *mut std::ffi::c_void,
            size_of::<SYSTEM_FILECACHE_INFORMATION>() as u32,
            &mut ret_len,
        );

        let cached = if status >= 0 {
            Some(sfci.CurrentSize as u64)
        } else {
            None
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
            cached,
            buffers: None,
            standby: None,
            modified: None,
        })
    }
}
