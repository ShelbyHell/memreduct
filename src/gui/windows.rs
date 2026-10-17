use crate::cleaner::{clean_memory, REDUCT_MASK_DEFAULT};
use crate::config::Config;
use crate::system::{format_bytes, get_memory_info};
use std::mem::size_of;
use std::ptr;
use std::time::{Duration, Instant};

type HWND = *mut std::ffi::c_void;
type HINSTANCE = *mut std::ffi::c_void;
type HICON = *mut std::ffi::c_void;
type HCURSOR = *mut std::ffi::c_void;
type HBRUSH = *mut std::ffi::c_void;
type HMENU = *mut std::ffi::c_void;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;

const WS_OVERLAPPED: u32 = 0x00000000;
const WS_CAPTION: u32 = 0x00C00000;
const WS_SYSMENU: u32 = 0x00080000;
const WS_MINIMIZEBOX: u32 = 0x00020000;
const WS_VISIBLE: u32 = 0x10000000;
const WS_CHILD: u32 = 0x40000000;
const BS_PUSHBUTTON: u32 = 0x00000000;
const SW_SHOW: i32 = 5;

const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_USER: u32 = 0x0400;
const WM_TRAYICON: u32 = WM_USER + 1;
const WM_LBUTTONDBLCLK: u32 = 0x0203;

const PBM_SETRANGE32: u32 = 0x0400 + 6;
const PBM_SETPOS: u32 = 0x0400 + 2;

const COLOR_WINDOW: u32 = 5;
const IDC_ARROW: *const u16 = 32512 as *const u16;

const NIM_ADD: u32 = 0x00000000;
const NIM_DELETE: u32 = 0x00000002;
const NIF_MESSAGE: u32 = 0x00000001;
const NIF_ICON: u32 = 0x00000002;
const NIF_TIP: u32 = 0x00000004;

const ID_BTN_CLEAN: usize = 1001;
const ID_LBL_STATUS: usize = 1002;
const ID_PB_PHYSICAL: usize = 1003;
const ID_LBL_PHYSICAL: usize = 1004;
const ID_PB_PAGEFILE: usize = 1005;
const ID_LBL_PAGEFILE: usize = 1006;
const ID_LBL_CACHE: usize = 1007;

#[repr(C)]
struct WNDCLASSEXW {
    cbSize: u32,
    style: u32,
    lpfnWndProc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HICON,
    hCursor: HCURSOR,
    hbrBackground: HBRUSH,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
    hIconSm: HICON,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct INITCOMMONCONTROLSEX {
    dwSize: u32,
    dwICC: u32,
}

#[repr(C)]
struct NOTIFYICONDATAW {
    cbSize: u32,
    hWnd: HWND,
    uID: u32,
    uFlags: u32,
    uCallbackMessage: u32,
    hIcon: HICON,
    szTip: [u16; 128],
    dwState: u32,
    dwStateMask: u32,
    szInfo: [u16; 256],
    uTimeoutOrVersion: u32,
    szInfoTitle: [u16; 64],
    dwInfoFlags: u32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(lpWndClass: *const WNDCLASSEXW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut std::ffi::c_void,
    ) -> HWND;
    fn DefWindowProcW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: HWND) -> i32;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    fn PostQuitMessage(nExitCode: i32);
    fn SetTimer(
        hWnd: HWND,
        nIDEvent: usize,
        uElapse: u32,
        lpTimerFunc: Option<unsafe extern "system" fn(HWND, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(hWnd: HWND, uIDEvent: usize) -> i32;
    fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> i32;
    fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    fn GetDlgItem(hDlg: HWND, nIDDlgItem: i32) -> HWND;
}

#[link(name = "comctl32")]
extern "system" {
    fn InitCommonControlsEx(picce: *const INITCOMMONCONTROLSEX) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(dwMessage: u32, lpData: *mut NOTIFYICONDATAW) -> i32;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

static mut HWND_MAIN: HWND = ptr::null_mut();
static mut LAST_CLEAN_TIME: Option<Instant> = None;

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            HWND_MAIN = hwnd;
            let hinst = GetModuleHandleW(ptr::null());

            // Static: Physical Memory Label
            let w_phys = to_wide("Physical memory: 0%");
            CreateWindowExW(
                0,
                to_wide("STATIC").as_ptr(),
                w_phys.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                15,
                15,
                290,
                20,
                hwnd,
                ID_LBL_PHYSICAL as HMENU,
                hinst,
                ptr::null_mut(),
            );

            // Progress bar: Physical
            let pb_phys = CreateWindowExW(
                0,
                to_wide("msctls_progress32").as_ptr(),
                ptr::null(),
                WS_CHILD | WS_VISIBLE,
                15,
                38,
                275,
                20,
                hwnd,
                ID_PB_PHYSICAL as HMENU,
                hinst,
                ptr::null_mut(),
            );
            SendMessageW(pb_phys, PBM_SETRANGE32, 0, 100);

            // Static: Page file Label
            let w_pf = to_wide("Page file / Swap: 0%");
            CreateWindowExW(
                0,
                to_wide("STATIC").as_ptr(),
                w_pf.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                15,
                68,
                290,
                20,
                hwnd,
                ID_LBL_PAGEFILE as HMENU,
                hinst,
                ptr::null_mut(),
            );

            // Progress bar: Page file
            let pb_pf = CreateWindowExW(
                0,
                to_wide("msctls_progress32").as_ptr(),
                ptr::null(),
                WS_CHILD | WS_VISIBLE,
                15,
                91,
                275,
                20,
                hwnd,
                ID_PB_PAGEFILE as HMENU,
                hinst,
                ptr::null_mut(),
            );
            SendMessageW(pb_pf, PBM_SETRANGE32, 0, 100);

            // Static: Cache info
            let w_cache = to_wide("File cache: N/A");
            CreateWindowExW(
                0,
                to_wide("STATIC").as_ptr(),
                w_cache.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                15,
                122,
                290,
                20,
                hwnd,
                ID_LBL_CACHE as HMENU,
                hinst,
                ptr::null_mut(),
            );

            // Button: Clean memory
            let w_clean = to_wide("Clean memory");
            CreateWindowExW(
                0,
                to_wide("BUTTON").as_ptr(),
                w_clean.as_ptr(),
                WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                15,
                155,
                275,
                32,
                hwnd,
                ID_BTN_CLEAN as HMENU,
                hinst,
                ptr::null_mut(),
            );

            // Static: Status / Log label
            let w_status = to_wide("Ready");
            CreateWindowExW(
                0,
                to_wide("STATIC").as_ptr(),
                w_status.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                15,
                198,
                290,
                35,
                hwnd,
                ID_LBL_STATUS as HMENU,
                hinst,
                ptr::null_mut(),
            );

            // Add tray icon
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            let tip = to_wide("Mem Reduct (Rust)");
            for (i, &c) in tip.iter().take(127).enumerate() {
                nid.szTip[i] = c;
            }
            Shell_NotifyIconW(NIM_ADD, &mut nid);

            // Timer every 1 second
            SetTimer(hwnd, 1, 1000, None);
            update_stats(hwnd);
            0
        }
        WM_TIMER => {
            update_stats(hwnd);
            0
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            if id == ID_BTN_CLEAN {
                do_clean(hwnd);
            }
            0
        }
        WM_TRAYICON => {
            if lparam as u32 == WM_LBUTTONDBLCLK {
                ShowWindow(hwnd, SW_SHOW);
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &mut nid);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn update_stats(hwnd: HWND) {
    if let Ok(info) = get_memory_info() {
        let lbl_phys = GetDlgItem(hwnd, ID_LBL_PHYSICAL as i32);
        let pb_phys = GetDlgItem(hwnd, ID_PB_PHYSICAL as i32);
        let lbl_pf = GetDlgItem(hwnd, ID_LBL_PAGEFILE as i32);
        let pb_pf = GetDlgItem(hwnd, ID_PB_PAGEFILE as i32);
        let lbl_cache = GetDlgItem(hwnd, ID_LBL_CACHE as i32);

        // Update Physical
        let text_phys = format!(
            "Physical memory: {}% ({} / {})",
            info.percent_physical,
            format_bytes(info.used_physical),
            format_bytes(info.total_physical)
        );
        SetWindowTextW(lbl_phys, to_wide(&text_phys).as_ptr());
        SendMessageW(pb_phys, PBM_SETPOS, info.percent_physical as WPARAM, 0);

        // Update Pagefile
        if info.total_pagefile > 0 {
            let text_pf = format!(
                "Page file: {}% ({} / {})",
                info.percent_pagefile,
                format_bytes(info.used_pagefile),
                format_bytes(info.total_pagefile)
            );
            SetWindowTextW(lbl_pf, to_wide(&text_pf).as_ptr());
            SendMessageW(pb_pf, PBM_SETPOS, info.percent_pagefile as WPARAM, 0);
        }

        // Update Cache
        let cached_str = info.cached.map(format_bytes).unwrap_or_else(|| "N/A".into());
        let text_cache = format!("File cache: {}", cached_str);
        SetWindowTextW(lbl_cache, to_wide(&text_cache).as_ptr());

        // Check auto-reduction
        let config = Config::load();
        if config.autoreduct_enable && info.percent_physical >= config.autoreduct_limit {
            let can_clean = match LAST_CLEAN_TIME {
                Some(t) => t.elapsed() >= Duration::from_secs(config.cooldown_seconds),
                None => true,
            };
            if can_clean {
                do_clean(hwnd);
            }
        }
    }
}

unsafe fn do_clean(hwnd: HWND) {
    let config = Config::load();
    let mask = if config.reduct_mask != 0 {
        config.reduct_mask
    } else {
        REDUCT_MASK_DEFAULT
    };

    let lbl_status = GetDlgItem(hwnd, ID_LBL_STATUS as i32);
    SetWindowTextW(lbl_status, to_wide("Cleaning memory...").as_ptr());

    match clean_memory(mask) {
        Ok(rep) => {
            LAST_CLEAN_TIME = Some(Instant::now());
            let status_text = format!(
                "Cleaned: {} freed ({}% -> {}%)\nin {:.1?}",
                format_bytes(rep.bytes_freed),
                rep.mem_before.percent_physical,
                rep.mem_after.percent_physical,
                rep.duration
            );
            SetWindowTextW(lbl_status, to_wide(&status_text).as_ptr());
            update_stats(hwnd);
        }
        Err(e) => {
            let err_text = format!("Error: {}", e);
            SetWindowTextW(lbl_status, to_wide(&err_text).as_ptr());
        }
    }
}

pub fn run_gui() -> Result<(), String> {
    unsafe {
        let icce = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: 0x00000020, // ICC_PROGRESS_CLASS
        };
        InitCommonControlsEx(&icce);

        let hinst = GetModuleHandleW(ptr::null());
        let class_name = to_wide("MemReductRustUI");

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: wnd_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        RegisterClassExW(&wc);

        let window_title = to_wide("Mem Reduct");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            150,
            150,
            320,
            285,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err("Failed to create main window".to_string());
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}
