use crate::cleaner::{
    self, clean_memory, CleanReport, REDUCT_COMBINEMEMORYLISTS, REDUCT_MASK_ALL,
    REDUCT_MASK_DEFAULT, REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST, REDUCT_REGISTRYCACHE,
    REDUCT_STANDBYLIST, REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE, REDUCT_WORKINGSET,
};
use crate::config::Config;
use crate::system::{self, format_bytes, MemoryInfo};
use crate::ui::{
    get_level_color, render_bar, COLOR_BOLD, COLOR_CYAN, COLOR_GRAY, COLOR_GREEN, COLOR_RED,
    COLOR_RESET, COLOR_YELLOW,
};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub const COLOR_MAGENTA: &str = "\x1B[35m";
pub const COLOR_BLUE: &str = "\x1B[34m";
pub const COLOR_WHITE_BOLD: &str = "\x1B[1;37m";

#[derive(Debug, PartialEq, Eq)]
enum TuiView {
    Dashboard,
    Settings,
}

#[derive(Debug, PartialEq, Eq)]
enum Key {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
}

#[cfg(unix)]
mod raw_terminal {
    use std::mem::zeroed;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_line: u8,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }

    const ICANON: u32 = 0o000002;
    const ECHO: u32 = 0o000010;
    const TCSANOW: i32 = 0;
    const VMIN: usize = 6;
    const VTIME: usize = 5;

    extern "C" {
        fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
    }

    pub struct RawGuard {
        orig: Termios,
    }

    impl RawGuard {
        pub fn enter() -> Option<Self> {
            unsafe {
                let mut orig: Termios = zeroed();
                if tcgetattr(0, &mut orig) != 0 {
                    return None;
                }
                let mut raw = orig;
                raw.c_lflag &= !(ICANON | ECHO);
                raw.c_cc[VMIN] = 0;
                raw.c_cc[VTIME] = 1; // 100ms timeout
                if tcsetattr(0, TCSANOW, &raw) != 0 {
                    return None;
                }
                Some(RawGuard { orig })
            }
        }
    }

    impl Drop for RawGuard {
        fn drop(&mut self) {
            unsafe {
                tcsetattr(0, TCSANOW, &self.orig);
            }
        }
    }
}

pub fn run_interactive_tui(running: Arc<AtomicBool>) {
    #[cfg(unix)]
    let _guard = raw_terminal::RawGuard::enter();

    let mut config = Config::load();
    let mut current_mask = if config.reduct_mask != 0 {
        config.reduct_mask
    } else {
        REDUCT_MASK_DEFAULT
    };

    let mut view = TuiView::Dashboard;
    let mut settings_cursor: usize = 0;

    let mut last_clean = Instant::now() - Duration::from_secs(config.cooldown_seconds);
    let mut last_interval_clean = Instant::now();
    let mut last_report: Option<CleanReport> = None;
    let mut status_msg: Option<(String, &'static str)> = None;

    // Hide cursor and clear screen
    print!("\x1B[?25l\x1B[2J\x1B[H");
    io::stdout().flush().ok();

    while running.load(Ordering::SeqCst) {
        let mem_info = system::get_memory_info().unwrap_or_default();

        // Check auto-reduction
        let cooldown = Duration::from_secs(config.cooldown_seconds);
        if cleaner::is_elevated() {
            let mut triggered = false;
            let mut reason = "";

            if config.autoreduct_enable
                && mem_info.percent_physical >= config.autoreduct_limit
                && last_clean.elapsed() >= cooldown
            {
                triggered = true;
                reason = "Threshold reached";
            } else if config.autoreduct_interval_enable
                && last_interval_clean.elapsed()
                    >= Duration::from_secs(config.autoreduct_interval_minutes * 60)
            {
                triggered = true;
                reason = "Interval elapsed";
            }

            if triggered {
                match clean_memory(current_mask) {
                    Ok(report) => {
                        let freed = format_bytes(report.bytes_freed);
                        status_msg = Some((
                            format!("Auto-clean ({}) freed {} in {:.1?}", reason, freed, report.duration),
                            COLOR_GREEN,
                        ));
                        last_report = Some(report);
                        last_clean = Instant::now();
                        last_interval_clean = Instant::now();
                    }
                    Err(e) => {
                        status_msg = Some((format!("Auto-clean error: {}", e), COLOR_RED));
                    }
                }
            }
        }

        // Render Current View
        match view {
            TuiView::Dashboard => {
                render_dashboard(
                    &mem_info,
                    current_mask,
                    &config,
                    last_report.as_ref(),
                    status_msg.as_ref(),
                );
            }
            TuiView::Settings => {
                render_settings_view(&config, current_mask, settings_cursor, status_msg.as_ref());
            }
        }

        // Process Key Input
        if let Some(key) = read_key() {
            match view {
                TuiView::Dashboard => match key {
                    Key::Char('q') | Key::Char('Q') => break,
                    Key::Esc => break,
                    Key::Char('s') | Key::Char('S') => {
                        view = TuiView::Settings;
                        settings_cursor = 0;
                        status_msg = None;
                    }
                    Key::Char(' ') | Key::Char('c') | Key::Char('C') => {
                        match clean_memory(current_mask) {
                            Ok(report) => {
                                let freed = format_bytes(report.bytes_freed);
                                status_msg = Some((
                                    format!("Cleaned: {} freed ({}% -> {}%) in {:.1?}", freed, report.mem_before.percent_physical, report.mem_after.percent_physical, report.duration),
                                    COLOR_GREEN,
                                ));
                                last_report = Some(report);
                                last_clean = Instant::now();
                                last_interval_clean = Instant::now();
                            }
                            Err(e) => {
                                status_msg = Some((format!("Error: {}", e), COLOR_RED));
                            }
                        }
                    }
                    Key::Char('f') | Key::Char('F') => {
                        match clean_memory(REDUCT_MASK_ALL) {
                            Ok(report) => {
                                let freed = format_bytes(report.bytes_freed);
                                status_msg = Some((
                                    format!("Full Clean: {} freed in {:.1?}", freed, report.duration),
                                    COLOR_GREEN,
                                ));
                                last_report = Some(report);
                                last_clean = Instant::now();
                                last_interval_clean = Instant::now();
                            }
                            Err(e) => {
                                status_msg = Some((format!("Error: {}", e), COLOR_RED));
                            }
                        }
                    }
                    Key::Char('1') => current_mask ^= REDUCT_WORKINGSET,
                    Key::Char('2') => current_mask ^= REDUCT_SYSTEMFILECACHE,
                    Key::Char('3') => current_mask ^= REDUCT_STANDBYPRIORITY0LIST,
                    Key::Char('4') => current_mask ^= REDUCT_STANDBYLIST,
                    Key::Char('5') => current_mask ^= REDUCT_MODIFIEDLIST,
                    Key::Char('6') => current_mask ^= REDUCT_COMBINEMEMORYLISTS,
                    Key::Char('7') => current_mask ^= REDUCT_REGISTRYCACHE,
                    Key::Char('8') => current_mask ^= REDUCT_MODIFIEDFILECACHE,
                    Key::Char('a') | Key::Char('A') => {
                        config.autoreduct_enable = !config.autoreduct_enable;
                        let _ = config.save();
                        status_msg = Some((
                            format!("Auto-clean threshold {}", if config.autoreduct_enable { "ENABLED" } else { "DISABLED" }),
                            COLOR_YELLOW,
                        ));
                    }
                    Key::Char('t') | Key::Char('T') => {
                        config.autoreduct_limit = match config.autoreduct_limit {
                            70 => 75,
                            75 => 80,
                            80 => 85,
                            85 => 90,
                            90 => 95,
                            _ => 70,
                        };
                        let _ = config.save();
                        status_msg = Some((format!("Threshold set to {}%", config.autoreduct_limit), COLOR_CYAN));
                    }
                    Key::Char('i') | Key::Char('I') => {
                        config.autoreduct_interval_enable = !config.autoreduct_interval_enable;
                        let _ = config.save();
                        status_msg = Some((
                            format!("Periodic interval {}", if config.autoreduct_interval_enable { "ENABLED" } else { "DISABLED" }),
                            COLOR_YELLOW,
                        ));
                    }
                    _ => {}
                },
                TuiView::Settings => match key {
                    Key::Esc | Key::Char('s') | Key::Char('S') | Key::Char('q') => {
                        config.reduct_mask = current_mask;
                        let _ = config.save();
                        view = TuiView::Dashboard;
                        status_msg = Some(("Settings saved to memreduct.toml".to_string(), COLOR_GREEN));
                    }
                    Key::Up | Key::Char('k') => {
                        if settings_cursor > 0 {
                            settings_cursor -= 1;
                        }
                    }
                    Key::Down | Key::Char('j') => {
                        if settings_cursor < 7 {
                            settings_cursor += 1;
                        }
                    }
                    Key::Left | Key::Right | Key::Enter | Key::Char(' ') => match settings_cursor {
                        0 => {
                            config.autoreduct_enable = !config.autoreduct_enable;
                        }
                        1 => {
                            if key == Key::Left {
                                config.autoreduct_limit = config.autoreduct_limit.saturating_sub(5).max(50);
                            } else {
                                config.autoreduct_limit = (config.autoreduct_limit + 5).min(95);
                            }
                        }
                        2 => {
                            config.autoreduct_interval_enable = !config.autoreduct_interval_enable;
                        }
                        3 => {
                            if key == Key::Left {
                                config.autoreduct_interval_minutes = config.autoreduct_interval_minutes.saturating_sub(5).max(5);
                            } else {
                                config.autoreduct_interval_minutes = (config.autoreduct_interval_minutes + 5).min(180);
                            }
                        }
                        4 => {
                            if key == Key::Left {
                                config.cooldown_seconds = config.cooldown_seconds.saturating_sub(5).max(5);
                            } else {
                                config.cooldown_seconds = (config.cooldown_seconds + 5).min(120);
                            }
                        }
                        5 => {
                            config.dark_theme = !config.dark_theme;
                        }
                        6 => {
                            config = Config::default();
                            current_mask = config.reduct_mask;
                            status_msg = Some(("Settings reset to default".to_string(), COLOR_YELLOW));
                        }
                        7 => {
                            config.reduct_mask = current_mask;
                            let _ = config.save();
                            view = TuiView::Dashboard;
                            status_msg = Some(("Saved & applied successfully".to_string(), COLOR_GREEN));
                        }
                        _ => {}
                    },
                    _ => {}
                },
            }
        }

        thread::sleep(Duration::from_millis(120));
    }

    // Show cursor and clear screen
    print!("\x1B[?25h\x1B[2J\x1B[H");
    io::stdout().flush().ok();
    println!("Mem Reduct closed.");
}

fn read_key() -> Option<Key> {
    let mut buf = [0u8; 4];
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    match handle.read(&mut buf) {
        Ok(1) => {
            let c = buf[0] as char;
            if buf[0] == 27 {
                Some(Key::Esc)
            } else if buf[0] == 10 || buf[0] == 13 {
                Some(Key::Enter)
            } else {
                Some(Key::Char(c))
            }
        }
        Ok(3) if buf[0] == 27 && buf[1] == b'[' => match buf[2] {
            b'A' => Some(Key::Up),
            b'B' => Some(Key::Down),
            b'C' => Some(Key::Right),
            b'D' => Some(Key::Left),
            _ => None,
        },
        _ => None,
    }
}

fn render_dashboard(
    info: &MemoryInfo,
    mask: u32,
    config: &Config,
    last_report: Option<&CleanReport>,
    status_msg: Option<&(String, &'static str)>,
) {
    let mut out = String::with_capacity(4096);
    out.push_str("\x1B[H"); // Cursor to top-left

    // Stylish Header
    out.push_str(&format!(
        "{}{}╭──────────────────────────────────────────────────────────────╮{}\n",
        COLOR_BOLD, COLOR_CYAN, COLOR_RESET
    ));
    out.push_str(&format!(
        "{}{}│                 ⚡  MEM REDUCT v3.5.3  ⚡                     │{}\n",
        COLOR_WHITE_BOLD, COLOR_CYAN, COLOR_RESET
    ));
    out.push_str(&format!(
        "{}{}╰──────────────────────────────────────────────────────────────╯{}\n",
        COLOR_BOLD, COLOR_CYAN, COLOR_RESET
    ));

    // Privilege indicator
    if cleaner::is_elevated() {
        out.push_str(&format!(
            "  {}● ELEVATED PRIVILEGES ACTIVE{}    {}⚡ Pure Rust Engine{}\n\n",
            COLOR_GREEN, COLOR_RESET, COLOR_GRAY, COLOR_RESET
        ));
    } else {
        out.push_str(&format!(
            "  {}⚠ NON-ELEVATED: Please run with sudo / admin for cleanup!{}\n\n",
            COLOR_YELLOW, COLOR_RESET
        ));
    }

    // 1. Physical Memory Box
    let p_color = get_level_color(info.percent_physical);
    out.push_str(&format!(
        "  {}┌─ Physical Memory ────────────────────────────────────────┐{}\n",
        COLOR_CYAN, COLOR_RESET
    ));
    out.push_str(&format!(
        "  {}│{}  Usage:      {}{:>3}%{}  {}   │\n",
        COLOR_CYAN,
        COLOR_RESET,
        p_color,
        info.percent_physical,
        COLOR_RESET,
        render_bar(info.percent_physical, 20)
    ));
    out.push_str(&format!(
        "  {}│{}  Available:  {:<10} (Used: {})               │\n",
        COLOR_CYAN,
        COLOR_RESET,
        format_bytes(info.avail_physical),
        format_bytes(info.used_physical)
    ));
    out.push_str(&format!(
        "  {}│{}  Total:      {:<10}                                │\n",
        COLOR_CYAN,
        COLOR_RESET,
        format_bytes(info.total_physical)
    ));
    out.push_str(&format!(
        "  {}└──────────────────────────────────────────────────────────┘{}\n\n",
        COLOR_CYAN, COLOR_RESET
    ));

    // 2. Pagefile Box
    out.push_str(&format!(
        "  {}┌─ Pagefile / Swap ─────────────────────────────────────────┐{}\n",
        COLOR_BLUE, COLOR_RESET
    ));
    if info.total_pagefile > 0 {
        let pf_color = get_level_color(info.percent_pagefile);
        out.push_str(&format!(
            "  {}│{}  Usage:      {}{:>3}%{}  {}   │\n",
            COLOR_BLUE,
            COLOR_RESET,
            pf_color,
            info.percent_pagefile,
            COLOR_RESET,
            render_bar(info.percent_pagefile, 20)
        ));
        out.push_str(&format!(
            "  {}│{}  Available:  {:<10} (Used: {})               │\n",
            COLOR_BLUE,
            COLOR_RESET,
            format_bytes(info.avail_pagefile),
            format_bytes(info.used_pagefile)
        ));
        out.push_str(&format!(
            "  {}│{}  Total:      {:<10}                                │\n",
            COLOR_BLUE,
            COLOR_RESET,
            format_bytes(info.total_pagefile)
        ));
    } else {
        out.push_str(&format!(
            "  {}│{}  No Pagefile / Swap active                             │\n",
            COLOR_BLUE, COLOR_RESET
        ));
    }
    out.push_str(&format!(
        "  {}└──────────────────────────────────────────────────────────┘{}\n\n",
        COLOR_BLUE, COLOR_RESET
    ));

    // 3. System Cache Summary
    let cached_str = info.cached.map(format_bytes).unwrap_or_else(|| "N/A".into());
    let buffers_str = info.buffers.map(format_bytes).unwrap_or_else(|| "N/A".into());
    out.push_str(&format!(
        "  {}System Working Set / Cache:{} {}  |  {}Buffers:{} {}\n\n",
        COLOR_BOLD, COLOR_RESET, cached_str, COLOR_BOLD, COLOR_RESET, buffers_str
    ));

    // Big Action Button
    out.push_str(&format!(
        "              {}╔════════════════════════════════════════╗{}\n",
        COLOR_GREEN, COLOR_RESET
    ));
    out.push_str(&format!(
        "              {}║         ⚡  [ CLEAN MEMORY ]  ⚡         ║{}  [SPACE / C]\n",
        COLOR_WHITE_BOLD, COLOR_RESET
    ));
    out.push_str(&format!(
        "              {}╚════════════════════════════════════════╝{}\n\n",
        COLOR_GREEN, COLOR_RESET
    ));

    // Status line
    if let Some((msg, col)) = status_msg {
        out.push_str(&format!("  {}Status:{} {}{}{}\n", COLOR_BOLD, COLOR_RESET, col, msg, COLOR_RESET));
    } else if let Some(rep) = last_report {
        out.push_str(&format!(
            "  {}Status:{} {}Cleaned {} in {:.1?} ({}% -> {}){}\n",
            COLOR_BOLD,
            COLOR_RESET,
            COLOR_GREEN,
            format_bytes(rep.bytes_freed),
            rep.duration,
            rep.mem_before.percent_physical,
            rep.mem_after.percent_physical,
            COLOR_RESET
        ));
    } else {
        out.push_str(&format!("  {}Status:{} Ready\n", COLOR_BOLD, COLOR_RESET));
    }
    out.push_str(&format!("  {}\n", "─".repeat(60)));

    // Memory Regions matrix
    let check = |bit: u32, num: &str, label: &str| {
        if (mask & bit) != 0 {
            format!("{}[✓] {}.{}{}", COLOR_GREEN, num, label, COLOR_RESET)
        } else {
            format!("{}[ ] {}.{}{}", COLOR_GRAY, num, label, COLOR_RESET)
        }
    };

    out.push_str(&format!(
        "  Areas: {} {} {} {}\n",
        check(REDUCT_WORKINGSET, "1", "WS"),
        check(REDUCT_SYSTEMFILECACHE, "2", "Cache"),
        check(REDUCT_STANDBYPRIORITY0LIST, "3", "Prio0"),
        check(REDUCT_STANDBYLIST, "4", "Standby")
    ));
    out.push_str(&format!(
        "         {} {} {} {}\n",
        check(REDUCT_MODIFIEDLIST, "5", "Modified"),
        check(REDUCT_COMBINEMEMORYLISTS, "6", "Combine"),
        check(REDUCT_REGISTRYCACHE, "7", "Registry"),
        check(REDUCT_MODIFIEDFILECACHE, "8", "Volume")
    ));

    // Settings Pill
    let auto_pill = if config.autoreduct_enable {
        format!("{}[ON: >= {}%]{}", COLOR_GREEN, config.autoreduct_limit, COLOR_RESET)
    } else {
        format!("{}[OFF]{}", COLOR_GRAY, COLOR_RESET)
    };
    let int_pill = if config.autoreduct_interval_enable {
        format!("{}[ON: {}m]{}", COLOR_GREEN, config.autoreduct_interval_minutes, COLOR_RESET)
    } else {
        format!("{}[OFF]{}", COLOR_GRAY, COLOR_RESET)
    };
    out.push_str(&format!(
        "  Config: Auto: {} | Interval: {} | Theme: {}\n\n",
        auto_pill, int_pill, if config.dark_theme { "DARK" } else { "LIGHT" }
    ));

    // Navigation Footer
    out.push_str(&format!(
        "  {}[SPACE] Clean  [F] Full Clean  [S] Settings  [1-8] Areas  [Q] Exit{}\n",
        COLOR_BOLD, COLOR_RESET
    ));

    print!("{}", out);
    io::stdout().flush().ok();
}

fn render_settings_view(
    config: &Config,
    mask: u32,
    cursor: usize,
    status_msg: Option<&(String, &'static str)>,
) {
    let mut out = String::with_capacity(4096);
    out.push_str("\x1B[H");

    out.push_str(&format!(
        "{}{}╭──────────────────────────────────────────────────────────────╮{}\n",
        COLOR_BOLD, COLOR_MAGENTA, COLOR_RESET
    ));
    out.push_str(&format!(
        "{}{}│                 ⚙  MEM REDUCT - SETTINGS  ⚙                   │{}\n",
        COLOR_WHITE_BOLD, COLOR_MAGENTA, COLOR_RESET
    ));
    out.push_str(&format!(
        "{}{}╰──────────────────────────────────────────────────────────────╯{}\n\n",
        COLOR_BOLD, COLOR_MAGENTA, COLOR_RESET
    ));

    let items = [
        format!(
            "1. Auto-Clean on Threshold:        {}",
            if config.autoreduct_enable {
                format!("{}[ ENABLED ]{}", COLOR_GREEN, COLOR_RESET)
            } else {
                format!("{}[ DISABLED ]{}", COLOR_GRAY, COLOR_RESET)
            }
        ),
        format!(
            "2. Threshold Percentage:           ◄ {:>2}% ►",
            config.autoreduct_limit
        ),
        format!(
            "3. Auto-Clean Periodic Interval:   {}",
            if config.autoreduct_interval_enable {
                format!("{}[ ENABLED ]{}", COLOR_GREEN, COLOR_RESET)
            } else {
                format!("{}[ DISABLED ]{}", COLOR_GRAY, COLOR_RESET)
            }
        ),
        format!(
            "4. Interval Duration:              ◄ {:>3} min ►",
            config.autoreduct_interval_minutes
        ),
        format!(
            "5. Cleanup Cooldown:               ◄ {:>2} sec ►",
            config.cooldown_seconds
        ),
        format!(
            "6. Dark Theme:                     {}",
            if config.dark_theme {
                format!("{}[ DARK ]{}", COLOR_CYAN, COLOR_RESET)
            } else {
                format!("{}[ LIGHT ]{}", COLOR_YELLOW, COLOR_RESET)
            }
        ),
        "7. Reset All Settings to Default".to_string(),
        "8. [ SAVE & APPLY CONFIGURATION ]".to_string(),
    ];

    for (i, item) in items.iter().enumerate() {
        if i == cursor {
            out.push_str(&format!("  {}➤  {}{}\n", COLOR_WHITE_BOLD, item, COLOR_RESET));
        } else {
            out.push_str(&format!("     {}{}\n", item, COLOR_RESET));
        }
    }

    out.push_str(&format!("\n  {}\n", "─".repeat(60)));
    out.push_str(&format!("  Active Clean Mask: 0x{:02X}\n\n", mask));

    if let Some((msg, col)) = status_msg {
        out.push_str(&format!("  {}{}{}\n\n", col, msg, COLOR_RESET));
    }

    out.push_str(&format!(
        "  {}[▲/▼] Navigate  [◄/► or ENTER] Change  [ESC / S] Back to Dashboard{}\n",
        COLOR_BOLD, COLOR_RESET
    ));

    print!("{}", out);
    io::stdout().flush().ok();
}
