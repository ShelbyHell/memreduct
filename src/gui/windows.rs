#![allow(dead_code, static_mut_refs)]

use crate::cleaner::{
    clean_memory, REDUCT_COMBINEMEMORYLISTS, REDUCT_MASK_DEFAULT,
    REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST, REDUCT_REGISTRYCACHE, REDUCT_STANDBYLIST,
    REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE, REDUCT_WORKINGSET,
};
use crate::config::Config;
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

const SC_MINIMIZE: usize = 0xF020;
const HTCAPTION: isize = 2;

const MF_STRING: u32 = 0x00000000;
const MF_SEPARATOR: u32 = 0x00000800;
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

const HWND_TOPMOST: HWND = -1isize as HWND;
const HWND_NOTOPMOST: HWND = -2isize as HWND;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;

const IDM_TRAY_SHOWHIDE: usize = 101;
const IDM_TRAY_CLEAN: usize = 102;
const IDM_TRAY_EXIT: usize = 103;

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
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: HDC) -> HDC;
    fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> *mut std::ffi::c_void;
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
static IS_CLEANING: AtomicBool = AtomicBool::new(false);

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
    Chip(u32),
    SettingAutoToggle,
    SettingLimit(u8),
    SettingIntervalToggle,
    SettingInterval(u64),
    SettingAlwaysOnTop,
    SettingReset,
    SettingSave,
}

unsafe fn create_fonts() {
    let font_name = to_wide("Segoe UI");
    FONT_HERO = CreateFontW(32, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_TITLE = CreateFontW(17, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
    FONT_SUBTITLE = CreateFontW(14, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, font_name.as_ptr());
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
    draw_string(hdc, "⚡ Mem Reduct", FONT_TITLE, rgb(255, 255, 255), 14, 12, 105, 24, DT_LEFT | DT_VCENTER);
    // Version Badge
    draw_rounded_card(hdc, 122, 15, 42, 18, 8, rgb(30, 33, 44), rgb(45, 50, 68));
    draw_string(hdc, "v3.5.3", FONT_SMALL, rgb(148, 163, 184), 122, 15, 42, 18, DT_CENTER | DT_VCENTER);

    // Navigation Tabs: [ Monitor ]  [ Settings ]
    let tab_ov_bg = if ACTIVE_TAB == ActiveTab::Overview {
        rgb(99, 102, 241)
    } else if HOVER_TARGET == Some(ClickTarget::TabOverview) {
        rgb(38, 42, 56)
    } else {
        rgb(24, 27, 36)
    };
    let tab_ov_text = if ACTIVE_TAB == ActiveTab::Overview { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
    draw_rounded_card(hdc, 170, 12, 62, 24, 12, tab_ov_bg, rgb(45, 50, 68));
    draw_string(hdc, "Monitor", FONT_SUBTITLE, tab_ov_text, 170, 12, 62, 24, DT_CENTER | DT_VCENTER);

    let tab_st_bg = if ACTIVE_TAB == ActiveTab::Settings {
        rgb(99, 102, 241)
    } else if HOVER_TARGET == Some(ClickTarget::TabSettings) {
        rgb(38, 42, 56)
    } else {
        rgb(24, 27, 36)
    };
    let tab_st_text = if ACTIVE_TAB == ActiveTab::Settings { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
    draw_rounded_card(hdc, 238, 12, 64, 24, 12, tab_st_bg, rgb(45, 50, 68));
    draw_string(hdc, "Settings", FONT_SUBTITLE, tab_st_text, 238, 12, 64, 24, DT_CENTER | DT_VCENTER);

    // Window Control Buttons: [ ─ ]  [ ✕ ]
    let is_min_hover = HOVER_TARGET == Some(ClickTarget::BtnMinimize);
    let min_bg = if is_min_hover { rgb(38, 42, 56) } else { rgb(24, 27, 36) };
    let min_border = if is_min_hover { rgb(99, 102, 241) } else { rgb(45, 50, 68) };
    let min_text = if is_min_hover { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
    draw_rounded_card(hdc, w - 64, 12, 26, 24, 8, min_bg, min_border);
    draw_string(hdc, "─", FONT_SUBTITLE, min_text, w - 64, 11, 26, 24, DT_CENTER | DT_VCENTER);

    let is_cls_hover = HOVER_TARGET == Some(ClickTarget::BtnClose);
    let cls_bg = if is_cls_hover { rgb(220, 38, 38) } else { rgb(24, 27, 36) };
    let cls_border = if is_cls_hover { rgb(239, 68, 68) } else { rgb(45, 50, 68) };
    let cls_text = if is_cls_hover { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
    draw_rounded_card(hdc, w - 34, 12, 26, 24, 8, cls_bg, cls_border);
    draw_string(hdc, "✕", FONT_SUBTITLE, cls_text, w - 34, 12, 26, 24, DT_CENTER | DT_VCENTER);

    // Divider
    draw_rect_solid(hdc, 14, 46, w - 28, 1, rgb(30, 34, 46));

    match ACTIVE_TAB {
        ActiveTab::Overview => {
            render_tab_overview(hdc, w, h);
        }
        ActiveTab::Settings => {
            render_tab_settings(hdc, w, h, config);
        }
    }
}

unsafe fn render_tab_overview(hdc: HDC, w: i32, _h: i32) {
    let info = get_memory_info().unwrap_or_default();

    // 1. Hero Card: Physical RAM
    let card_x = 16;
    let card_y = 56;
    let card_w = w - 32;
    let card_h = 110;
    draw_rounded_card(hdc, card_x, card_y, card_w, card_h, 14, rgb(24, 27, 36), rgb(38, 43, 58));

    draw_string(hdc, "PHYSICAL MEMORY", FONT_SMALL, rgb(148, 163, 184), card_x + 16, card_y + 12, 160, 16, DT_LEFT);

    let p_color = if info.percent_physical >= 90 {
        rgb(239, 68, 68)
    } else if info.percent_physical >= 70 {
        rgb(245, 158, 11)
    } else {
        rgb(16, 185, 129)
    };

    draw_string(
        hdc,
        &format!("{}%", info.percent_physical),
        FONT_HERO,
        p_color,
        card_x + 16,
        card_y + 30,
        140,
        36,
        DT_LEFT | DT_VCENTER,
    );

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
        card_y + 24,
        125,
        45,
        DT_RIGHT,
    );

    draw_pill_gauge(
        hdc,
        card_x + 16,
        card_y + 82,
        card_w - 32,
        10,
        info.percent_physical,
        p_color,
        rgb(36, 41, 56),
    );

    // 2. Dual Sub-cards: Pagefile and Cache
    let sub_y = 176;
    let sub_w = (card_w - 10) / 2;
    let sub_h = 76;

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
        sub_y + 24,
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
        sub_y + 46,
        sub_w - 24,
        14,
        DT_LEFT,
    );

    // Right Card: System Cache
    draw_rounded_card(hdc, card_x + sub_w + 10, sub_y, sub_w, sub_h, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "SYSTEM CACHE", FONT_SMALL, rgb(148, 163, 184), card_x + sub_w + 22, sub_y + 8, sub_w - 24, 14, DT_LEFT);
    let cached_str = info.cached.map(format_bytes).unwrap_or_else(|| "N/A".into());
    draw_string(
        hdc,
        &cached_str,
        FONT_TITLE,
        rgb(168, 85, 247),
        card_x + sub_w + 22,
        sub_y + 24,
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
        sub_y + 46,
        sub_w - 24,
        14,
        DT_LEFT,
    );

    // 3. Areas to Clean Chips Grid
    let chips_y = 262;
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
    let chip_h = 24;

    for (idx, (bit, label)) in regions.iter().enumerate() {
        let col = idx % 2;
        let row = idx / 2;
        let cx = card_x + col as i32 * (chip_w + 8);
        let cy = chips_y + 20 + row as i32 * (chip_h + 6);

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

        let mark = if is_active { "✓ " } else { "   " };
        let chip_text = format!("{}{}", mark, label);
        draw_string(hdc, &chip_text, FONT_SMALL, text_color, cx + 8, cy, chip_w - 16, chip_h, DT_LEFT | DT_VCENTER);
    }

    // 4. Main Action Button: ⚡ CLEAN MEMORY NOW
    let btn_y = 398;
    let btn_h = 44;
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
    let status_y = 452;
    let status_h = 28;
    if let Some(msg) = &LAST_CLEAN_STATUS {
        draw_rounded_card(hdc, card_x, status_y, card_w, status_h, 8, rgb(18, 36, 28), rgb(34, 76, 52));
        draw_string(hdc, msg, FONT_SMALL, rgb(52, 211, 153), card_x + 8, status_y, card_w - 16, status_h, DT_CENTER | DT_VCENTER);
    } else {
        draw_rounded_card(hdc, card_x, status_y, card_w, status_h, 8, rgb(20, 22, 30), rgb(32, 36, 48));
        draw_string(hdc, "Ready to optimize system memory", FONT_SMALL, rgb(148, 163, 184), card_x, status_y, card_w, status_h, DT_CENTER | DT_VCENTER);
    }
}

unsafe fn render_tab_settings(hdc: HDC, w: i32, _h: i32, config: &Config) {
    let card_x = 16;
    let card_w = w - 32;

    // 1. Setting: Auto-Clean Threshold
    let mut y = 56;
    draw_rounded_card(hdc, card_x, y, card_w, 88, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "AUTO-REDUCT ON THRESHOLD", FONT_SMALL, rgb(148, 163, 184), card_x + 14, y + 10, 200, 16, DT_LEFT);

    let tog_text = if config.autoreduct_enable { "ACTIVE" } else { "DISABLED" };
    let tog_bg = if config.autoreduct_enable { rgb(16, 185, 129) } else { rgb(45, 50, 68) };
    draw_rounded_card(hdc, card_x + card_w - 90, y + 10, 76, 20, 10, tog_bg, tog_bg);
    draw_string(hdc, tog_text, FONT_SMALL, rgb(255, 255, 255), card_x + card_w - 90, y + 10, 76, 20, DT_CENTER | DT_VCENTER);

    let limits = [70u8, 75, 80, 85, 90, 95];
    let pill_w = 42;
    let pill_h = 24;
    for (i, &lim) in limits.iter().enumerate() {
        let px = card_x + 14 + i as i32 * (pill_w + 7);
        let py = y + 44;
        let is_sel = config.autoreduct_limit == lim;
        let bg = if is_sel { rgb(99, 102, 241) } else { rgb(32, 36, 48) };
        let tc = if is_sel { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
        draw_rounded_card(hdc, px, py, pill_w, pill_h, 8, bg, bg);
        draw_string(hdc, &format!("{}%", lim), FONT_SMALL, tc, px, py, pill_w, pill_h, DT_CENTER | DT_VCENTER);
    }

    // 2. Setting: Periodic Interval
    y = 154;
    draw_rounded_card(hdc, card_x, y, card_w, 88, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "PERIODIC CLEANUP INTERVAL", FONT_SMALL, rgb(148, 163, 184), card_x + 14, y + 10, 200, 16, DT_LEFT);

    let int_text = if config.autoreduct_interval_enable { "ACTIVE" } else { "DISABLED" };
    let int_bg = if config.autoreduct_interval_enable { rgb(16, 185, 129) } else { rgb(45, 50, 68) };
    draw_rounded_card(hdc, card_x + card_w - 90, y + 10, 76, 20, 10, int_bg, int_bg);
    draw_string(hdc, int_text, FONT_SMALL, rgb(255, 255, 255), card_x + card_w - 90, y + 10, 76, 20, DT_CENTER | DT_VCENTER);

    let intervals = [15u64, 30, 45, 60, 120];
    let ipill_w = 52;
    for (i, &iv) in intervals.iter().enumerate() {
        let px = card_x + 14 + i as i32 * (ipill_w + 8);
        let py = y + 44;
        let is_sel = config.autoreduct_interval_minutes == iv;
        let bg = if is_sel { rgb(99, 102, 241) } else { rgb(32, 36, 48) };
        let tc = if is_sel { rgb(255, 255, 255) } else { rgb(160, 170, 190) };
        draw_rounded_card(hdc, px, py, ipill_w, pill_h, 8, bg, bg);
        draw_string(hdc, &format!("{}m", iv), FONT_SMALL, tc, px, py, ipill_w, pill_h, DT_CENTER | DT_VCENTER);
    }

    // 3. Setting: Behavior & Always on top
    y = 252;
    draw_rounded_card(hdc, card_x, y, card_w, 64, 12, rgb(24, 27, 36), rgb(38, 43, 58));
    draw_string(hdc, "WINDOW ALWAYS ON TOP", FONT_SUBTITLE, rgb(235, 240, 250), card_x + 14, y + 20, 180, 24, DT_LEFT | DT_VCENTER);

    let top_text = if config.always_on_top { "ON" } else { "OFF" };
    let top_bg = if config.always_on_top { rgb(99, 102, 241) } else { rgb(45, 50, 68) };
    draw_rounded_card(hdc, card_x + card_w - 70, y + 18, 56, 24, 12, top_bg, top_bg);
    draw_string(hdc, top_text, FONT_SUBTITLE, rgb(255, 255, 255), card_x + card_w - 70, y + 18, 56, 24, DT_CENTER | DT_VCENTER);

    // 4. Reset Button
    y = 326;
    let rst_bg = if HOVER_TARGET == Some(ClickTarget::SettingReset) { rgb(60, 30, 36) } else { rgb(36, 22, 26) };
    draw_rounded_card(hdc, card_x, y, card_w, 40, 10, rst_bg, rgb(80, 36, 42));
    draw_string(hdc, "↺  Reset Settings to Default", FONT_SUBTITLE, rgb(248, 113, 113), card_x, y, card_w, 40, DT_CENTER | DT_VCENTER);

    // 5. Save Button
    y = 380;
    let save_bg = if HOVER_TARGET == Some(ClickTarget::SettingSave) { rgb(52, 211, 153) } else { rgb(16, 185, 129) };
    draw_rounded_card(hdc, card_x, y, card_w, 44, 12, save_bg, save_bg);
    draw_string(hdc, "💾  SAVE & RETURN TO MONITOR", FONT_BTN, rgb(255, 255, 255), card_x, y, card_w, 44, DT_CENTER | DT_VCENTER);
}

fn hit_test(x: i32, y: i32, w: i32) -> Option<ClickTarget> {
    if y >= 10 && y <= 38 {
        if x >= w - 34 && x <= w - 8 {
            return Some(ClickTarget::BtnClose);
        }
        if x >= w - 64 && x <= w - 38 {
            return Some(ClickTarget::BtnMinimize);
        }
        if x >= 170 && x <= 232 {
            return Some(ClickTarget::TabOverview);
        }
        if x >= 238 && x <= 302 {
            return Some(ClickTarget::TabSettings);
        }
    }

    unsafe {
        match ACTIVE_TAB {
            ActiveTab::Overview => {
                let card_x = 16;
                let card_w = w - 32;

                if x >= card_x && x <= card_x + card_w && y >= 398 && y <= 442 {
                    return Some(ClickTarget::CleanButton);
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
                let chip_h = 24;

                for (bit, col, row) in regions {
                    let cx = card_x + col * (chip_w + 8);
                    let cy = 282 + row * (chip_h + 6);
                    if x >= cx && x <= cx + chip_w && y >= cy && y <= cy + chip_h {
                        return Some(ClickTarget::Chip(bit));
                    }
                }
            }
            ActiveTab::Settings => {
                let card_x = 16;
                let card_w = w - 32;

                if x >= card_x + card_w - 90 && x <= card_x + card_w - 14 && y >= 66 && y <= 86 {
                    return Some(ClickTarget::SettingAutoToggle);
                }

                let limits = [70u8, 75, 80, 85, 90, 95];
                let pill_w = 42;
                let pill_h = 24;
                for (i, &lim) in limits.iter().enumerate() {
                    let px = card_x + 14 + i as i32 * (pill_w + 7);
                    let py = 100;
                    if x >= px && x <= px + pill_w && y >= py && y <= py + pill_h {
                        return Some(ClickTarget::SettingLimit(lim));
                    }
                }

                if x >= card_x + card_w - 90 && x <= card_x + card_w - 14 && y >= 164 && y <= 184 {
                    return Some(ClickTarget::SettingIntervalToggle);
                }

                let intervals = [15u64, 30, 45, 60, 120];
                let ipill_w = 52;
                for (i, &iv) in intervals.iter().enumerate() {
                    let px = card_x + 14 + i as i32 * (ipill_w + 8);
                    let py = 198;
                    if x >= px && x <= px + ipill_w && y >= py && y <= py + pill_h {
                        return Some(ClickTarget::SettingInterval(iv));
                    }
                }

                if x >= card_x + card_w - 70 && x <= card_x + card_w - 14 && y >= 270 && y <= 294 {
                    return Some(ClickTarget::SettingAlwaysOnTop);
                }

                if x >= card_x && x <= card_x + card_w && y >= 326 && y <= 366 {
                    return Some(ClickTarget::SettingReset);
                }

                if x >= card_x && x <= card_x + card_w && y >= 380 && y <= 424 {
                    return Some(ClickTarget::SettingSave);
                }
            }
        }
    }
    None
}

unsafe fn setup_tray_icon(hwnd: HWND) {
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

            SetTimer(hwnd, 1, 1000, None);
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
                    ClickTarget::Chip(bit) => {
                        CURRENT_MASK ^= bit;
                        config.reduct_mask = CURRENT_MASK;
                        let _ = config.save();
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
                    ClickTarget::SettingAlwaysOnTop => {
                        config.always_on_top = !config.always_on_top;
                        let insert = if config.always_on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
                        SetWindowPos(hwnd, insert, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                        let _ = config.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingReset => {
                        let def = Config::default();
                        CURRENT_MASK = def.reduct_mask;
                        let _ = def.save();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    ClickTarget::SettingSave => {
                        config.reduct_mask = CURRENT_MASK;
                        let _ = config.save();
                        ACTIVE_TAB = ActiveTab::Overview;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
            } else if y <= 48 {
                // Header bar dragging
                ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as WPARAM, 0);
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
                        LAST_CLEAN_STATUS = Some(format!(
                            "✓ Memory released: {} ({}% -> {}) in {:.1?}",
                            freed, rep.mem_before.percent_physical, rep.mem_after.percent_physical, rep.duration
                        ));
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
                let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = 1;
                nid.uFlags = NIF_TIP;

                let tip = format!("Mem Reduct: {}% RAM used", info.percent_physical);
                let tip_wide = to_wide(&tip);
                for (i, &c) in tip_wide.iter().take(127).enumerate() {
                    nid.szTip[i] = c;
                }
                Shell_NotifyIconW(NIM_MODIFY, &mut nid);
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
                let htray = CreatePopupMenu();
                AppendMenuW(htray, MF_STRING, IDM_TRAY_SHOWHIDE, to_wide("Show / Hide").as_ptr());
                AppendMenuW(htray, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray, MF_STRING, IDM_TRAY_CLEAN, to_wide("⚡ Clean Memory").as_ptr());
                AppendMenuW(htray, MF_SEPARATOR, 0, ptr::null());
                AppendMenuW(htray, MF_STRING, IDM_TRAY_EXIT, to_wide("Exit").as_ptr());

                SetForegroundWindow(hwnd);
                TrackPopupMenu(htray, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, ptr::null());
            }
            0
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            if id == IDM_TRAY_SHOWHIDE {
                if IsWindowVisible(hwnd) != 0 {
                    ShowWindow(hwnd, SW_HIDE);
                } else {
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                }
            } else if id == IDM_TRAY_CLEAN {
                do_clean(hwnd);
            } else if id == IDM_TRAY_EXIT {
                PostQuitMessage(0);
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

        let window_title = to_wide("Mem Reduct");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            410,
            545,
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

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

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
