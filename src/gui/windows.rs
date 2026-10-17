use crate::cleaner::{
    clean_memory, REDUCT_COMBINEMEMORYLISTS, REDUCT_MASK_ALL, REDUCT_MASK_DEFAULT,
    REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST, REDUCT_REGISTRYCACHE, REDUCT_STANDBYLIST,
    REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE, REDUCT_WORKINGSET,
};
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
type HFONT = *mut std::ffi::c_void;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;

const WS_OVERLAPPED: u32 = 0x00000000;
const WS_CAPTION: u32 = 0x00C00000;
const WS_SYSMENU: u32 = 0x00080000;
const WS_MINIMIZEBOX: u32 = 0x00020000;
const WS_VISIBLE: u32 = 0x10000000;
const WS_CHILD: u32 = 0x40000000;
const WS_BORDER: u32 = 0x00800000;
const BS_PUSHBUTTON: u32 = 0x00000000;
const BS_DEFPUSHBUTTON: u32 = 0x00000001;
const SW_SHOW: i32 = 5;
const SW_HIDE: i32 = 0;

const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_SETFONT: u32 = 0x0030;
const WM_USER: u32 = 0x0400;
const WM_TRAYICON: u32 = WM_USER + 1;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONUP: u32 = 0x0205;

const MF_STRING: u32 = 0x00000000;
const MF_POPUP: u32 = 0x00000010;
const MF_CHECKED: u32 = 0x00000008;
const MF_UNCHECKED: u32 = 0x00000000;
const MF_SEPARATOR: u32 = 0x00000800;
const TPM_RIGHTBUTTON: u32 = 0x0002;

const COLOR_BTNFACE: u32 = 15;
const IDC_ARROW: *const u16 = 32512 as *const u16;
const DEFAULT_GUI_FONT: i32 = 17;

const NIM_ADD: u32 = 0x00000000;
const NIM_DELETE: u32 = 0x00000002;
const NIM_MODIFY: u32 = 0x00000001;
const NIF_MESSAGE: u32 = 0x00000001;
const NIF_ICON: u32 = 0x00000002;
const NIF_TIP: u32 = 0x00000004;

const HWND_TOPMOST: HWND = -1isize as HWND;
const HWND_NOTOPMOST: HWND = -2isize as HWND;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;

// ListView messages
const LVM_FIRST: u32 = 0x1000;
const LVM_INSERTCOLUMNW: u32 = LVM_FIRST + 97;
const LVM_SETCOLUMNWIDTH: u32 = LVM_FIRST + 30;
const LVM_INSERTITEMW: u32 = LVM_FIRST + 77;
const LVM_SETITEMTEXTW: u32 = LVM_FIRST + 116;
const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = LVM_FIRST + 54;
const LVS_EX_FULLROWSELECT: u32 = 0x00000020;
const LVS_EX_DOUBLEBUFFER: u32 = 0x00010000;
const LVS_REPORT: u32 = 0x0001;
const LVS_NOCOLUMNHEADER: u32 = 0x4000;
const LVS_SINGLESEL: u32 = 0x0004;
const LVCF_FMT: u32 = 0x0001;
const LVCF_WIDTH: u32 = 0x0002;
const LVCF_TEXT: u32 = 0x0004;
const LVIF_TEXT: u32 = 0x0001;
const LVCFMT_LEFT: i32 = 0x0000;
const LVCFMT_RIGHT: i32 = 0x0001;

// Menu and control IDs
const ID_CLEAN_BTN: usize = 101;
const ID_LISTVIEW: usize = 102;
const ID_STATUS_LABEL: usize = 103;

const IDM_FILE_SETTINGS: usize = 201;
const IDM_FILE_EXIT: usize = 202;
const IDM_VIEW_TOPMOST: usize = 203;
const IDM_HELP_WEBSITE: usize = 204;
const IDM_HELP_ABOUT: usize = 205;
const IDM_TRAY_SHOWHIDE: usize = 206;
const IDM_TRAY_CLEAN: usize = 207;

const IDM_REGION_WORKINGSET: usize = 301;
const IDM_REGION_FILECACHE: usize = 302;
const IDM_REGION_STANDBY0: usize = 303;
const IDM_REGION_STANDBY: usize = 304;
const IDM_REGION_MODIFIED: usize = 305;
const IDM_REGION_COMBINE: usize = 306;
const IDM_REGION_REGISTRY: usize = 307;
const IDM_REGION_VOLUME: usize = 308;
const IDM_REGION_ALL: usize = 309;
const IDM_REGION_DEFAULT: usize = 310;

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

#[repr(C)]
struct LVCOLUMNW {
    mask: u32,
    fmt: i32,
    cx: i32,
    pszText: *mut u16,
    cchTextMax: i32,
    iSubItem: i32,
    iImage: i32,
    iOrder: i32,
}

#[repr(C)]
struct LVITEMW {
    mask: u32,
    iItem: i32,
    iSubItem: i32,
    state: u32,
    stateMask: u32,
    pszText: *mut u16,
    cchTextMax: i32,
    iImage: i32,
    lParam: LPARAM,
    iIndent: i32,
    iGroupId: i32,
    cColumns: u32,
    puColumns: *mut u32,
    piColFmt: *mut i32,
    iGroup: i32,
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
    fn IsWindowVisible(hWnd: HWND) -> i32;
    fn UpdateWindow(hWnd: HWND) -> i32;
    fn SetForegroundWindow(hWnd: HWND) -> i32;
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
    fn CreateMenu() -> HMENU;
    fn CreatePopupMenu() -> HMENU;
    fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> i32;
    fn CheckMenuItem(hMenu: HMENU, uIDCheckItem: u32, uCheck: u32) -> u32;
    fn SetMenu(hWnd: HWND, hMenu: HMENU) -> i32;
    fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: u32,
        x: i32,
        y: i32,
        nReserved: i32,
        hWnd: HWND,
        prcRect: *const std::ffi::c_void,
    ) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn MessageBoxW(hWnd: HWND, lpText: *const u16, lpCaption: *const u16, uType: u32) -> i32;
    fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(i: i32) -> HFONT;
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

static mut CURRENT_MASK: u32 = REDUCT_MASK_DEFAULT;
static mut IS_ALWAYS_ON_TOP: bool = false;
static mut LAST_CLEAN_INSTANT: Option<Instant> = None;
static mut MAIN_MENU: HMENU = ptr::null_mut();

unsafe fn build_window_menu() -> HMENU {
    let hmenu_bar = CreateMenu();

    // 1. File Menu
    let hmenu_file = CreatePopupMenu();
    AppendMenuW(
        hmenu_file,
        MF_STRING,
        IDM_FILE_SETTINGS,
        to_wide("Settings\tF2").as_ptr(),
    );
    AppendMenuW(hmenu_file, MF_SEPARATOR, 0, ptr::null());
    AppendMenuW(hmenu_file, MF_STRING, IDM_FILE_EXIT, to_wide("Exit").as_ptr());
    AppendMenuW(hmenu_bar, MF_POPUP, hmenu_file as usize, to_wide("File").as_ptr());

    // 2. Clean Areas Menu (direct quick selection)
    let hmenu_areas = CreatePopupMenu();
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_WORKINGSET, to_wide("Working set").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_FILECACHE, to_wide("System file cache").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_STANDBY0, to_wide("Standby list (priority 0)").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_STANDBY, to_wide("Standby list (full)*").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_MODIFIED, to_wide("Modified page list*").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_REGISTRY, to_wide("Registry cache").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_COMBINE, to_wide("Combine memory lists").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_VOLUME, to_wide("Flush volume cache").as_ptr());
    AppendMenuW(hmenu_areas, MF_SEPARATOR, 0, ptr::null());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_DEFAULT, to_wide("Reset to default").as_ptr());
    AppendMenuW(hmenu_areas, MF_STRING, IDM_REGION_ALL, to_wide("Select all regions").as_ptr());
    AppendMenuW(hmenu_bar, MF_POPUP, hmenu_areas as usize, to_wide("Clean areas").as_ptr());

    // 3. View Menu
    let hmenu_view = CreatePopupMenu();
    AppendMenuW(hmenu_view, MF_STRING, IDM_VIEW_TOPMOST, to_wide("Always on top").as_ptr());
    AppendMenuW(hmenu_bar, MF_POPUP, hmenu_view as usize, to_wide("View").as_ptr());

    // 4. Help Menu
    let hmenu_help = CreatePopupMenu();
    AppendMenuW(hmenu_help, MF_STRING, IDM_HELP_WEBSITE, to_wide("Website").as_ptr());
    AppendMenuW(hmenu_help, MF_STRING, IDM_HELP_ABOUT, to_wide("About\tF1").as_ptr());
    AppendMenuW(hmenu_bar, MF_POPUP, hmenu_help as usize, to_wide("Help").as_ptr());

    hmenu_bar
}

unsafe fn update_menu_checks(hmenu: HMENU) {
    if hmenu.is_null() {
        return;
    }
    let check = |mask_bit: u32| {
        if (CURRENT_MASK & mask_bit) != 0 {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        }
    };

    CheckMenuItem(hmenu, IDM_REGION_WORKINGSET as u32, check(REDUCT_WORKINGSET));
    CheckMenuItem(hmenu, IDM_REGION_FILECACHE as u32, check(REDUCT_SYSTEMFILECACHE));
    CheckMenuItem(hmenu, IDM_REGION_STANDBY0 as u32, check(REDUCT_STANDBYPRIORITY0LIST));
    CheckMenuItem(hmenu, IDM_REGION_STANDBY as u32, check(REDUCT_STANDBYLIST));
    CheckMenuItem(hmenu, IDM_REGION_MODIFIED as u32, check(REDUCT_MODIFIEDLIST));
    CheckMenuItem(hmenu, IDM_REGION_REGISTRY as u32, check(REDUCT_REGISTRYCACHE));
    CheckMenuItem(hmenu, IDM_REGION_COMBINE as u32, check(REDUCT_COMBINEMEMORYLISTS));
    CheckMenuItem(hmenu, IDM_REGION_VOLUME as u32, check(REDUCT_MODIFIEDFILECACHE));

    CheckMenuItem(
        hmenu,
        IDM_VIEW_TOPMOST as u32,
        if IS_ALWAYS_ON_TOP { MF_CHECKED } else { MF_UNCHECKED },
    );
}

unsafe fn init_listview(hwnd_lv: HWND) {
    SendMessageW(
        hwnd_lv,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        0,
        (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as LPARAM,
    );

    // Column 0: Label (~125 px)
    let mut col = LVCOLUMNW {
        mask: LVCF_FMT | LVCF_WIDTH,
        fmt: LVCFMT_LEFT,
        cx: 135,
        pszText: ptr::null_mut(),
        cchTextMax: 0,
        iSubItem: 0,
        iImage: 0,
        iOrder: 0,
    };
    SendMessageW(hwnd_lv, LVM_INSERTCOLUMNW, 0, &col as *const _ as LPARAM);

    // Column 1: Value (~100 px, right aligned)
    col.fmt = LVCFMT_RIGHT;
    col.cx = 95;
    col.iSubItem = 1;
    SendMessageW(hwnd_lv, LVM_INSERTCOLUMNW, 1, &col as *const _ as LPARAM);

    // Insert 12 rows matching the classic Mem Reduct 3-group layout:
    let rows = [
        "Physical memory",
        "  Usage",
        "  Available",
        "  Total available",
        "Pagefile",
        "  Usage",
        "  Available",
        "  Total available",
        "System working set",
        "  Usage",
        "  Available",
        "  Total available",
    ];

    for (i, &name) in rows.iter().enumerate() {
        let mut item = LVITEMW {
            mask: LVIF_TEXT,
            iItem: i as i32,
            iSubItem: 0,
            state: 0,
            stateMask: 0,
            pszText: to_wide(name).as_mut_ptr(),
            cchTextMax: 0,
            iImage: 0,
            lParam: 0,
            iIndent: 0,
            iGroupId: 0,
            cColumns: 0,
            puColumns: ptr::null_mut(),
            piColFmt: ptr::null_mut(),
            iGroup: 0,
        };
        SendMessageW(hwnd_lv, LVM_INSERTITEMW, 0, &item as *const _ as LPARAM);
    }
}

unsafe fn update_listview_values(hwnd_lv: HWND) {
    if let Ok(info) = get_memory_info() {
        let set_val = |row: i32, text: &str| {
            let mut val_item = LVITEMW {
                mask: LVIF_TEXT,
                iItem: row,
                iSubItem: 1,
                state: 0,
                stateMask: 0,
                pszText: to_wide(text).as_mut_ptr(),
                cchTextMax: 0,
                iImage: 0,
                lParam: 0,
                iIndent: 0,
                iGroupId: 0,
                cColumns: 0,
                puColumns: ptr::null_mut(),
                piColFmt: ptr::null_mut(),
                iGroup: 0,
            };
            SendMessageW(hwnd_lv, LVM_SETITEMTEXTW, row as WPARAM, &val_item as *const _ as LPARAM);
        };

        // 1. Physical memory
        set_val(1, &format!("{}%", info.percent_physical));
        set_val(2, &format_bytes(info.avail_physical));
        set_val(3, &format_bytes(info.total_physical));

        // 2. Pagefile
        if info.total_pagefile > 0 {
            set_val(5, &format!("{}%", info.percent_pagefile));
            set_val(6, &format_bytes(info.avail_pagefile));
            set_val(7, &format_bytes(info.total_pagefile));
        } else {
            set_val(5, "N/A");
            set_val(6, "N/A");
            set_val(7, "N/A");
        }

        // 3. System working set / File cache
        if let Some(cached) = info.cached {
            let pct = if info.total_physical > 0 {
                (cached * 100) / info.total_physical
            } else {
                0
            };
            set_val(9, &format!("{}%", pct));
            set_val(10, &format_bytes(cached));
            set_val(11, &format_bytes(info.total_physical));
        } else {
            set_val(9, "N/A");
            set_val(10, "N/A");
            set_val(11, "N/A");
        }
    }
}

unsafe fn update_tray_icon(hwnd: HWND) {
    if let Ok(info) = get_memory_info() {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_TIP;

        let tip = format!(
            "Mem Reduct\nPhysical memory: {}%\nPagefile: {}%",
            info.percent_physical, info.percent_pagefile
        );
        let tip_wide = to_wide(&tip);
        for (i, &c) in tip_wide.iter().take(127).enumerate() {
            nid.szTip[i] = c;
        }

        Shell_NotifyIconW(NIM_MODIFY, &mut nid);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let hinst = GetModuleHandleW(ptr::null());
            let default_font = GetStockObject(DEFAULT_GUI_FONT);

            // 1. Build and attach Window Menu
            MAIN_MENU = build_window_menu();
            update_menu_checks(MAIN_MENU);
            SetMenu(hwnd, MAIN_MENU);

            // 2. Create ListView (IDC_LISTVIEW): exactly matching Mem Reduct
            let hwnd_lv = CreateWindowExW(
                0,
                to_wide("SysListView32").as_ptr(),
                ptr::null(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | LVS_REPORT | LVS_NOCOLUMNHEADER | LVS_SINGLESEL,
                8,
                8,
                244,
                218,
                hwnd,
                ID_LISTVIEW as HMENU,
                hinst,
                ptr::null_mut(),
            );
            SendMessageW(hwnd_lv, WM_SETFONT, default_font as WPARAM, 1);
            init_listview(hwnd_lv);

            // 3. Create Big Clean Memory Button (IDC_CLEAN_BTN)
            let hwnd_btn = CreateWindowExW(
                0,
                to_wide("BUTTON").as_ptr(),
                to_wide("Clean memory").as_ptr(),
                WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON,
                8,
                232,
                244,
                26,
                hwnd,
                ID_CLEAN_BTN as HMENU,
                hinst,
                ptr::null_mut(),
            );
            SendMessageW(hwnd_btn, WM_SETFONT, default_font as WPARAM, 1);

            // 4. Create Status Label (IDC_STATUS_LABEL)
            let hwnd_lbl = CreateWindowExW(
                0,
                to_wide("STATIC").as_ptr(),
                to_wide("Ready").as_ptr(),
                WS_CHILD | WS_VISIBLE,
                8,
                264,
                244,
                18,
                hwnd,
                ID_STATUS_LABEL as HMENU,
                hinst,
                ptr::null_mut(),
            );
            SendMessageW(hwnd_lbl, WM_SETFONT, default_font as WPARAM, 1);

            // 5. System Tray Icon
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

            // 6. Set timer every 1000ms
            SetTimer(hwnd, 1, 1000, None);
            update_listview_values(hwnd_lv);
            0
        }
        WM_TIMER => {
            let hwnd_lv = GetDlgItem(hwnd, ID_LISTVIEW as i32);
            update_listview_values(hwnd_lv);
            update_tray_icon(hwnd);

            // Check auto-reduct
            let config = Config::load();
            if config.autoreduct_enable {
                if let Ok(info) = get_memory_info() {
                    if info.percent_physical >= config.autoreduct_limit {
                        let can_clean = match LAST_CLEAN_INSTANT {
                            Some(t) => t.elapsed() >= Duration::from_secs(config.cooldown_seconds),
                            None => true,
                        };
                        if can_clean {
                            execute_gui_clean(hwnd);
                        }
                    }
                }
            }
            0
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            match id {
                ID_CLEAN_BTN => {
                    execute_gui_clean(hwnd);
                }
                IDM_REGION_WORKINGSET => {
                    CURRENT_MASK ^= REDUCT_WORKINGSET;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_FILECACHE => {
                    CURRENT_MASK ^= REDUCT_SYSTEMFILECACHE;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_STANDBY0 => {
                    CURRENT_MASK ^= REDUCT_STANDBYPRIORITY0LIST;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_STANDBY => {
                    CURRENT_MASK ^= REDUCT_STANDBYLIST;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_MODIFIED => {
                    CURRENT_MASK ^= REDUCT_MODIFIEDLIST;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_REGISTRY => {
                    CURRENT_MASK ^= REDUCT_REGISTRYCACHE;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_COMBINE => {
                    CURRENT_MASK ^= REDUCT_COMBINEMEMORYLISTS;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_VOLUME => {
                    CURRENT_MASK ^= REDUCT_MODIFIEDFILECACHE;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_DEFAULT => {
                    CURRENT_MASK = REDUCT_MASK_DEFAULT;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_REGION_ALL => {
                    CURRENT_MASK = REDUCT_MASK_ALL;
                    update_menu_checks(MAIN_MENU);
                }
                IDM_VIEW_TOPMOST => {
                    IS_ALWAYS_ON_TOP = !IS_ALWAYS_ON_TOP;
                    let insert_after = if IS_ALWAYS_ON_TOP {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    };
                    SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                    update_menu_checks(MAIN_MENU);
                }
                IDM_FILE_SETTINGS => {
                    MessageBoxW(
                        hwnd,
                        to_wide("Settings can be customized in memreduct.toml or via 'Clean areas' menu.").as_ptr(),
                        to_wide("Mem Reduct - Settings").as_ptr(),
                        0,
                    );
                }
                IDM_FILE_EXIT => {
                    PostQuitMessage(0);
                }
                IDM_HELP_WEBSITE => {
                    // Open website
                }
                IDM_HELP_ABOUT => {
                    MessageBoxW(
                        hwnd,
                        to_wide("Mem Reduct v3.5.3 (Rust)\n\nOriginal utility: (c) 2011-2026 Henry++\nRewritten in pure Rust for Windows & Linux.").as_ptr(),
                        to_wide("About Mem Reduct").as_ptr(),
                        0,
                    );
                }
                IDM_TRAY_SHOWHIDE => {
                    if IsWindowVisible(hwnd) != 0 {
                        ShowWindow(hwnd, SW_HIDE);
                    } else {
                        ShowWindow(hwnd, SW_SHOW);
                        SetForegroundWindow(hwnd);
                    }
                }
                IDM_TRAY_CLEAN => {
                    execute_gui_clean(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_TRAYICON => {
            let event = lparam as u32;
            if event == WM_LBUTTONDBLCLK {
                if IsWindowVisible(hwnd) != 0 {
                    ShowWindow(hwnd, SW_HIDE);
                } else {
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                }
            } else if event == WM_RBUTTONUP {
                let mut pt: POINT = std::mem::zeroed();
                GetCursorPos(&mut pt);
                let htray_menu = CreatePopupMenu();
                AppendMenuW(htray_menu, MF_STRING, IDM_TRAY_SHOWHIDE, to_wide("Show/Hide").as_ptr());
                AppendMenuW(htray_menu, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray_menu, MF_STRING, IDM_TRAY_CLEAN, to_wide("Clean memory").as_ptr());
                AppendMenuW(htray_menu, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray_menu, MF_STRING, IDM_HELP_ABOUT, to_wide("About").as_ptr());
                AppendMenuW(htray_menu, MF_STRING, IDM_FILE_EXIT, to_wide("Exit").as_ptr());

                SetForegroundWindow(hwnd);
                TrackPopupMenu(htray_menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, ptr::null());
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

unsafe fn execute_gui_clean(hwnd: HWND) {
    let hwnd_lbl = GetDlgItem(hwnd, ID_STATUS_LABEL as i32);
    let hwnd_lv = GetDlgItem(hwnd, ID_LISTVIEW as i32);

    SetWindowTextW(hwnd_lbl, to_wide("Cleaning memory...").as_ptr());

    match clean_memory(CURRENT_MASK) {
        Ok(rep) => {
            LAST_CLEAN_INSTANT = Some(Instant::now());
            let status_text = format!(
                "Memory was released with {} result.",
                format_bytes(rep.bytes_freed)
            );
            SetWindowTextW(hwnd_lbl, to_wide(&status_text).as_ptr());
            update_listview_values(hwnd_lv);
            update_tray_icon(hwnd);
        }
        Err(e) => {
            let err_text = format!("Error: {}", e);
            SetWindowTextW(hwnd_lbl, to_wide(&err_text).as_ptr());
        }
    }
}

pub fn run_gui() -> Result<(), String> {
    unsafe {
        let icce = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: 0x00000001 | 0x00000020, // ICC_LISTVIEW_CLASSES | ICC_PROGRESS_CLASS
        };
        InitCommonControlsEx(&icce);

        let hinst = GetModuleHandleW(ptr::null());
        let class_name = to_wide("MemReductClassicUI");

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: wnd_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        RegisterClassExW(&wc);

        let window_title = to_wide("Mem Reduct");
        // Compact dimensions matching original 224x214 dialog units (~275x335 pixels)
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            150,
            150,
            276,
            345,
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
