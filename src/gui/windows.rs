#![allow(dead_code, static_mut_refs)]

use crate::cleaner::{
    clean_memory, REDUCT_COMBINEMEMORYLISTS, REDUCT_MASK_DEFAULT,
    REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST, REDUCT_REGISTRYCACHE, REDUCT_STANDBYLIST,
    REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE, REDUCT_WORKINGSET,
};
use crate::config::{Config, format_hotkey};
use crate::system::{format_bytes, get_memory_info};
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

type HWND = *mut std::ffi::c_void;
type HINSTANCE = *mut std::ffi::c_void;
type HICON = *mut std::ffi::c_void;
type HCURSOR = *mut std::ffi::c_void;
type HBRUSH = *mut std::ffi::c_void;
type HPEN = *mut std::ffi::c_void;
type HMENU = *mut std::ffi::c_void;
type HFONT = *mut std::ffi::c_void;
type HDC = *mut std::ffi::c_void;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;

const WS_OVERLAPPED: u32 = 0x00000000;
const WS_CAPTION: u32 = 0x00C00000;
const WS_SYSMENU: u32 = 0x00080000;
const WS_MINIMIZEBOX: u32 = 0x00020000;
const WS_VISIBLE: u32 = 0x10000000;
const SW_SHOW: i32 = 5;
const SW_HIDE: i32 = 0;
const SW_MINIMIZE: i32 = 6;
const CW_USEDEFAULT: i32 = 0x80000000u32 as i32;

const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_NCCREATE: u32 = 0x0081;
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const WM_KEYDOWN: u32 = 0x0100;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_COMMAND: u32 = 0x0111;
const WM_SYSCOMMAND: u32 = 0x0112;
const WM_TIMER: u32 = 0x0113;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_USER: u32 = 0x0400;
const WM_TRAYICON: u32 = WM_USER + 1;
const WM_CLEAN_FINISHED: u32 = WM_USER + 2;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_HOTKEY: u32 = 0x0312;

const SC_MINIMIZE: usize = 0xF020;
const HTCAPTION: isize = 2;

const ID_HOTKEY_CLEAN: i32 = 1337;
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_WIN: u32 = 0x0008;
const MOD_NOREPEAT: u32 = 0x4000;
const VK_F1: u32 = 0x70;

const MF_STRING: u32 = 0x00000000;
const MF_SEPARATOR: u32 = 0x00000800;
const MF_CHECKED: u32 = 0x00000008;
const MF_UNCHECKED: u32 = 0x00000000;
const MF_POPUP: u32 = 0x00000010;
const TPM_RIGHTBUTTON: u32 = 0x0002;

const IDC_ARROW: *const u16 = 32512 as *const u16;
const TRANSPARENT: i32 = 1;
const SRCCOPY: u32 = 0x00CC0020;
const PS_SOLID: i32 = 0;

const DT_LEFT: u32 = 0x00000000;
const DT_CENTER: u32 = 0x00000001;
const DT_RIGHT: u32 = 0x00000002;
const DT_VCENTER: u32 = 0x00000004;
const DT_SINGLELINE: u32 = 0x00000020;
const TME_LEAVE: u32 = 0x00000002;

const NIM_ADD: u32 = 0x00000000;
const NIM_DELETE: u32 = 0x00000002;
const NIM_MODIFY: u32 = 0x00000001;
const NIF_MESSAGE: u32 = 0x00000001;
const NIF_ICON: u32 = 0x00000002;
const NIF_TIP: u32 = 0x00000004;
const NIF_INFO: u32 = 0x00000010;
const NIIF_INFO: u32 = 0x00000001;
const NIIF_NOSOUND: u32 = 0x00000010;

const HWND_TOPMOST: HWND = -1isize as HWND;
const HWND_NOTOPMOST: HWND = -2isize as HWND;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;

const IDM_TRAY_SHOWHIDE: usize = 101;
const IDM_TRAY_CLEAN: usize = 102;
const IDM_TRAY_EXIT: usize = 103;
const IDM_TRAY_SETTINGS: usize = 104;

// Clean Areas menu IDs (201..208)
const IDM_AREA_WORKINGSET: usize = 201;
const IDM_AREA_SYSTEMFILECACHE: usize = 202;
const IDM_AREA_MODIFIEDFILECACHE: usize = 203;
const IDM_AREA_STANDBY0: usize = 204;
const IDM_AREA_STANDBY: usize = 205;
const IDM_AREA_MODIFIED: usize = 206;
const IDM_AREA_REGISTRY: usize = 207;
const IDM_AREA_COMBINE: usize = 208;

// Clean when above menu IDs (220..226)
const IDM_ABOVE_DISABLED: usize = 220;
const IDM_ABOVE_70: usize = 221;
const IDM_ABOVE_75: usize = 222;
const IDM_ABOVE_80: usize = 223;
const IDM_ABOVE_85: usize = 224;
const IDM_ABOVE_90: usize = 225;
const IDM_ABOVE_95: usize = 226;

// Clean every menu IDs (230..235)
const IDM_EVERY_DISABLED: usize = 230;
const IDM_EVERY_15: usize = 231;
const IDM_EVERY_30: usize = 232;
const IDM_EVERY_45: usize = 233;
const IDM_EVERY_60: usize = 234;
const IDM_EVERY_120: usize = 235;

#[repr(C)]
#[allow(non_snake_case)]
struct ICONINFO {
    fIcon: i32,
    xHotspot: u32,
    yHotspot: u32,
    hbmMask: *mut std::ffi::c_void,
    hbmColor: *mut std::ffi::c_void,
}

#[repr(C)]
#[allow(non_snake_case)]
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
#[allow(non_snake_case)]
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
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[allow(non_snake_case)]
struct PAINTSTRUCT {
    hdc: HDC,
    fErase: i32,
    rcPaint: RECT,
    fRestore: i32,
    fIncUpdate: i32,
    rgbReserved: [u8; 32],
}

#[repr(C)]
#[allow(non_snake_case)]
struct TRACKMOUSEEVENT {
    cbSize: u32,
    dwFlags: u32,
    hwndTrack: HWND,
    dwHoverTime: u32,
}

#[repr(C)]
#[allow(non_snake_case)]
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

#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
    fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
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
    fn PostMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> i32;
    fn SendMessageW(hWnd: HWND, Msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn ReleaseCapture() -> i32;
    fn DestroyWindow(hWnd: HWND) -> i32;
    fn PostQuitMessage(nExitCode: i32);
    fn SetTimer(
        hWnd: HWND,
        nIDEvent: usize,
        uElapse: u32,
        lpTimerFunc: Option<unsafe extern "system" fn(HWND, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(hWnd: HWND, uIDEvent: usize) -> i32;
    fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> i32;
    fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: i32) -> i32;
    fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn DrawTextW(hdc: HDC, lpchText: *const u16, cchText: i32, lprc: *mut RECT, format: u32) -> i32;
    fn TrackMouseEvent(lpEventTrack: *mut TRACKMOUSEEVENT) -> i32;
    fn CreatePopupMenu() -> HMENU;
    fn AppendMenuW(hMenu: HMENU, uFlags: u32, uIDNewItem: usize, lpNewItem: *const u16) -> i32;
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
    fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    fn GetDC(hWnd: HWND) -> HDC;
    fn ReleaseDC(hWnd: HWND, hDC: HDC) -> i32;
    fn CreateIconIndirect(piconinfo: *mut ICONINFO) -> HICON;
    fn DestroyIcon(hIcon: HICON) -> i32;
    fn RegisterHotKey(hWnd: HWND, id: i32, fsModifiers: u32, vk: u32) -> i32;
    fn UnregisterHotKey(hWnd: HWND, id: i32) -> i32;
    fn MessageBeep(uType: u32) -> i32;
    fn DestroyMenu(hMenu: HMENU) -> i32;
    fn GetKeyState(nVirtKey: i32) -> i16;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: HDC) -> HDC;
    fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> *mut std::ffi::c_void;
    fn CreateBitmap(nWidth: i32, nHeight: i32, nPlanes: u32, nBitCount: u32, lpBits: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    fn SelectObject(hdc: HDC, h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn DeleteObject(ho: *mut std::ffi::c_void) -> i32;
    fn DeleteDC(hdc: HDC) -> i32;
    fn BitBlt(hdc: HDC, x: i32, y: i32, cx: i32, cy: i32, hdcSrc: HDC, x1: i32, y1: i32, rop: u32) -> i32;
    fn CreateSolidBrush(color: u32) -> HBRUSH;
    fn CreatePen(iStyle: i32, cWidth: i32, color: u32) -> HPEN;
    fn SetTextColor(hdc: HDC, color: u32) -> u32;
    fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    fn RoundRect(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32, width: i32, height: i32) -> i32;
    fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn CreateFontW(
        cHeight: i32,
        cWidth: i32,
        cEscapement: i32,
        cOrientation: i32,
        cWeight: i32,
        bItalic: u32,
        bUnderline: u32,
        bStrikeOut: u32,
        iCharSet: u32,
        iOutPrecision: u32,
        iClipPrecision: u32,
        iQuality: u32,
        iPitchAndFamily: u32,
        pszFaceName: *const u16,
    ) -> HFONT;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(dwMessage: u32, lpData: *mut NOTIFYICONDATAW) -> i32;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Overview,
    Settings,
}

static mut ACTIVE_TAB: ActiveTab = ActiveTab::Overview;
static mut HOVER_TARGET: Option<ClickTarget> = None;
static mut CURRENT_MASK: u32 = REDUCT_MASK_DEFAULT;
static mut LAST_CLEAN_TIME: Option<Instant> = None;
static mut LAST_CLEAN_STATUS: Option<String> = None;
static mut IS_MOUSE_TRACKED: bool = false;
static mut CURRENT_TRAY_ICON: HICON = ptr::null_mut();
static IS_CLEANING: AtomicBool = AtomicBool::new(false);
static mut IS_RECORDING_HOTKEY: bool = false;

// Fonts
static mut FONT_HERO: HFONT = ptr::null_mut();
static mut FONT_TITLE: HFONT = ptr::null_mut();
static mut FONT_SUBTITLE: HFONT = ptr::null_mut();
static mut FONT_BODY: HFONT = ptr::null_mut();
static mut FONT_BTN: HFONT = ptr::null_mut();
static mut FONT_SMALL: HFONT = ptr::null_mut();

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ClickTarget {
    TabOverview,
    TabSettings,
    BtnMinimize,
    BtnClose,
    CleanButton,
    HotkeyBanner,
    Chip(u32),
    // Settings toggles
    SettingAlwaysOnTop,
    SettingAutostart,
    SettingStartMinimized,
    SettingBalloonNotifications,
    SettingNotificationSound,
    SettingHotkey,
    SettingHotkeyRecord,
    SettingHotkeyReset,
    SettingAutoToggle,
    SettingLimit(u8),
    SettingIntervalToggle,
    SettingInterval(u64),
    SettingReset,
    SettingSave,
}

unsafe fn create_fonts() {
    let font_name = to_wide("Segoe UI");
    FONT_HERO = CreateFontW(34, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_TITLE = CreateFontW(17, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_SUBTITLE = CreateFontW(13, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_BODY = CreateFontW(13, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_BTN = CreateFontW(14, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_SMALL = CreateFontW(11, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
}

unsafe fn destroy_fonts() {
    DeleteObject(FONT_HERO as *mut _);
    DeleteObject(FONT_TITLE as *mut _);
    DeleteObject(FONT_SUBTITLE as *mut _);
    DeleteObject(FONT_BODY as *mut _);
    DeleteObject(FONT_BTN as *mut _);
    DeleteObject(FONT_SMALL as *mut _);
}

unsafe fn draw_rect_solid(hdc: HDC, x: i32, y: i32, w: i32, h: i32, color: u32) {
    let rc = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let br = CreateSolidBrush(color);
    FillRect(hdc, &rc, br);
    DeleteObject(br as *mut _);
}

unsafe fn draw_rounded_card(
    hdc: HDC,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    bg_color: u32,
    border_color: u32,
) {
    let brush = CreateSolidBrush(bg_color);
    let pen = CreatePen(PS_SOLID, 1, border_color);

    let old_brush = SelectObject(hdc, brush as *mut _);
    let old_pen = SelectObject(hdc, pen as *mut _);

    RoundRect(hdc, x, y, x + w, y + h, radius, radius);

    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    DeleteObject(brush as *mut _);
    DeleteObject(pen as *mut _);
}

unsafe fn draw_pill_gauge(
    hdc: HDC,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    percent: u8,
    fill_color: u32,
    bg_color: u32,
) {
    draw_rounded_card(hdc, x, y, w, h, h, bg_color, bg_color);
    let filled_w = ((w as f64 * (percent as f64 / 100.0)).round() as i32).max(h).min(w);
    if percent > 0 {
        draw_rounded_card(hdc, x, y, filled_w, h, h, fill_color, fill_color);
    }
}

unsafe fn draw_string(
    hdc: HDC,
    text: &str,
    font: HFONT,
    color: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    alignment: u32,
) {
    let wide = to_wide(text);
    let old_font = SelectObject(hdc, font as *mut _);
    SetTextColor(hdc, color);
    SetBkMode(hdc, TRANSPARENT);

    let mut rc = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    DrawTextW(hdc, wide.as_ptr(), -1, &mut rc, alignment | DT_SINGLELINE);
    SelectObject(hdc, old_font);
}

unsafe fn render_modern_gui(hdc: HDC, w: i32, h: i32, config: &Config) {
    // 1. Window Background
    draw_rect_solid(hdc, 0, 0, w, h, rgb(15, 17, 23));

    // 2. Header Bar
    // Logo and App Name
    draw_string(hdc, "⚡ Mem Reduct", FONT_TITLE, rgb(255, 255, 255), 14, 12, 110, 24, DT_LEFT | DT_VCENTER);

    // Segmented Navigation Tabs: [ Monitor ]  [ Settings ]
    let seg_w = 176;
    let seg_h = 28;
    let seg_x = (w - seg_w) / 2;
    let seg_y = 10;
    draw_rounded_card(hdc, seg_x, seg_y, seg_w, seg_h, 8, rgb(22, 25, 34), rgb(38, 43, 58));

    let tab1_w = 84;
    let tab1_x = seg_x + 2;
    let tab2_x = seg_x + 88;
    let tab_h = 24;
    let tab_y = seg_y + 2;

    // Tab 1: Monitor
    let tab1_active = ACTIVE_TAB == ActiveTab::Overview;
    let tab1_hover = HOVER_TARGET == Some(ClickTarget::TabOverview);
    let tab1_bg = if tab1_active {
        rgb(99, 102, 241)
    } else if tab1_hover {
        rgb(34, 38, 52)
    } else {
        rgb(22, 25, 34)
    };
    let tab1_tc = if tab1_active { rgb(255, 255, 255) } else { rgb(148, 163, 184) };
    draw_rounded_card(hdc, tab1_x, tab_y, tab1_w, tab_h, 6, tab1_bg, tab1_bg);
    draw_string(hdc, "Monitor", FONT_SUBTITLE, tab1_tc, tab1_x, tab_y, tab1_w, tab_h, DT_CENTER | DT_VCENTER);

    // Tab 2: Settings
    let tab2_active = ACTIVE_TAB == ActiveTab::Settings;
    let tab2_hover = HOVER_TARGET == Some(ClickTarget::TabSettings);
    let tab2_bg = if tab2_active {
        rgb(99, 102, 241)
    } else if tab2_hover {
        rgb(34, 38, 52)
    } else {
        rgb(22, 25, 34)
    };
    let tab2_tc = if tab2_active { rgb(255, 255, 255) } else { rgb(148, 163, 184) };
    draw_rounded_card(hdc, tab2_x, tab_y, tab1_w, tab_h, 6, tab2_bg, tab2_bg);
    draw_string(hdc, "Settings", FONT_SUBTITLE, tab2_tc, tab2_x, tab_y, tab1_w, tab_h, DT_CENTER | DT_VCENTER);

    // Window Control Buttons: [ ─ ]  [ ✕ ]
    let is_min_hover = HOVER_TARGET == Some(ClickTarget::BtnMinimize);
    let min_bg = if is_min_hover { rgb(38, 42, 56) } else { rgb(24, 27, 36) };
    let min_border = if is_min_hover { rgb(99, 102, 241) } else { rgb(45, 50, 68) };
    let min_text = if is_min_hover { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
    draw_rounded_card(hdc, w - 66, 11, 26, 26, 8, min_bg, min_border);
    draw_string(hdc, "─", FONT_SUBTITLE, min_text, w - 66, 11, 26, 26, DT_CENTER | DT_VCENTER);

    let is_cls_hover = HOVER_TARGET == Some(ClickTarget::BtnClose);
    let cls_bg = if is_cls_hover { rgb(220, 38, 38) } else { rgb(24, 27, 36) };
    let cls_border = if is_cls_hover { rgb(239, 68, 68) } else { rgb(45, 50, 68) };
    let cls_text = if is_cls_hover { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
    draw_rounded_card(hdc, w - 36, 11, 26, 26, 8, cls_bg, cls_border);
    draw_string(hdc, "✕", FONT_SUBTITLE, cls_text, w - 36, 11, 26, 26, DT_CENTER | DT_VCENTER);

    // Divider
    draw_rect_solid(hdc, 14, 46, w - 28, 1, rgb(30, 34, 46));

    match ACTIVE_TAB {
        ActiveTab::Overview => {
            render_tab_overview(hdc, w, h, config);
        }
        ActiveTab::Settings => {
            render_tab_settings(hdc, w, h, config);
        }
    }
}

unsafe fn render_tab_overview(hdc: HDC, w: i32, _h: i32, config: &Config) {
    let info = get_memory_info().unwrap_or_default();

    // 1. Hero Card: Physical RAM
    let card_x = 16;
    let card_y = 56;
    let card_w = w - 32;
    let card_h = 110;
    draw_rounded_card(hdc, card_x, card_y, card_w, card_h, 14, rgb(24, 27, 36), rgb(38, 43, 58));

    draw_string(hdc, "PHYSICAL MEMORY", FONT_SMALL, rgb(148, 163, 184), card_x + 16, card_y + 12, 160, 16, DT_LEFT);

    let p_color = if info.percent_physical >= 90 {
        rgb(244, 63, 94) // Red / Danger
    } else if info.percent_physical >= 70 {
        rgb(251, 191, 36) // Amber / Warning
    } else {
        rgb(52, 211, 153) // Mint Green / Optimal
    };

    draw_string(
        hdc,
        &format!("{}%", info.percent_physical),
        FONT_HERO,
        p_color,
        card_x + 16,
        card_y + 30,
        100,
        42,
        DT_LEFT | DT_VCENTER,
    );

    // Status Badge
    let (badge_txt, badge_bg, badge_tc) = if info.percent_physical >= 90 {
        ("CRITICAL", rgb(60, 20, 25), rgb(244, 63, 94))
    } else if info.percent_physical >= 70 {
        ("ELEVATED", rgb(55, 40, 15), rgb(251, 191, 36))
    } else {
        ("OPTIMAL", rgb(18, 50, 36), rgb(52, 211, 153))
    };
    draw_rounded_card(hdc, card_x + 118, card_y + 40, 68, 20, 6, badge_bg, badge_bg);
    draw_string(hdc, badge_txt, FONT_SMALL, badge_tc, card_x + 118, card_y + 40, 68, 20, DT_CENTER | DT_VCENTER);

    let stats_text = format!(
        "Used: {}\nFree: {}\nTotal: {}",
        format_bytes(info.used_physical),
        format_bytes(info.avail_physical),
        format_bytes(info.total_physical)
    );
    draw_string(
        hdc,
        &stats_text,
        FONT_SMALL,
        rgb(203, 213, 225),
        card_x + card_w - 140,
        card_y + 20,
        125,
        50,
        DT_RIGHT,
    );

    draw_pill_gauge(
        hdc,
        card_x + 16,
        card_y + 84,
        card_w - 32,
        10,
        info.percent_physical,
        p_color,
        rgb(36, 41, 56),
    );

    // 2. Dual Sub-cards: Pagefile and Cache
    let sub_y = 176;
    let sub_w = (card_w - 10) / 2;
    let sub_h = 78;

    // Left Card: Pagefile / Swap
    draw_rounded_card(hdc, card_x, sub_y, sub_w, sub_h, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "PAGEFILE / SWAP", FONT_SMALL, rgb(148, 163, 184), card_x + 12, sub_y + 8, sub_w - 24, 14, DT_LEFT);
    let pf_color = if info.percent_pagefile >= 80 { rgb(245, 158, 11) } else { rgb(59, 130, 246) };
    draw_string(
        hdc,
        &format!("{}%", info.percent_pagefile),
        FONT_TITLE,
        pf_color,
        card_x + 12,
        sub_y + 26,
        60,
        22,
        DT_LEFT,
    );
    draw_string(
        hdc,
        &format!("{}/{}", format_bytes(info.used_pagefile), format_bytes(info.total_pagefile)),
        FONT_SMALL,
        rgb(203, 213, 225),
        card_x + 12,
        sub_y + 50,
        sub_w - 24,
        14,
        DT_LEFT,
    );

    // Right Card: System Cache
    draw_rounded_card(hdc, card_x + sub_w + 10, sub_y, sub_w, sub_h, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "STANDBY & CACHE", FONT_SMALL, rgb(148, 163, 184), card_x + sub_w + 22, sub_y + 8, sub_w - 24, 14, DT_LEFT);
    let cached_str = info.cached.map(format_bytes).unwrap_or_else(|| "N/A".into());
    draw_string(
        hdc,
        &cached_str,
        FONT_TITLE,
        rgb(168, 85, 247),
        card_x + sub_w + 22,
        sub_y + 26,
        sub_w - 24,
        22,
        DT_LEFT,
    );
    let buffers_str = info.buffers.map(format_bytes).unwrap_or_else(|| "0 B".into());
    draw_string(
        hdc,
        &format!("Buffers: {}", buffers_str),
        FONT_SMALL,
        rgb(203, 213, 225),
        card_x + sub_w + 22,
        sub_y + 50,
        sub_w - 24,
        14,
        DT_LEFT,
    );

    // 3. Areas to Clean Chips Grid
    let chips_y = 264;
    draw_string(hdc, "AREAS TO REDUCE (CLICK TO TOGGLE)", FONT_SMALL, rgb(148, 163, 184), card_x, chips_y, card_w, 14, DT_LEFT);

    let regions = [
        (REDUCT_WORKINGSET, "Working Set"),
        (REDUCT_SYSTEMFILECACHE, "System File Cache"),
        (REDUCT_STANDBYPRIORITY0LIST, "Standby (Prio 0)"),
        (REDUCT_STANDBYLIST, "Standby (Full)*"),
        (REDUCT_MODIFIEDLIST, "Modified List*"),
        (REDUCT_REGISTRYCACHE, "Registry Cache"),
        (REDUCT_COMBINEMEMORYLISTS, "Combine Memory"),
        (REDUCT_MODIFIEDFILECACHE, "Volume Buffers"),
    ];

    let chip_w = (card_w - 8) / 2;
    let chip_h = 26;

    for (idx, (bit, label)) in regions.iter().enumerate() {
        let col = idx % 2;
        let row = idx / 2;
        let cx = card_x + col as i32 * (chip_w + 8);
        let cy = chips_y + 20 + row as i32 * (chip_h + 4);

        let is_active = (CURRENT_MASK & *bit) != 0;
        let is_hovered = HOVER_TARGET == Some(ClickTarget::Chip(*bit));

        let bg = if is_active {
            if is_hovered { rgb(40, 52, 75) } else { rgb(28, 38, 56) }
        } else {
            if is_hovered { rgb(32, 36, 48) } else { rgb(20, 22, 30) }
        };

        let border = if is_active { rgb(99, 102, 241) } else { rgb(40, 44, 58) };
        let text_color = if is_active { rgb(255, 255, 255) } else { rgb(140, 150, 168) };

        draw_rounded_card(hdc, cx, cy, chip_w, chip_h, 8, bg, border);

        let mark = if is_active { "✓ " } else { "○ " };
        let chip_text = format!("{}{}", mark, label);
        draw_string(hdc, &chip_text, FONT_SMALL, text_color, cx + 8, cy, chip_w - 16, chip_h, DT_LEFT | DT_VCENTER);
    }

    // 4. Main Action Button: ⚡ CLEAN MEMORY NOW
    let btn_y = 402;
    let btn_h = 46;
    let is_cleaning = IS_CLEANING.load(Ordering::SeqCst);
    let btn_bg = if is_cleaning {
        rgb(79, 70, 229)
    } else if HOVER_TARGET == Some(ClickTarget::CleanButton) {
        rgb(129, 140, 248)
    } else {
        rgb(99, 102, 241)
    };

    draw_rounded_card(hdc, card_x, btn_y, card_w, btn_h, 12, btn_bg, btn_bg);
    let btn_label = if is_cleaning {
        "⚡  OPTIMIZING MEMORY..."
    } else {
        "⚡  CLEAN MEMORY NOW"
    };
    draw_string(
        hdc,
        btn_label,
        FONT_BTN,
        rgb(255, 255, 255),
        card_x,
        btn_y,
        card_w,
        btn_h,
        DT_CENTER | DT_VCENTER,
    );

    // 5. Result Status Banner
    let status_y = 458;
    let status_h = 28;
    if let Some(msg) = &LAST_CLEAN_STATUS {
        draw_rounded_card(hdc, card_x, status_y, card_w, status_h, 8, rgb(18, 36, 28), rgb(34, 76, 52));
        draw_string(hdc, msg, FONT_SMALL, rgb(52, 211, 153), card_x + 8, status_y, card_w - 16, status_h, DT_CENTER | DT_VCENTER);
    } else {
        draw_rounded_card(hdc, card_x, status_y, card_w, status_h, 8, rgb(20, 22, 30), rgb(32, 36, 48));
        draw_string(hdc, "Ready to optimize system memory", FONT_SMALL, rgb(148, 163, 184), card_x, status_y, card_w, status_h, DT_CENTER | DT_VCENTER);
    }

    // 6. Hotkey Quick Indicator Banner
    let hint_y = 496;
    let hint_h = 28;
    let is_hk_hover = HOVER_TARGET == Some(ClickTarget::HotkeyBanner);
    let (hk_bg, hk_border, hk_tc) = if is_hk_hover {
        (rgb(28, 34, 52), rgb(99, 102, 241), rgb(226, 232, 240))
    } else {
        (rgb(20, 24, 34), rgb(32, 38, 54), rgb(148, 163, 184))
    };
    draw_rounded_card(hdc, card_x, hint_y, card_w, hint_h, 8, hk_bg, hk_border);

    let hk_str = format_hotkey(config.hotkey_modifiers, config.hotkey_vk);
    let hk_label = if config.hotkey_enable {
        format!("⌨  Global Hotkey: [ {} ]  •  Click to customize", hk_str)
    } else {
        "⌨  Global Hotkey: Disabled  •  Click to configure".to_string()
    };
    draw_string(hdc, &hk_label, FONT_SMALL, hk_tc, card_x, hint_y, card_w, hint_h, DT_CENTER | DT_VCENTER);
}

unsafe fn render_tab_settings(hdc: HDC, w: i32, _h: i32, config: &Config) {
    let card_x = 16;
    let card_w = w - 32;

    // 1. Hero Card: Global Cleanup Hotkey
    let y1 = 54;
    let h1 = 94;
    draw_rounded_card(hdc, card_x, y1, card_w, h1, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "GLOBAL CLEANUP SHORTCUT", FONT_SMALL, rgb(148, 163, 184), card_x + 14, y1 + 8, 200, 16, DT_LEFT);

    let hk_tog_x = card_x + card_w - 68;
    let is_hk_hover = HOVER_TARGET == Some(ClickTarget::SettingHotkey);
    let hk_tog_bg = if config.hotkey_enable {
        if is_hk_hover { rgb(129, 140, 248) } else { rgb(99, 102, 241) }
    } else {
        if is_hk_hover { rgb(55, 62, 82) } else { rgb(40, 45, 60) }
    };
    let hk_tog_txt = if config.hotkey_enable { "ON" } else { "OFF" };
    draw_rounded_card(hdc, hk_tog_x, y1 + 8, 54, 20, 10, hk_tog_bg, hk_tog_bg);
    draw_string(hdc, hk_tog_txt, FONT_SMALL, rgb(255, 255, 255), hk_tog_x, y1 + 8, 54, 20, DT_CENTER | DT_VCENTER);

    if IS_RECORDING_HOTKEY {
        let rec_w = card_w - 28;
        let rec_y = y1 + 38;
        let rec_h = 44;
        draw_rounded_card(hdc, card_x + 14, rec_y, rec_w, rec_h, 10, rgb(50, 30, 15), rgb(245, 158, 11));
        draw_string(
            hdc,
            "🔴  PRESS ANY KEY COMBINATION NOW...",
            FONT_BTN,
            rgb(251, 191, 36),
            card_x + 14,
            rec_y + 4,
            rec_w,
            20,
            DT_CENTER | DT_VCENTER,
        );
        draw_string(
            hdc,
            "Hold Ctrl/Alt/Shift + Key  •  Press Esc to cancel",
            FONT_SMALL,
            rgb(253, 230, 138),
            card_x + 14,
            rec_y + 24,
            rec_w,
            16,
            DT_CENTER | DT_VCENTER,
        );
    } else {
        // Display badge
        let hk_badge_w = 140;
        let hk_badge_y = y1 + 42;
        let hk_badge_h = 36;
        draw_rounded_card(hdc, card_x + 14, hk_badge_y, hk_badge_w, hk_badge_h, 8, rgb(30, 36, 54), rgb(99, 102, 241));
        let cur_hk = format_hotkey(config.hotkey_modifiers, config.hotkey_vk);
        draw_string(hdc, &cur_hk, FONT_BTN, rgb(255, 255, 255), card_x + 14, hk_badge_y, hk_badge_w, hk_badge_h, DT_CENTER | DT_VCENTER);

        // Change button
        let chg_x = card_x + 162;
        let chg_w = card_w - 250;
        let is_chg_hover = HOVER_TARGET == Some(ClickTarget::SettingHotkeyRecord);
        let chg_bg = if is_chg_hover { rgb(55, 65, 95) } else { rgb(40, 48, 70) };
        let chg_border = if is_chg_hover { rgb(129, 140, 248) } else { rgb(60, 70, 100) };
        draw_rounded_card(hdc, chg_x, hk_badge_y, chg_w, hk_badge_h, 8, chg_bg, chg_border);
        draw_string(hdc, "⌨  Record Key", FONT_SUBTITLE, rgb(226, 232, 240), chg_x, hk_badge_y, chg_w, hk_badge_h, DT_CENTER | DT_VCENTER);

        // Reset button
        let rst_hk_x = card_x + card_w - 78;
        let rst_hk_w = 64;
        let is_rst_hk_hover = HOVER_TARGET == Some(ClickTarget::SettingHotkeyReset);
        let rst_hk_bg = if is_rst_hk_hover { rgb(50, 24, 30) } else { rgb(35, 22, 26) };
        let rst_hk_border = if is_rst_hk_hover { rgb(140, 40, 50) } else { rgb(70, 30, 36) };
        draw_rounded_card(hdc, rst_hk_x, hk_badge_y, rst_hk_w, hk_badge_h, 8, rst_hk_bg, rst_hk_border);
        draw_string(hdc, "↺ Reset", FONT_SMALL, rgb(248, 113, 113), rst_hk_x, hk_badge_y, rst_hk_w, hk_badge_h, DT_CENTER | DT_VCENTER);
    }

    // 2. System & App Settings Card
    let y2 = 156;
    let h2 = 168;
    draw_rounded_card(hdc, card_x, y2, card_w, h2, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "APPLICATION PREFERENCES", FONT_SMALL, rgb(148, 163, 184), card_x + 14, y2 + 8, 240, 16, DT_LEFT);

    let rows: [(&str, bool, ClickTarget); 5] = [
        ("Window always on top", config.always_on_top, ClickTarget::SettingAlwaysOnTop),
        ("Load on Windows startup", config.load_on_startup, ClickTarget::SettingAutostart),
        ("Start minimized to tray", config.start_minimized, ClickTarget::SettingStartMinimized),
        ("Show balloon notifications", config.balloon_notifications, ClickTarget::SettingBalloonNotifications),
        ("Notification sound effect", config.notification_sound, ClickTarget::SettingNotificationSound),
    ];

    for (i, (label, val, target)) in rows.iter().enumerate() {
        let ry = y2 + 28 + i as i32 * 27;
        draw_string(hdc, label, FONT_BODY, rgb(226, 232, 240), card_x + 14, ry, 230, 20, DT_LEFT | DT_VCENTER);

        let sw_x = card_x + card_w - 68;
        let is_hover = HOVER_TARGET == Some(*target);
        let sw_bg = if *val {
            if is_hover { rgb(129, 140, 248) } else { rgb(99, 102, 241) }
        } else {
            if is_hover { rgb(55, 62, 82) } else { rgb(40, 45, 60) }
        };
        let sw_txt = if *val { "ON" } else { "OFF" };
        draw_rounded_card(hdc, sw_x, ry, 54, 20, 10, sw_bg, sw_bg);
        draw_string(hdc, sw_txt, FONT_SMALL, rgb(255, 255, 255), sw_x, ry, 54, 20, DT_CENTER | DT_VCENTER);
    }

    // 3. Setting: Auto-Clean Threshold
    let y3 = 332;
    let h3 = 80;
    draw_rounded_card(hdc, card_x, y3, card_w, h3, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "AUTO-REDUCT ON THRESHOLD", FONT_SMALL, rgb(148, 163, 184), card_x + 14, y3 + 8, 200, 16, DT_LEFT);

    let is_at_hover = HOVER_TARGET == Some(ClickTarget::SettingAutoToggle);
    let tog_text = if config.autoreduct_enable { "ACTIVE" } else { "DISABLED" };
    let tog_bg = if config.autoreduct_enable {
        if is_at_hover { rgb(52, 211, 153) } else { rgb(16, 185, 129) }
    } else {
        if is_at_hover { rgb(60, 68, 88) } else { rgb(45, 50, 68) }
    };
    draw_rounded_card(hdc, card_x + card_w - 86, y3 + 8, 72, 20, 10, tog_bg, tog_bg);
    draw_string(hdc, tog_text, FONT_SMALL, rgb(255, 255, 255), card_x + card_w - 86, y3 + 8, 72, 20, DT_CENTER | DT_VCENTER);

    let limits = [70u8, 75, 80, 85, 90, 95];
    let pill_w = 42;
    let pill_h = 24;
    for (i, &lim) in limits.iter().enumerate() {
        let px = card_x + 14 + i as i32 * (pill_w + 7);
        let py = y3 + 42;
        let is_sel = config.autoreduct_limit == lim;
        let is_hover = HOVER_TARGET == Some(ClickTarget::SettingLimit(lim));
        let bg = if is_sel {
            rgb(99, 102, 241)
        } else if is_hover {
            rgb(45, 52, 72)
        } else {
            rgb(32, 36, 48)
        };
        let tc = if is_sel { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
        draw_rounded_card(hdc, px, py, pill_w, pill_h, 8, bg, bg);
        draw_string(hdc, &format!("{}%", lim), FONT_SMALL, tc, px, py, pill_w, pill_h, DT_CENTER | DT_VCENTER);
    }

    // 4. Setting: Periodic Interval
    let y4 = 420;
    let h4 = 80;
    draw_rounded_card(hdc, card_x, y4, card_w, h4, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "PERIODIC CLEANUP INTERVAL", FONT_SMALL, rgb(148, 163, 184), card_x + 14, y4 + 8, 200, 16, DT_LEFT);

    let is_it_hover = HOVER_TARGET == Some(ClickTarget::SettingIntervalToggle);
    let int_text = if config.autoreduct_interval_enable { "ACTIVE" } else { "DISABLED" };
    let int_bg = if config.autoreduct_interval_enable {
        if is_it_hover { rgb(52, 211, 153) } else { rgb(16, 185, 129) }
    } else {
        if is_it_hover { rgb(60, 68, 88) } else { rgb(45, 50, 68) }
    };
    draw_rounded_card(hdc, card_x + card_w - 86, y4 + 8, 72, 20, 10, int_bg, int_bg);
    draw_string(hdc, int_text, FONT_SMALL, rgb(255, 255, 255), card_x + card_w - 86, y4 + 8, 72, 20, DT_CENTER | DT_VCENTER);

    let intervals = [15u64, 30, 45, 60, 120];
    let ipill_w = 52;
    for (i, &iv) in intervals.iter().enumerate() {
        let px = card_x + 14 + i as i32 * (ipill_w + 8);
        let py = y4 + 42;
        let is_sel = config.autoreduct_interval_minutes == iv;
        let is_hover = HOVER_TARGET == Some(ClickTarget::SettingInterval(iv));
        let bg = if is_sel {
            rgb(99, 102, 241)
        } else if is_hover {
            rgb(45, 52, 72)
        } else {
            rgb(32, 36, 48)
        };
        let tc = if is_sel { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
        draw_rounded_card(hdc, px, py, ipill_w, pill_h, 8, bg, bg);
        draw_string(hdc, &format!("{}m", iv), FONT_SMALL, tc, px, py, ipill_w, pill_h, DT_CENTER | DT_VCENTER);
    }

    // 5. Bottom Action Buttons
    let y5 = 510;
    let btn_h = 38;

    // Reset Defaults Button
    let rst_w = 140;
    let is_rst_hover = HOVER_TARGET == Some(ClickTarget::SettingReset);
    let rst_bg = if is_rst_hover { rgb(55, 25, 32) } else { rgb(36, 22, 26) };
    let rst_border = if is_rst_hover { rgb(140, 45, 55) } else { rgb(80, 36, 42) };
    draw_rounded_card(hdc, card_x, y5, rst_w, btn_h, 10, rst_bg, rst_border);
    draw_string(hdc, "↺  Reset All", FONT_SUBTITLE, rgb(248, 113, 113), card_x, y5, rst_w, btn_h, DT_CENTER | DT_VCENTER);

    // Save & Return Button
    let save_x = card_x + rst_w + 10;
    let save_w = card_w - rst_w - 10;
    let is_save_hover = HOVER_TARGET == Some(ClickTarget::SettingSave);
    let save_bg = if is_save_hover { rgb(52, 211, 153) } else { rgb(16, 185, 129) };
    draw_rounded_card(hdc, save_x, y5, save_w, btn_h, 10, save_bg, save_bg);
    draw_string(hdc, "💾  SAVE & RETURN", FONT_BTN, rgb(255, 255, 255), save_x, y5, save_w, btn_h, DT_CENTER | DT_VCENTER);
}

fn hit_test(x: i32, y: i32, w: i32) -> Option<ClickTarget> {
    if y >= 8 && y <= 38 {
        if x >= w - 38 && x <= w - 8 {
            return Some(ClickTarget::BtnClose);
        }
        if x >= w - 68 && x <= w - 38 {
            return Some(ClickTarget::BtnMinimize);
        }
        let seg_w = 176;
        let seg_x = (w - seg_w) / 2;
        if x >= seg_x && x <= seg_x + 86 {
            return Some(ClickTarget::TabOverview);
        }
        if x >= seg_x + 88 && x <= seg_x + seg_w {
            return Some(ClickTarget::TabSettings);
        }
    }

    unsafe {
        match ACTIVE_TAB {
            ActiveTab::Overview => {
                let card_x = 16;
                let card_w = w - 32;

                // Main Clean Button
                if x >= card_x && x <= card_x + card_w && y >= 402 && y <= 448 {
                    return Some(ClickTarget::CleanButton);
                }

                // Quick Hotkey Banner
                if x >= card_x && x <= card_x + card_w && y >= 496 && y <= 524 {
                    return Some(ClickTarget::HotkeyBanner);
                }

                let regions = [
                    (REDUCT_WORKINGSET, 0, 0),
                    (REDUCT_SYSTEMFILECACHE, 1, 0),
                    (REDUCT_STANDBYPRIORITY0LIST, 0, 1),
                    (REDUCT_STANDBYLIST, 1, 1),
                    (REDUCT_MODIFIEDLIST, 0, 2),
                    (REDUCT_REGISTRYCACHE, 1, 2),
                    (REDUCT_COMBINEMEMORYLISTS, 0, 3),
                    (REDUCT_MODIFIEDFILECACHE, 1, 3),
                ];

                let chip_w = (card_w - 8) / 2;
                let chip_h = 26;

                for (bit, col, row) in regions {
                    let cx = card_x + col * (chip_w + 8);
                    let cy = 284 + row * (chip_h + 4);
                    if x >= cx && x <= cx + chip_w && y >= cy && y <= cy + chip_h {
                        return Some(ClickTarget::Chip(bit));
                    }
                }
            }
            ActiveTab::Settings => {
                let card_x = 16;
                let card_w = w - 32;

                // Section 1: Hotkey Card
                let y1 = 54;
                // Hotkey Toggle (ON/OFF)
                if x >= card_x + card_w - 68 && x <= card_x + card_w - 14 && y >= y1 + 8 && y <= y1 + 28 {
                    return Some(ClickTarget::SettingHotkey);
                }
                // Hotkey Change button
                if x >= card_x + 162 && x <= card_x + card_w - 82 && y >= y1 + 42 && y <= y1 + 78 {
                    return Some(ClickTarget::SettingHotkeyRecord);
                }
                // Hotkey Reset button
                if x >= card_x + card_w - 78 && x <= card_x + card_w - 14 && y >= y1 + 42 && y <= y1 + 78 {
                    return Some(ClickTarget::SettingHotkeyReset);
                }

                // Section 2: Preferences
                let y2 = 156;
                let sw_x = card_x + card_w - 68;
                if x >= sw_x && x <= sw_x + 56 {
                    for i in 0..5 {
                        let ry = y2 + 28 + i * 27;
                        if y >= ry && y <= ry + 22 {
                            return match i {
                                0 => Some(ClickTarget::SettingAlwaysOnTop),
                                1 => Some(ClickTarget::SettingAutostart),
                                2 => Some(ClickTarget::SettingStartMinimized),
                                3 => Some(ClickTarget::SettingBalloonNotifications),
                                4 => Some(ClickTarget::SettingNotificationSound),
                                _ => None,
                            };
                        }
                    }
                }

                // Section 3: Threshold
                let y3 = 332;
                if x >= card_x + card_w - 86 && x <= card_x + card_w - 14 && y >= y3 + 8 && y <= y3 + 28 {
                    return Some(ClickTarget::SettingAutoToggle);
                }

                let limits = [70u8, 75, 80, 85, 90, 95];
                let pill_w = 42;
                let pill_h = 24;
                for (i, &lim) in limits.iter().enumerate() {
                    let px = card_x + 14 + i as i32 * (pill_w + 7);
                    let py = y3 + 42;
                    if x >= px && x <= px + pill_w && y >= py && y <= py + pill_h {
                        return Some(ClickTarget::SettingLimit(lim));
                    }
                }

                // Section 4: Interval
                let y4 = 420;
                if x >= card_x + card_w - 86 && x <= card_x + card_w - 14 && y >= y4 + 8 && y <= y4 + 28 {
                    return Some(ClickTarget::SettingIntervalToggle);
                }

                let intervals = [15u64, 30, 45, 60, 120];
                let ipill_w = 52;
                for (i, &iv) in intervals.iter().enumerate() {
                    let px = card_x + 14 + i as i32 * (ipill_w + 8);
                    let py = y4 + 42;
                    if x >= px && x <= px + ipill_w && y >= py && y <= py + pill_h {
                        return Some(ClickTarget::SettingInterval(iv));
                    }
                }

                // Section 5: Bottom Buttons
                let y5 = 510;
                let btn_h = 38;
                let rst_w = 140;
                // Reset defaults
                if x >= card_x && x <= card_x + rst_w && y >= y5 && y <= y5 + btn_h {
                    return Some(ClickTarget::SettingReset);
                }
                // Save & Return
                if x >= card_x + rst_w + 10 && x <= card_x + card_w && y >= y5 && y <= y5 + btn_h {
                    return Some(ClickTarget::SettingSave);
                }
            }
        }
    }
    None
}

unsafe fn create_dynamic_tray_icon(percent: u8, warning_lvl: u8, danger_lvl: u8) -> HICON {
    let screen_dc = GetDC(ptr::null_mut());
    if screen_dc.is_null() {
        return ptr::null_mut();
    }

    let hdc = CreateCompatibleDC(screen_dc);
    let hbm_color = CreateCompatibleBitmap(screen_dc, 16, 16);
    let hbm_mask = CreateBitmap(16, 16, 1, 1, ptr::null());

    let old_bmp = SelectObject(hdc, hbm_color);

    let bg_color = if percent >= danger_lvl {
        rgb(236, 28, 36)   // Danger: Red (#EC1C24)
    } else if percent >= warning_lvl {
        rgb(255, 128, 64)  // Warning: Orange (#FF8040)
    } else {
        rgb(0, 128, 64)    // Normal: Green (#008040)
    };

    draw_rounded_card(hdc, 0, 0, 16, 16, 4, bg_color, bg_color);

    let font_name = to_wide("Tahoma");
    let font = CreateFontW(10, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    let old_font = SelectObject(hdc, font as *mut _);

    SetTextColor(hdc, rgb(255, 255, 255));
    SetBkMode(hdc, TRANSPARENT);

    let text = format!("{}", percent.min(99));
    let text_wide = to_wide(&text);
    let mut rc = RECT { left: 0, top: 0, right: 16, bottom: 16 };
    DrawTextW(hdc, text_wide.as_ptr(), -1, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    SelectObject(hdc, old_font);
    DeleteObject(font as *mut _);

    SelectObject(hdc, old_bmp);
    DeleteDC(hdc);
    ReleaseDC(ptr::null_mut(), screen_dc);

    let mut ii = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };

    let hicon = CreateIconIndirect(&mut ii);

    DeleteObject(hbm_color);
    DeleteObject(hbm_mask);

    hicon
}

unsafe fn show_balloon_notification(hwnd: HWND, title: &str, message: &str, sound: bool) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO | if sound { 0 } else { NIIF_NOSOUND };

    let title_w = to_wide(title);
    for (i, &c) in title_w.iter().take(63).enumerate() {
        nid.szInfoTitle[i] = c;
    }
    let msg_w = to_wide(message);
    for (i, &c) in msg_w.iter().take(255).enumerate() {
        nid.szInfo[i] = c;
    }

    Shell_NotifyIconW(NIM_MODIFY, &mut nid);

    if sound {
        MessageBeep(0x00000040); // MB_ICONASTERISK
    }
}

unsafe fn setup_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
    nid.uCallbackMessage = WM_TRAYICON;

    let config = Config::load();
    let initial_pct = get_memory_info().map(|m| m.percent_physical).unwrap_or(0);
    let hicon = create_dynamic_tray_icon(initial_pct, config.warning_level, config.danger_level);
    nid.hIcon = hicon;
    CURRENT_TRAY_ICON = hicon;

    let tip = to_wide(&format!("Mem Reduct: {}% RAM used", initial_pct));
    for (i, &c) in tip.iter().take(127).enumerate() {
        nid.szTip[i] = c;
    }
    Shell_NotifyIconW(NIM_ADD, &mut nid);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCREATE => 1,
        WM_ERASEBKGND => 1,
        WM_CREATE => {
            let config = Config::load();
            CURRENT_MASK = if config.reduct_mask != 0 {
                config.reduct_mask
            } else {
                REDUCT_MASK_DEFAULT
            };

            if config.always_on_top {
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }

            if config.hotkey_enable {
                RegisterHotKey(
                    hwnd,
                    ID_HOTKEY_CLEAN,
                    config.hotkey_modifiers | MOD_NOREPEAT,
                    config.hotkey_vk,
                );
            }

            SetTimer(hwnd, 1, 1000, None);
            0
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            if IS_RECORDING_HOTKEY {
                let vk = wparam as u32;
                // Esc cancels recording
                if vk == 0x1B {
                    IS_RECORDING_HOTKEY = false;
                    InvalidateRect(hwnd, ptr::null(), 0);
                    return 0;
                }

                // Ignore standalone modifier keypresses alone
                let is_modifier_key = matches!(
                    vk,
                    0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5
                );
                if is_modifier_key {
                    return 0;
                }

                let mut mods = 0u32;
                if (GetKeyState(0x11) as u16 & 0x8000) != 0 {
                    mods |= MOD_CONTROL;
                }
                if (GetKeyState(0x12) as u16 & 0x8000) != 0 {
                    mods |= MOD_ALT;
                }
                if (GetKeyState(0x10) as u16 & 0x8000) != 0 {
                    mods |= MOD_SHIFT;
                }
                if ((GetKeyState(0x5B) as u16 & 0x8000) != 0)
                    || ((GetKeyState(0x5C) as u16 & 0x8000) != 0)
                {
                    mods |= MOD_WIN;
                }

                let mut config = Config::load();
                config.hotkey_modifiers = mods;
                config.hotkey_vk = vk;
                let _ = config.save();
                IS_RECORDING_HOTKEY = false;

                UnregisterHotKey(hwnd, ID_HOTKEY_CLEAN);
                if config.hotkey_enable {
                    RegisterHotKey(
                        hwnd,
                        ID_HOTKEY_CLEAN,
                        config.hotkey_modifiers | MOD_NOREPEAT,
                        config.hotkey_vk,
                    );
                }

                InvalidateRect(hwnd, ptr::null(), 0);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_HOTKEY => {
            if (wparam as i32) == ID_HOTKEY_CLEAN {
                do_clean(hwnd);
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let w = rc.right - rc.left;
            let h = rc.bottom - rc.top;

            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bmp = CreateCompatibleBitmap(hdc, w, h);
            let old_bmp = SelectObject(mem_dc, mem_bmp);

            let config = Config::load();
            render_modern_gui(mem_dc, w, h, &config);

            BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bmp);
            DeleteObject(mem_bmp);
            DeleteDC(mem_dc);

            EndPaint(hwnd, &ps);
            0
        }
        WM_MOUSEMOVE => {
            if !IS_MOUSE_TRACKED {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut tme);
                IS_MOUSE_TRACKED = true;
            }

            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let w = rc.right - rc.left;

            let new_target = hit_test(x, y, w);
            if new_target != HOVER_TARGET {
                HOVER_TARGET = new_target;
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_MOUSELEAVE => {
            IS_MOUSE_TRACKED = false;
            if HOVER_TARGET.is_some() {
                HOVER_TARGET = None;
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let w = rc.right - rc.left;

            let mut config = Config::load();
            if let Some(target) = hit_test(x, y, w) {
                if target != ClickTarget::SettingHotkeyRecord && IS_RECORDING_HOTKEY {
                    IS_RECORDING_HOTKEY = false;
                }

                match target {
                    ClickTarget::BtnMinimize => {
                        let config = Config::load();
                        if config.tray_minimized {
                            ShowWindow(hwnd, SW_HIDE);
                        } else {
                            ShowWindow(hwnd, SW_MINIMIZE);
                        }
                    }
                    ClickTarget::BtnClose => {
                        SendMessageW(hwnd, WM_CLOSE, 0, 0);
                    }
                    ClickTarget::TabOverview => {
                        ACTIVE_TAB = ActiveTab::Overview;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::TabSettings => {
                        ACTIVE_TAB = ActiveTab::Settings;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::CleanButton => {
                        do_clean(hwnd);
                    }
                    ClickTarget::HotkeyBanner => {
                        ACTIVE_TAB = ActiveTab::Settings;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::Chip(bit) => {
                        CURRENT_MASK ^= bit;
                        config.reduct_mask = CURRENT_MASK;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingAlwaysOnTop => {
                        config.always_on_top = !config.always_on_top;
                        let insert = if config.always_on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
                        SetWindowPos(hwnd, insert, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingAutostart => {
                        config.load_on_startup = !config.load_on_startup;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingStartMinimized => {
                        config.start_minimized = !config.start_minimized;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingBalloonNotifications => {
                        config.balloon_notifications = !config.balloon_notifications;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingNotificationSound => {
                        config.notification_sound = !config.notification_sound;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingHotkey => {
                        config.hotkey_enable = !config.hotkey_enable;
                        if config.hotkey_enable {
                            RegisterHotKey(
                                hwnd,
                                ID_HOTKEY_CLEAN,
                                config.hotkey_modifiers | MOD_NOREPEAT,
                                config.hotkey_vk,
                            );
                        } else {
                            UnregisterHotKey(hwnd, ID_HOTKEY_CLEAN);
                        }
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingHotkeyRecord => {
                        IS_RECORDING_HOTKEY = true;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingHotkeyReset => {
                        config.hotkey_modifiers = 0x0002; // MOD_CONTROL
                        config.hotkey_vk = 0x70;          // VK_F1
                        let _ = config.save();
                        UnregisterHotKey(hwnd, ID_HOTKEY_CLEAN);
                        if config.hotkey_enable {
                            RegisterHotKey(
                                hwnd,
                                ID_HOTKEY_CLEAN,
                                config.hotkey_modifiers | MOD_NOREPEAT,
                                config.hotkey_vk,
                            );
                        }
                        IS_RECORDING_HOTKEY = false;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingAutoToggle => {
                        config.autoreduct_enable = !config.autoreduct_enable;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingLimit(val) => {
                        config.autoreduct_limit = val;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingIntervalToggle => {
                        config.autoreduct_interval_enable = !config.autoreduct_interval_enable;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingInterval(val) => {
                        config.autoreduct_interval_minutes = val;
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingReset => {
                        let def = Config::default();
                        CURRENT_MASK = def.reduct_mask;
                        let _ = def.save();
                        UnregisterHotKey(hwnd, ID_HOTKEY_CLEAN);
                        if def.hotkey_enable {
                            RegisterHotKey(
                                hwnd,
                                ID_HOTKEY_CLEAN,
                                def.hotkey_modifiers | MOD_NOREPEAT,
                                def.hotkey_vk,
                            );
                        }
                        IS_RECORDING_HOTKEY = false;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingSave => {
                        config.reduct_mask = CURRENT_MASK;
                        let _ = config.save();
                        ACTIVE_TAB = ActiveTab::Overview;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
            } else {
                if IS_RECORDING_HOTKEY {
                    IS_RECORDING_HOTKEY = false;
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
                if y <= 48 {
                    // Header bar dragging
                    ReleaseCapture();
                    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as WPARAM, 0);
                }
            }
            0
        }
        WM_CLEAN_FINISHED => {
            IS_CLEANING.store(false, Ordering::SeqCst);
            if lparam != 0 {
                let boxed = unsafe { Box::from_raw(lparam as *mut Result<crate::cleaner::CleanReport, String>) };
                match *boxed {
                    Ok(rep) => {
                        LAST_CLEAN_TIME = Some(Instant::now());
                        let freed = format_bytes(rep.bytes_freed);
                        let status_str = format!(
                            "✓ Memory released: {} ({}% -> {}) in {:.1?}",
                            freed, rep.mem_before.percent_physical, rep.mem_after.percent_physical, rep.duration
                        );
                        LAST_CLEAN_STATUS = Some(status_str);

                        let config = Config::load();
                        if config.balloon_notifications {
                            let balloon_msg = format!(
                                "Memory cleaned: {}\nUsage: {}% -> {}%",
                                freed, rep.mem_before.percent_physical, rep.mem_after.percent_physical
                            );
                            show_balloon_notification(hwnd, "Mem Reduct", &balloon_msg, config.notification_sound);
                        }
                    }
                    Err(e) => {
                        LAST_CLEAN_STATUS = Some(format!("Error: {}", e));
                    }
                }
            }
            InvalidateRect(hwnd, ptr::null(), 0);
            0
        }
        WM_TIMER => {
            let config = Config::load();
            let is_cleaning = IS_CLEANING.load(Ordering::SeqCst);

            if config.autoreduct_enable && !is_cleaning {
                if let Ok(info) = get_memory_info() {
                    if info.percent_physical >= config.autoreduct_limit {
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

            if config.autoreduct_interval_enable && config.autoreduct_interval_minutes > 0 && !is_cleaning {
                let interval = Duration::from_secs(config.autoreduct_interval_minutes * 60);
                let can_clean = match LAST_CLEAN_TIME {
                    Some(t) => t.elapsed() >= interval,
                    None => true,
                };
                if can_clean {
                    do_clean(hwnd);
                }
            }

            if let Ok(info) = get_memory_info() {
                // Update dynamic tray icon with live RAM % number and color
                let new_icon = create_dynamic_tray_icon(info.percent_physical, config.warning_level, config.danger_level);
                if !new_icon.is_null() {
                    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                    nid.hWnd = hwnd;
                    nid.uID = 1;
                    nid.uFlags = NIF_ICON | NIF_TIP;
                    nid.hIcon = new_icon;

                    let tip = format!(
                        "Mem Reduct: {}% ({} / {})",
                        info.percent_physical,
                        format_bytes(info.used_physical),
                        format_bytes(info.total_physical)
                    );
                    let tip_wide = to_wide(&tip);
                    for (i, &c) in tip_wide.iter().take(127).enumerate() {
                        nid.szTip[i] = c;
                    }
                    Shell_NotifyIconW(NIM_MODIFY, &mut nid);

                    if !CURRENT_TRAY_ICON.is_null() {
                        DestroyIcon(CURRENT_TRAY_ICON);
                    }
                    CURRENT_TRAY_ICON = new_icon;
                }
            }

            InvalidateRect(hwnd, ptr::null(), 0);
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
                let config = Config::load();
                let htray = CreatePopupMenu();

                AppendMenuW(htray, MF_STRING, IDM_TRAY_SHOWHIDE, to_wide("Show / Hide").as_ptr());
                AppendMenuW(htray, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray, MF_STRING, IDM_TRAY_CLEAN, to_wide("⚡ Clean Memory").as_ptr());
                AppendMenuW(htray, MF_SEPARATOR, 0, ptr::null());

                // Submenu: Clean areas
                let hareas = CreatePopupMenu();
                let reg_items = [
                    (REDUCT_WORKINGSET, IDM_AREA_WORKINGSET, "Working Set"),
                    (REDUCT_SYSTEMFILECACHE, IDM_AREA_SYSTEMFILECACHE, "System File Cache"),
                    (REDUCT_MODIFIEDFILECACHE, IDM_AREA_MODIFIEDFILECACHE, "Modified File Cache"),
                    (REDUCT_STANDBYPRIORITY0LIST, IDM_AREA_STANDBY0, "Standby List (Priority 0)"),
                    (REDUCT_STANDBYLIST, IDM_AREA_STANDBY, "Standby List"),
                    (REDUCT_MODIFIEDLIST, IDM_AREA_MODIFIED, "Modified Page List"),
                    (REDUCT_REGISTRYCACHE, IDM_AREA_REGISTRY, "Registry Cache"),
                    (REDUCT_COMBINEMEMORYLISTS, IDM_AREA_COMBINE, "Combine Memory Lists"),
                ];
                for (bit, id, label) in &reg_items {
                    let checked = (CURRENT_MASK & *bit) != 0;
                    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
                    AppendMenuW(hareas, flags, *id, to_wide(label).as_ptr());
                }
                AppendMenuW(htray, MF_POPUP, hareas as usize, to_wide("Clean Areas").as_ptr());

                // Submenu: Clean when above
                let habove = CreatePopupMenu();
                let above_items = [
                    (0u8, IDM_ABOVE_DISABLED, "Disabled"),
                    (70, IDM_ABOVE_70, "70%"),
                    (75, IDM_ABOVE_75, "75%"),
                    (80, IDM_ABOVE_80, "80%"),
                    (85, IDM_ABOVE_85, "85%"),
                    (90, IDM_ABOVE_90, "90%"),
                    (95, IDM_ABOVE_95, "95%"),
                ];
                for (lvl, id, label) in &above_items {
                    let checked = if *lvl == 0 { !config.autoreduct_enable } else { config.autoreduct_enable && config.autoreduct_limit == *lvl };
                    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
                    AppendMenuW(habove, flags, *id, to_wide(label).as_ptr());
                }
                AppendMenuW(htray, MF_POPUP, habove as usize, to_wide("Clean When Above").as_ptr());

                // Submenu: Clean every
                let hevery = CreatePopupMenu();
                let every_items = [
                    (0u64, IDM_EVERY_DISABLED, "Disabled"),
                    (15, IDM_EVERY_15, "15 min"),
                    (30, IDM_EVERY_30, "30 min"),
                    (45, IDM_EVERY_45, "45 min"),
                    (60, IDM_EVERY_60, "60 min"),
                    (120, IDM_EVERY_120, "120 min"),
                ];
                for (iv, id, label) in &every_items {
                    let checked = if *iv == 0 { !config.autoreduct_interval_enable } else { config.autoreduct_interval_enable && config.autoreduct_interval_minutes == *iv };
                    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
                    AppendMenuW(hevery, flags, *id, to_wide(label).as_ptr());
                }
                AppendMenuW(htray, MF_POPUP, hevery as usize, to_wide("Clean Every").as_ptr());

                AppendMenuW(htray, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray, MF_STRING, IDM_TRAY_SETTINGS, to_wide("Settings...").as_ptr());
                AppendMenuW(htray, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray, MF_STRING, IDM_TRAY_EXIT, to_wide("Exit").as_ptr());

                SetForegroundWindow(hwnd);
                TrackPopupMenu(htray, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, ptr::null());
                DestroyMenu(htray);
            }
            0
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let mut config = Config::load();
            match id {
                IDM_TRAY_SHOWHIDE => {
                    if IsWindowVisible(hwnd) != 0 {
                        ShowWindow(hwnd, SW_HIDE);
                    } else {
                        ShowWindow(hwnd, SW_SHOW);
                        SetForegroundWindow(hwnd);
                    }
                }
                IDM_TRAY_CLEAN => {
                    do_clean(hwnd);
                }
                IDM_TRAY_SETTINGS => {
                    ACTIVE_TAB = ActiveTab::Settings;
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
                IDM_TRAY_EXIT => {
                    PostQuitMessage(0);
                }
                // Clean Areas: 201..208
                IDM_AREA_WORKINGSET => { CURRENT_MASK ^= REDUCT_WORKINGSET; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_SYSTEMFILECACHE => { CURRENT_MASK ^= REDUCT_SYSTEMFILECACHE; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_MODIFIEDFILECACHE => { CURRENT_MASK ^= REDUCT_MODIFIEDFILECACHE; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_STANDBY0 => { CURRENT_MASK ^= REDUCT_STANDBYPRIORITY0LIST; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_STANDBY => { CURRENT_MASK ^= REDUCT_STANDBYLIST; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_MODIFIED => { CURRENT_MASK ^= REDUCT_MODIFIEDLIST; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_REGISTRY => { CURRENT_MASK ^= REDUCT_REGISTRYCACHE; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_AREA_COMBINE => { CURRENT_MASK ^= REDUCT_COMBINEMEMORYLISTS; config.reduct_mask = CURRENT_MASK; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }

                // Clean when above: 220..226
                IDM_ABOVE_DISABLED => { config.autoreduct_enable = false; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_ABOVE_70 => { config.autoreduct_enable = true; config.autoreduct_limit = 70; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_ABOVE_75 => { config.autoreduct_enable = true; config.autoreduct_limit = 75; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_ABOVE_80 => { config.autoreduct_enable = true; config.autoreduct_limit = 80; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_ABOVE_85 => { config.autoreduct_enable = true; config.autoreduct_limit = 85; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_ABOVE_90 => { config.autoreduct_enable = true; config.autoreduct_limit = 90; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_ABOVE_95 => { config.autoreduct_enable = true; config.autoreduct_limit = 95; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }

                // Clean every: 230..235
                IDM_EVERY_DISABLED => { config.autoreduct_interval_enable = false; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_EVERY_15 => { config.autoreduct_interval_enable = true; config.autoreduct_interval_minutes = 15; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_EVERY_30 => { config.autoreduct_interval_enable = true; config.autoreduct_interval_minutes = 30; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_EVERY_45 => { config.autoreduct_interval_enable = true; config.autoreduct_interval_minutes = 45; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_EVERY_60 => { config.autoreduct_interval_enable = true; config.autoreduct_interval_minutes = 60; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }
                IDM_EVERY_120 => { config.autoreduct_interval_enable = true; config.autoreduct_interval_minutes = 120; let _ = config.save(); InvalidateRect(hwnd, ptr::null(), 0); }

                _ => {}
            }
            0
        }
        WM_SYSCOMMAND => {
            let cmd = wparam & 0xFFF0;
            if cmd == SC_MINIMIZE {
                let config = Config::load();
                if config.tray_minimized {
                    ShowWindow(hwnd, SW_HIDE);
                    return 0;
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let config = Config::load();
            if config.tray_minimized {
                ShowWindow(hwnd, SW_HIDE);
            } else {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            UnregisterHotKey(hwnd, ID_HOTKEY_CLEAN);
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &mut nid);
            if !CURRENT_TRAY_ICON.is_null() {
                DestroyIcon(CURRENT_TRAY_ICON);
                CURRENT_TRAY_ICON = ptr::null_mut();
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn do_clean(hwnd: HWND) {
    if IS_CLEANING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    LAST_CLEAN_STATUS = Some("⚡ Cleaning memory in background...".to_string());
    InvalidateRect(hwnd, ptr::null(), 0);

    let hwnd_raw = hwnd as usize;
    let mask = CURRENT_MASK;
    std::thread::spawn(move || {
        let rep = clean_memory(mask);
        let boxed = Box::new(rep);
        let lparam = Box::into_raw(boxed) as LPARAM;
        unsafe {
            let res = PostMessageW(hwnd_raw as HWND, WM_CLEAN_FINISHED, 0, lparam);
            if res == 0 {
                drop(Box::from_raw(lparam as *mut Result<crate::cleaner::CleanReport, String>));
            }
        }
    });
}

pub fn run_gui() -> Result<(), String> {
    unsafe {
        let hinst = GetModuleHandleW(ptr::null());
        let class_name = to_wide("MemReductModernUI");

        create_fonts();

        let bg_brush = CreateSolidBrush(rgb(15, 17, 23));

        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0x0003, // CS_HREDRAW | CS_VREDRAW
            lpfnWndProc: wnd_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: bg_brush,
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        let atom = RegisterClassExW(&wc);
        let reg_err = GetLastError();
        if atom == 0 && reg_err != 1410 {
            DeleteObject(bg_brush as *mut _);
            return Err(format!("Failed to register window class (error code {})", reg_err));
        }

        let config = Config::load();
        let window_title = to_wide("Mem Reduct");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            440,
            640,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null_mut(),
        );

        let create_err = GetLastError();
        if hwnd.is_null() {
            DeleteObject(bg_brush as *mut _);
            return Err(format!(
                "Failed to create main window (error code {}, class registration atom: {})",
                create_err, atom
            ));
        }

        setup_tray_icon(hwnd);

        if !config.start_minimized {
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
        } else {
            ShowWindow(hwnd, SW_HIDE);
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        DeleteObject(bg_brush as *mut _);
        destroy_fonts();
        Ok(())
    }
}
