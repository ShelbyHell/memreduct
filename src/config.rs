use crate::cleaner::REDUCT_MASK_DEFAULT;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Config {
    pub autoreduct_enable: bool,
    pub autoreduct_limit: u8,
    pub autoreduct_interval_enable: bool,
    pub autoreduct_interval_minutes: u64,
    pub reduct_mask: u32,
    pub cooldown_seconds: u64,
    pub dark_theme: bool,
    pub always_on_top: bool,
    pub tray_minimized: bool,
    pub load_on_startup: bool,
    pub start_minimized: bool,
    pub balloon_notifications: bool,
    pub notification_sound: bool,
    pub hotkey_enable: bool,
    pub hotkey_modifiers: u32,
    pub hotkey_vk: u32,
    pub allow_standby_cleanup: bool,
    pub warning_level: u8,
    pub danger_level: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            autoreduct_enable: false,
            autoreduct_limit: 90,
            autoreduct_interval_enable: false,
            autoreduct_interval_minutes: 30,
            reduct_mask: REDUCT_MASK_DEFAULT,
            cooldown_seconds: 30,
            dark_theme: true,
            always_on_top: false,
            tray_minimized: false,
            load_on_startup: false,
            start_minimized: false,
            balloon_notifications: true,
            notification_sound: true,
            hotkey_enable: true,
            hotkey_modifiers: 0x0002, // MOD_CONTROL
            hotkey_vk: 0x70,          // VK_F1
            allow_standby_cleanup: false,
            warning_level: 70,
            danger_level: 90,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let mut config = Config::default();

        let paths = ["memreduct.toml", "memreduct.ini"];
        for path in &paths {
            if Path::new(path).exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    config.parse_content(&content);
                    break;
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            config.load_on_startup = is_autostart_enabled();
        }

        config
    }

    pub fn save(&self) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            sync_autostart(self.load_on_startup);
        }

        let content = format!(
            "# Mem Reduct Configuration File\n\n\
            [general]\n\
            dark_theme = {}\n\
            always_on_top = {}\n\
            tray_minimized = {}\n\
            load_on_startup = {}\n\
            start_minimized = {}\n\
            balloon_notifications = {}\n\
            notification_sound = {}\n\
            hotkey_enable = {}\n\
            hotkey_modifiers = {}\n\
            hotkey_vk = {}\n\
            allow_standby_cleanup = {}\n\n\
            [auto_clean]\n\
            autoreduct_enable = {}\n\
            autoreduct_limit = {}\n\
            autoreduct_interval_enable = {}\n\
            autoreduct_interval_minutes = {}\n\
            cooldown_seconds = {}\n\
            reduct_mask = {}\n\n\
            [levels]\n\
            warning_level = {}\n\
            danger_level = {}\n",
            self.dark_theme,
            self.always_on_top,
            self.tray_minimized,
            self.load_on_startup,
            self.start_minimized,
            self.balloon_notifications,
            self.notification_sound,
            self.hotkey_enable,
            self.hotkey_modifiers,
            self.hotkey_vk,
            self.allow_standby_cleanup,
            self.autoreduct_enable,
            self.autoreduct_limit,
            self.autoreduct_interval_enable,
            self.autoreduct_interval_minutes,
            self.cooldown_seconds,
            self.reduct_mask,
            self.warning_level,
            self.danger_level
        );

        fs::write("memreduct.toml", content)
            .map_err(|e| format!("Failed to write memreduct.toml: {}", e))
    }

    pub fn parse_content(&mut self, content: &str) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with('[') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }

            let key = parts[0].trim().to_lowercase();
            let val = parts[1].trim().trim_matches('"');

            match key.as_str() {
                "trayminimized" | "tray_minimized" | "close_to_tray" => {
                    self.tray_minimized = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "loadonstartup" | "load_on_startup" | "autostart" => {
                    self.load_on_startup = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "startminimized" | "start_minimized" => {
                    self.start_minimized = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "ballooncleanresults" | "balloon_notifications" | "notifications" => {
                    self.balloon_notifications = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "isnotificationssound" | "notification_sound" | "sound" => {
                    self.notification_sound = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "hotkeycleanenable" | "hotkey_enable" | "hotkey" => {
                    self.hotkey_enable = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "hotkey_modifiers" | "hotkeymodifiers" | "hotkey_mods" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.hotkey_modifiers = v;
                    }
                }
                "hotkey_vk" | "hotkeyvk" | "hotkey_key" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.hotkey_vk = v;
                    }
                }
                "isallowstandbylistcleanup" | "allow_standby_cleanup" => {
                    self.allow_standby_cleanup = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "autoreductenable" | "auto_reduct_enable" | "auto_clean" => {
                    self.autoreduct_enable = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "autoreductval" | "autoreduct_val" | "limit" | "threshold" | "autoreduct_limit" => {
                    if let Ok(v) = val.parse::<u8>() {
                        self.autoreduct_limit = v.min(100);
                    }
                }
                "autoreductintervalenable" | "auto_interval_enable" | "autoreduct_interval_enable" => {
                    self.autoreduct_interval_enable = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "autoreductinterval" | "interval" | "interval_minutes" | "autoreduct_interval_minutes" => {
                    if let Ok(v) = val.parse::<u64>() {
                        self.autoreduct_interval_minutes = v;
                    }
                }
                "reductmask" | "mask" | "reduct_mask" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.reduct_mask = v;
                    }
                }
                "cooldown" | "cooldown_seconds" => {
                    if let Ok(v) = val.parse::<u64>() {
                        self.cooldown_seconds = v;
                    }
                }
                "dark_theme" | "darktheme" | "usedarktheme" => {
                    self.dark_theme = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "always_on_top" | "alwaysontop" => {
                    self.always_on_top = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "warning_level" | "traylevelwarning" => {
                    if let Ok(v) = val.parse::<u8>() {
                        self.warning_level = v.min(100);
                    }
                }
                "danger_level" | "trayleveldanger" => {
                    if let Ok(v) = val.parse::<u8>() {
                        self.danger_level = v.min(100);
                    }
                }
                _ => {}
            }
        }
    }
}

#[allow(dead_code)]
pub fn format_hotkey(modifiers: u32, vk: u32) -> String {
    let mut parts = Vec::new();
    // MOD_CONTROL = 0x0002, MOD_ALT = 0x0001, MOD_SHIFT = 0x0004, MOD_WIN = 0x0008
    if (modifiers & 0x0002) != 0 {
        parts.push("Ctrl");
    }
    if (modifiers & 0x0001) != 0 {
        parts.push("Alt");
    }
    if (modifiers & 0x0004) != 0 {
        parts.push("Shift");
    }
    if (modifiers & 0x0008) != 0 {
        parts.push("Win");
    }

    let key_name = match vk {
        0x70..=0x87 => format!("F{}", vk - 0x70 + 1), // F1 to F24
        0x30..=0x39 => format!("{}", (vk as u8) as char), // 0-9
        0x41..=0x5A => format!("{}", (vk as u8) as char), // A-Z
        0x20 => "Space".to_string(),
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Esc".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x6A => "Num *".to_string(),
        0x6B => "Num +".to_string(),
        0x6D => "Num -".to_string(),
        0x6E => "Num .".to_string(),
        0x6F => "Num /".to_string(),
        0xBC => ",".to_string(),
        0xBD => "-".to_string(),
        0xBE => ".".to_string(),
        0xBF => "/".to_string(),
        0xC0 => "`".to_string(),
        0xDB => "[".to_string(),
        0xDC => "\\".to_string(),
        0xDD => "]".to_string(),
        0xDE => "'".to_string(),
        _ => format!("Key(0x{:X})", vk),
    };
    parts.push(&key_name);
    parts.join(" + ")
}

#[cfg(target_os = "windows")]
pub fn sync_autostart(enable: bool) {
    unsafe {
        type HKEY = *mut std::ffi::c_void;
        const HKEY_CURRENT_USER: HKEY = 0x80000001u32 as usize as HKEY;
        const KEY_SET_VALUE: u32 = 0x0002;
        const REG_SZ: u32 = 1;

        #[link(name = "advapi32")]
        extern "system" {
            fn RegOpenKeyExW(
                hKey: HKEY,
                lpSubKey: *const u16,
                ulOptions: u32,
                samDesired: u32,
                phkResult: *mut HKEY,
            ) -> i32;
            fn RegSetValueExW(
                hKey: HKEY,
                lpValueName: *const u16,
                Reserved: u32,
                dwType: u32,
                lpData: *const u8,
                cbData: u32,
            ) -> i32;
            fn RegDeleteValueW(hKey: HKEY, lpValueName: *const u16) -> i32;
            fn RegCloseKey(hKey: HKEY) -> i32;
        }

        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0".encode_utf16().collect();
        let val_name: Vec<u16> = "Mem Reduct\0".encode_utf16().collect();

        let mut hk: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hk) == 0 {
            if enable {
                if let Ok(exe_path) = std::env::current_exe() {
                    let cmd = format!("\"{}\"\0", exe_path.display());
                    let cmd_wide: Vec<u16> = cmd.encode_utf16().collect();
                    let bytes = std::slice::from_raw_parts(cmd_wide.as_ptr() as *const u8, cmd_wide.len() * 2);
                    RegSetValueExW(hk, val_name.as_ptr(), 0, REG_SZ, bytes.as_ptr(), bytes.len() as u32);
                }
            } else {
                RegDeleteValueW(hk, val_name.as_ptr());
            }
            RegCloseKey(hk);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn is_autostart_enabled() -> bool {
    unsafe {
        type HKEY = *mut std::ffi::c_void;
        const HKEY_CURRENT_USER: HKEY = 0x80000001u32 as usize as HKEY;
        const KEY_QUERY_VALUE: u32 = 0x0001;

        #[link(name = "advapi32")]
        extern "system" {
            fn RegOpenKeyExW(
                hKey: HKEY,
                lpSubKey: *const u16,
                ulOptions: u32,
                samDesired: u32,
                phkResult: *mut HKEY,
            ) -> i32;
            fn RegQueryValueExW(
                hKey: HKEY,
                lpValueName: *const u16,
                lpReserved: *mut u32,
                lpType: *mut u32,
                lpData: *mut u8,
                lpcbData: *mut u32,
            ) -> i32;
            fn RegCloseKey(hKey: HKEY) -> i32;
        }

        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0".encode_utf16().collect();
        let val_name: Vec<u16> = "Mem Reduct\0".encode_utf16().collect();

        let mut hk: HKEY = std::ptr::null_mut();
        let mut enabled = false;
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hk) == 0 {
            let mut cb_data = 0u32;
            if RegQueryValueExW(hk, val_name.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut cb_data) == 0 {
                enabled = true;
            }
            RegCloseKey(hk);
        }
        enabled
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn sync_autostart(_enable: bool) {}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn is_autostart_enabled() -> bool { false }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content() {
        let content = r#"
            # Settings
            autoreductenable = true
            limit = 85
            interval = 45
            mask = 255
            cooldown = 15
            dark_theme = true
            always_on_top = true
            hotkey_modifiers = 3
            hotkey_vk = 112
        "#;
        let mut cfg = Config::default();
        cfg.parse_content(content);

        assert!(cfg.autoreduct_enable);
        assert_eq!(cfg.autoreduct_limit, 85);
        assert_eq!(cfg.autoreduct_interval_minutes, 45);
        assert_eq!(cfg.reduct_mask, 255);
        assert_eq!(cfg.cooldown_seconds, 15);
        assert!(cfg.dark_theme);
        assert!(cfg.always_on_top);
        assert_eq!(cfg.hotkey_modifiers, 3);
        assert_eq!(cfg.hotkey_vk, 112);
    }

    #[test]
    fn test_format_hotkey() {
        assert_eq!(format_hotkey(0x0002, 0x70), "Ctrl + F1");
        assert_eq!(format_hotkey(0x0001 | 0x0002, 0x52), "Ctrl + Alt + R");
        assert_eq!(format_hotkey(0x0004 | 0x0008, 0x20), "Shift + Win + Space");
        assert_eq!(format_hotkey(0, 0x74), "F5");
    }
}
