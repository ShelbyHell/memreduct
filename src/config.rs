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

        config
    }

    pub fn save(&self) -> Result<(), String> {
        let content = format!(
            "# Mem Reduct Configuration File\n\n\
            [general]\n\
            dark_theme = {}\n\
            always_on_top = {}\n\
            tray_minimized = {}\n\n\
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
    }
}
