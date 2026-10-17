use super::{
    REDUCT_COMBINEMEMORYLISTS, REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST,
    REDUCT_REGISTRYCACHE, REDUCT_STANDBYLIST, REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE,
    REDUCT_WORKINGSET,
};
use std::mem::size_of;
use std::ptr;

type HANDLE = *mut std::ffi::c_void;
type NTSTATUS = i32;

const STATUS_SUCCESS: NTSTATUS = 0;
const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
const TOKEN_QUERY: u32 = 0x0008;
const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;
const TokenElevation: u32 = 20;

const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const FILE_SHARE_READ: u32 = 0x00000001;
const FILE_SHARE_WRITE: u32 = 0x00000002;
const OPEN_EXISTING: u32 = 3;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
const DRIVE_FIXED: u32 = 3;

#[repr(C)]
struct LUID {
    LowPart: u32,
    HighPart: i32,
}

#[repr(C)]
struct LUID_AND_ATTRIBUTES {
    Luid: LUID,
    Attributes: u32,
}

#[repr(C)]
struct TOKEN_PRIVILEGES {
    PrivilegeCount: u32,
    Privileges: [LUID_AND_ATTRIBUTES; 1],
}

#[repr(C)]
struct TOKEN_ELEVATION {
    TokenIsElevated: u32,
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

#[repr(C)]
#[allow(non_snake_case)]
struct MEMORY_COMBINE_INFORMATION_EX {
    Handle: HANDLE,
    PagesCombined: usize,
    Flags: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> HANDLE;
    fn CloseHandle(hObject: HANDLE) -> i32;
    fn GetLastError() -> u32;
    fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: u32,
        dwShareMode: u32,
        lpSecurityAttributes: *mut std::ffi::c_void,
        dwCreationDisposition: u32,
        dwFlagsAndAttributes: u32,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    fn FlushFileBuffers(hFile: HANDLE) -> i32;
    fn GetLogicalDriveStringsW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
    fn GetDriveTypeW(lpRootPathName: *const u16) -> u32;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(ProcessHandle: HANDLE, DesiredAccess: u32, TokenHandle: *mut HANDLE) -> i32;
    fn LookupPrivilegeValueW(
        lpSystemName: *const u16,
        lpName: *const u16,
        lpLuid: *mut LUID,
    ) -> i32;
    fn AdjustTokenPrivileges(
        TokenHandle: HANDLE,
        DisableAllPrivileges: i32,
        NewState: *mut TOKEN_PRIVILEGES,
        BufferLength: u32,
        PreviousState: *mut TOKEN_PRIVILEGES,
        ReturnLength: *mut u32,
    ) -> i32;
    fn GetTokenInformation(
        TokenHandle: HANDLE,
        TokenInformationClass: u32,
        TokenInformation: *mut std::ffi::c_void,
        TokenInformationLength: u32,
        ReturnLength: *mut u32,
    ) -> i32;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtSetSystemInformation(
        SystemInformationClass: u32,
        SystemInformation: *mut std::ffi::c_void,
        SystemInformationLength: u32,
    ) -> NTSTATUS;
}

const SystemFileCacheInformationEx: u32 = 21;
const SystemMemoryListInformation: u32 = 80;
const SystemCombinePhysicalMemoryInformation: u32 = 130;
const SystemRegistryReconciliationInformation: u32 = 134;

const MemoryEmptyWorkingSets: u32 = 2;
const MemoryPurgeStandbyList: u32 = 3;
const MemoryPurgeLowPriorityStandbyList: u32 = 4;
const MemoryFlushModifiedList: u32 = 8;

pub fn is_elevated() -> bool {
    unsafe {
        let mut h_token: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h_token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut ret_len = 0;
        let res = GetTokenInformation(
            h_token,
            TokenElevation,
            &mut elevation as *mut _ as *mut std::ffi::c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(h_token);
        res != 0 && elevation.TokenIsElevated != 0
    }
}

unsafe fn enable_privilege(privilege_name: &str) -> bool {
    let mut h_token: HANDLE = ptr::null_mut();
    if OpenProcessToken(
        GetCurrentProcess(),
        TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
        &mut h_token,
    ) == 0
    {
        return false;
    }

    let wide_name: Vec<u16> = privilege_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut luid = LUID {
        LowPart: 0,
        HighPart: 0,
    };
    if LookupPrivilegeValueW(ptr::null(), wide_name.as_ptr(), &mut luid) == 0 {
        CloseHandle(h_token);
        return false;
    }

    let mut tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };

    let res = AdjustTokenPrivileges(
        h_token,
        0,
        &mut tp,
        size_of::<TOKEN_PRIVILEGES>() as u32,
        ptr::null_mut(),
        ptr::null_mut(),
    );
    let err = GetLastError();
    CloseHandle(h_token);
    res != 0 && err == 0
}

fn flush_volumes() -> Result<(), String> {
    unsafe {
        let mut buffer = [0u16; 512];
        let len = GetLogicalDriveStringsW(buffer.len() as u32, buffer.as_mut_ptr());
        if len == 0 {
            return Err("GetLogicalDriveStringsW failed".to_string());
        }

        let mut offset = 0;
        while offset < len as usize && buffer[offset] != 0 {
            let mut drive_str = Vec::new();
            while offset < len as usize && buffer[offset] != 0 {
                drive_str.push(buffer[offset]);
                offset += 1;
            }
            offset += 1; // skip null terminator
            drive_str.push(0);

            if GetDriveTypeW(drive_str.as_ptr()) == DRIVE_FIXED && drive_str.len() >= 2 {
                let letter = drive_str[0] as u8 as char;
                let volume_path = format!("\\\\.\\{}:\0", letter);
                let wide_path: Vec<u16> = volume_path.encode_utf16().collect();

                let h_vol = CreateFileW(
                    wide_path.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    ptr::null_mut(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    ptr::null_mut(),
                );

                if h_vol != INVALID_HANDLE_VALUE {
                    FlushFileBuffers(h_vol);
                    CloseHandle(h_vol);
                }
            }
        }
        Ok(())
    }
}

pub fn execute_clean(mask: u32) -> Result<(Vec<String>, Vec<(String, String)>), String> {
    unsafe {
        // Enable necessary privileges for memory management
        enable_privilege("SeIncreaseQuotaPrivilege");
        enable_privilege("SeProfileSingleProcessPrivilege");

        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        // 1. Working set (vista+)
        if (mask & REDUCT_WORKINGSET) != 0 {
            let cmd = MemoryEmptyWorkingSets;
            let status = NtSetSystemInformation(
                SystemMemoryListInformation,
                &cmd as *const _ as *mut std::ffi::c_void,
                size_of::<u32>() as u32,
            );
            if status >= 0 {
                succeeded.push("Working Set".to_string());
            } else {
                failed.push(("Working Set".to_string(), format!("NTSTATUS 0x{:08X}", status)));
            }
        }

        // 2. System file cache
        if (mask & REDUCT_SYSTEMFILECACHE) != 0 {
            let mut sfci: SYSTEM_FILECACHE_INFORMATION = std::mem::zeroed();
            sfci.MinimumWorkingSet = usize::MAX;
            sfci.MaximumWorkingSet = usize::MAX;
            let status = NtSetSystemInformation(
                SystemFileCacheInformationEx,
                &mut sfci as *mut _ as *mut std::ffi::c_void,
                size_of::<SYSTEM_FILECACHE_INFORMATION>() as u32,
            );
            if status >= 0 {
                succeeded.push("System File Cache".to_string());
            } else {
                failed.push((
                    "System File Cache".to_string(),
                    format!("NTSTATUS 0x{:08X}", status),
                ));
            }
        }

        // 3. Flush volume cache
        if (mask & REDUCT_MODIFIEDFILECACHE) != 0 {
            match flush_volumes() {
                Ok(_) => succeeded.push("Flush Volume Cache".to_string()),
                Err(e) => failed.push(("Flush Volume Cache".to_string(), e)),
            }
        }

        // 4. Modified page list
        if (mask & REDUCT_MODIFIEDLIST) != 0 {
            let cmd = MemoryFlushModifiedList;
            let status = NtSetSystemInformation(
                SystemMemoryListInformation,
                &cmd as *const _ as *mut std::ffi::c_void,
                size_of::<u32>() as u32,
            );
            if status >= 0 {
                succeeded.push("Modified Page List".to_string());
            } else {
                failed.push((
                    "Modified Page List".to_string(),
                    format!("NTSTATUS 0x{:08X}", status),
                ));
            }
        }

        // 5. Standby list
        if (mask & REDUCT_STANDBYLIST) != 0 {
            let cmd = MemoryPurgeStandbyList;
            let status = NtSetSystemInformation(
                SystemMemoryListInformation,
                &cmd as *const _ as *mut std::ffi::c_void,
                size_of::<u32>() as u32,
            );
            if status >= 0 {
                succeeded.push("Standby List".to_string());
            } else {
                failed.push(("Standby List".to_string(), format!("NTSTATUS 0x{:08X}", status)));
            }
        }

        // 6. Standby priority-0 list
        if (mask & REDUCT_STANDBYPRIORITY0LIST) != 0 {
            let cmd = MemoryPurgeLowPriorityStandbyList;
            let status = NtSetSystemInformation(
                SystemMemoryListInformation,
                &cmd as *const _ as *mut std::ffi::c_void,
                size_of::<u32>() as u32,
            );
            if status >= 0 {
                succeeded.push("Standby Priority 0 List".to_string());
            } else {
                failed.push((
                    "Standby Priority 0 List".to_string(),
                    format!("NTSTATUS 0x{:08X}", status),
                ));
            }
        }

        // 7. Flush registry cache (win8.1+)
        if (mask & REDUCT_REGISTRYCACHE) != 0 {
            let status = NtSetSystemInformation(
                SystemRegistryReconciliationInformation,
                ptr::null_mut(),
                0,
            );
            if status >= 0 {
                succeeded.push("Registry Cache".to_string());
            } else {
                failed.push(("Registry Cache".to_string(), format!("NTSTATUS 0x{:08X}", status)));
            }
        }

        // 8. Combine memory lists (win10+)
        if (mask & REDUCT_COMBINEMEMORYLISTS) != 0 {
            let mut combine_info: MEMORY_COMBINE_INFORMATION_EX = std::mem::zeroed();
            let status = NtSetSystemInformation(
                SystemCombinePhysicalMemoryInformation,
                &mut combine_info as *mut _ as *mut std::ffi::c_void,
                size_of::<MEMORY_COMBINE_INFORMATION_EX>() as u32,
            );
            if status >= 0 {
                succeeded.push("Combine Memory Lists".to_string());
            } else {
                failed.push((
                    "Combine Memory Lists".to_string(),
                    format!("NTSTATUS 0x{:08X}", status),
                ));
            }
        }

        Ok((succeeded, failed))
    }
}
