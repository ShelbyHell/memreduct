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

    fn parse_content(&mut self, content: &str) {
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
                "autoreductenable" | "auto_reduct_enable" | "auto_clean" => {
                    self.autoreduct_enable = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "autoreductval" | "autoreduct_val" | "limit" | "threshold" => {
                    if let Ok(v) = val.parse::<u8>() {
                        self.autoreduct_limit = v.min(100);
                    }
                }
                "autoreductintervalenable" | "auto_interval_enable" => {
                    self.autoreduct_interval_enable = val.eq_ignore_ascii_case("true") || val == "1";
                }
                "autoreductinterval" | "interval" | "interval_minutes" => {
                    if let Ok(v) = val.parse::<u64>() {
                        self.autoreduct_interval_minutes = v;
                    }
                }
                "reductmask" | "mask" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.reduct_mask = v;
                    }
                }
                "cooldown" | "cooldown_seconds" => {
                    if let Ok(v) = val.parse::<u64>() {
                        self.cooldown_seconds = v;
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
        "#;
        let mut cfg = Config::default();
        cfg.parse_content(content);

        assert!(cfg.autoreduct_enable);
        assert_eq!(cfg.autoreduct_limit, 85);
        assert_eq!(cfg.autoreduct_interval_minutes, 45);
        assert_eq!(cfg.reduct_mask, 255);
        assert_eq!(cfg.cooldown_seconds, 15);
    }
}
