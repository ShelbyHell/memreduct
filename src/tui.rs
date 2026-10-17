use crate::cleaner::{
    self, clean_memory, CleanReport, REDUCT_COMBINEMEMORYLISTS,
    REDUCT_MASK_ALL, REDUCT_MASK_DEFAULT, REDUCT_MODIFIEDFILECACHE, REDUCT_MODIFIEDLIST,
    REDUCT_REGISTRYCACHE, REDUCT_STANDBYLIST, REDUCT_STANDBYPRIORITY0LIST, REDUCT_SYSTEMFILECACHE,
    REDUCT_WORKINGSET,
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

    let config = Config::load();
    let mut current_mask = config.reduct_mask;
    if current_mask == 0 {
        current_mask = REDUCT_MASK_DEFAULT;
    }

    let mut auto_clean_enabled = config.autoreduct_enable;
    let mut auto_limit = config.autoreduct_limit;
    let mut auto_interval_enabled = config.autoreduct_interval_enable;
    let mut auto_interval_mins = config.autoreduct_interval_minutes;

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

            if auto_clean_enabled && mem_info.percent_physical >= auto_limit && last_clean.elapsed() >= cooldown {
                triggered = true;
                reason = "Threshold auto-clean";
            } else if auto_interval_enabled && last_interval_clean.elapsed() >= Duration::from_secs(auto_interval_mins * 60) {
                triggered = true;
                reason = "Interval auto-clean";
            }

            if triggered {
                match clean_memory(current_mask) {
                    Ok(report) => {
                        let freed = format_bytes(report.bytes_freed);
                        status_msg = Some((
                            format!("{} [Auto]: Reclaimed {} in {:.1?}", reason, freed, report.duration),
                            COLOR_GREEN,
                        ));
                        last_report = Some(report);
                        last_clean = Instant::now();
                        last_interval_clean = Instant::now();
                    }
                    Err(e) => {
                        status_msg = Some((format!("Auto-clean failed: {}", e), COLOR_RED));
                    }
                }
            }
        }

        // Render UI
        render_dashboard(
            &mem_info,
            current_mask,
            auto_clean_enabled,
            auto_limit,
            auto_interval_enabled,
            auto_interval_mins,
            last_report.as_ref(),
            status_msg.as_ref(),
        );

        // Read keypress (non-blocking with timeout)
        let key = read_key();
        match key {
            Some('q') | Some('Q') | Some('\x1B') => {
                break;
            }
            Some(' ') | Some('c') | Some('C') => {
                // Trigger Clean
                match clean_memory(current_mask) {
                    Ok(report) => {
                        let freed = format_bytes(report.bytes_freed);
                        status_msg = Some((
                            format!("Success: Reclaimed {} in {:.1?}", freed, report.duration),
                            COLOR_GREEN,
                        ));
                        last_report = Some(report);
                        last_clean = Instant::now();
                        last_interval_clean = Instant::now();
                    }
                    Err(e) => {
                        status_msg = Some((format!("Clean error: {}", e), COLOR_RED));
                    }
                }
            }
            Some('f') | Some('F') => {
                // Full Clean
                match clean_memory(REDUCT_MASK_ALL) {
                    Ok(report) => {
                        let freed = format_bytes(report.bytes_freed);
                        status_msg = Some((
                            format!("Full Clean: Reclaimed {} in {:.1?}", freed, report.duration),
                            COLOR_GREEN,
                        ));
                        last_report = Some(report);
                        last_clean = Instant::now();
                        last_interval_clean = Instant::now();
                    }
                    Err(e) => {
                        status_msg = Some((format!("Clean error: {}", e), COLOR_RED));
                    }
                }
            }
            Some('1') => current_mask ^= REDUCT_WORKINGSET,
            Some('2') => current_mask ^= REDUCT_SYSTEMFILECACHE,
            Some('3') => current_mask ^= REDUCT_STANDBYPRIORITY0LIST,
            Some('4') => current_mask ^= REDUCT_STANDBYLIST,
            Some('5') => current_mask ^= REDUCT_MODIFIEDLIST,
            Some('6') => current_mask ^= REDUCT_COMBINEMEMORYLISTS,
            Some('7') => current_mask ^= REDUCT_REGISTRYCACHE,
            Some('8') => current_mask ^= REDUCT_MODIFIEDFILECACHE,
            Some('a') | Some('A') => {
                auto_clean_enabled = !auto_clean_enabled;
                status_msg = Some((
                    format!(
                        "Auto-clean threshold {}",
                        if auto_clean_enabled { "ENABLED" } else { "DISABLED" }
                    ),
                    COLOR_YELLOW,
                ));
            }
            Some('t') | Some('T') => {
                auto_limit = match auto_limit {
                    70 => 80,
                    80 => 85,
                    85 => 90,
                    90 => 95,
                    _ => 70,
                };
                status_msg = Some((format!("Threshold set to {}%", auto_limit), COLOR_CYAN));
            }
            Some('i') | Some('I') => {
                if !auto_interval_enabled {
                    auto_interval_enabled = true;
                } else if auto_interval_mins == 30 {
                    auto_interval_mins = 60;
                } else if auto_interval_mins == 60 {
                    auto_interval_mins = 15;
                } else {
                    auto_interval_enabled = false;
                    auto_interval_mins = 30;
                }
                status_msg = Some((
                    format!(
                        "Auto-clean interval: {}",
                        if auto_interval_enabled {
                            format!("ON ({} min)", auto_interval_mins)
                        } else {
                            "OFF".to_string()
                        }
                    ),
                    COLOR_YELLOW,
                ));
            }
            _ => {}
        }

        thread::sleep(Duration::from_millis(150));
    }

    // Show cursor and clear screen
    print!("\x1B[?25h\x1B[2J\x1B[H");
    io::stdout().flush().ok();
    println!("Mem Reduct closed.");
}

fn read_key() -> Option<char> {
    let mut buf = [0u8; 1];
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    match handle.read(&mut buf) {
        Ok(1) => Some(buf[0] as char),
        _ => None,
    }
}

fn render_dashboard(
    info: &MemoryInfo,
    mask: u32,
    auto_enabled: bool,
    auto_limit: u8,
    interval_enabled: bool,
    interval_mins: u64,
    last_report: Option<&CleanReport>,
    status_msg: Option<&(String, &'static str)>,
) {
    let mut out = String::with_capacity(4096);
    out.push_str("\x1B[H"); // Cursor to top-left

    // Title bar
    out.push_str(&format!(
        "{}{}╭────────────────────────────────────────────────────────────╮{}\n",
        COLOR_BOLD, COLOR_CYAN, COLOR_RESET
    ));
    out.push_str(&format!(
        "{}{}│                Mem Reduct v3.5.3 (Rust)                    │{}\n",
        COLOR_BOLD, COLOR_CYAN, COLOR_RESET
    ));
    out.push_str(&format!(
        "{}{}╰────────────────────────────────────────────────────────────╯{}\n",
        COLOR_BOLD, COLOR_CYAN, COLOR_RESET
    ));

    // Admin privileges notice
    if !cleaner::is_elevated() {
        out.push_str(&format!(
            "  {}⚠ Run with sudo / Administrator privileges to clean memory!{}\n",
            COLOR_YELLOW, COLOR_RESET
        ));
    } else {
        out.push_str(&format!(
            "  {}✓ Running with elevated privileges{}\n",
            COLOR_GREEN, COLOR_RESET
        ));
    }
    out.push('\n');

    // Physical Memory
    let p_color = get_level_color(info.percent_physical);
    out.push_str(&format!(
        "  {}Physical Memory:{}  {} {}{:>3}%{}\n",
        COLOR_BOLD,
        COLOR_RESET,
        render_bar(info.percent_physical, 28),
        p_color,
        info.percent_physical,
        COLOR_RESET
    ));
    out.push_str(&format!(
        "    Used: {}{:<10}{} | Available: {:<10} | Total: {}\n\n",
        p_color,
        format_bytes(info.used_physical),
        COLOR_RESET,
        format_bytes(info.avail_physical),
        format_bytes(info.total_physical)
    ));

    // Pagefile / Swap
    if info.total_pagefile > 0 {
        let pf_color = get_level_color(info.percent_pagefile);
        out.push_str(&format!(
            "  {}Pagefile / Swap:{}  {} {}{:>3}%{}\n",
            COLOR_BOLD,
            COLOR_RESET,
            render_bar(info.percent_pagefile, 28),
            pf_color,
            info.percent_pagefile,
            COLOR_RESET
        ));
        out.push_str(&format!(
            "    Used: {}{:<10}{} | Available: {:<10} | Total: {}\n\n",
            pf_color,
            format_bytes(info.used_pagefile),
            COLOR_RESET,
            format_bytes(info.avail_pagefile),
            format_bytes(info.total_pagefile)
        ));
    }

    // Cache line
    let cached_str = info.cached.map(format_bytes).unwrap_or_else(|| "N/A".into());
    let buffers_str = info.buffers.map(format_bytes).unwrap_or_else(|| "N/A".into());
    out.push_str(&format!(
        "  {}File Cache:{} {} | {}Buffers:{} {}\n",
        COLOR_BOLD, COLOR_RESET, cached_str, COLOR_BOLD, COLOR_RESET, buffers_str
    ));
    out.push_str(&format!("  {}\n", "─".repeat(60)));

    // Memory Regions Checklist
    out.push_str(&format!("  {}Memory Regions to Clean (press 1-8 to toggle):{}\n", COLOR_BOLD, COLOR_RESET));

    let check = |bit: u32| {
        if (mask & bit) != 0 {
            format!("{}[✓]{}", COLOR_GREEN, COLOR_RESET)
        } else {
            format!("{}[ ]{}", COLOR_GRAY, COLOR_RESET)
        }
    };

    out.push_str(&format!("    {} 1. Working Set          {} 5. Modified Page List\n", check(REDUCT_WORKINGSET), check(REDUCT_MODIFIEDLIST)));
    out.push_str(&format!("    {} 2. System File Cache    {} 6. Combine Memory Lists\n", check(REDUCT_SYSTEMFILECACHE), check(REDUCT_COMBINEMEMORYLISTS)));
    out.push_str(&format!("    {} 3. Standby (Prio 0)     {} 7. Registry Cache\n", check(REDUCT_STANDBYPRIORITY0LIST), check(REDUCT_REGISTRYCACHE)));
    out.push_str(&format!("    {} 4. Standby List (All)   {} 8. Flush Volume Cache\n", check(REDUCT_STANDBYLIST), check(REDUCT_MODIFIEDFILECACHE)));
    out.push('\n');

    // Auto-clean settings
    let auto_status = if auto_enabled {
        format!("{}[ON: >= {}%]{}", COLOR_GREEN, auto_limit, COLOR_RESET)
    } else {
        format!("{}[OFF]{}", COLOR_GRAY, COLOR_RESET)
    };
    let int_status = if interval_enabled {
        format!("{}[ON: {}m]{}", COLOR_GREEN, interval_mins, COLOR_RESET)
    } else {
        format!("{}[OFF]{}", COLOR_GRAY, COLOR_RESET)
    };
    out.push_str(&format!("  {}Auto-Clean:{} {} (toggle: [A], limit: [T]) | {}Interval:{} {} (toggle: [I])\n", COLOR_BOLD, COLOR_RESET, auto_status, COLOR_BOLD, COLOR_RESET, int_status));
    out.push_str(&format!("  {}\n", "─".repeat(60)));

    // Status message or last clean report
    if let Some((msg, col)) = status_msg {
        out.push_str(&format!("  {}{}{}\n\n", col, msg, COLOR_RESET));
    } else if let Some(rep) = last_report {
        out.push_str(&format!(
            "  {}Last Clean:{} Freed {}{} ({}% -> {}%) in {:.1?}\n\n",
            COLOR_GREEN,
            COLOR_RESET,
            format_bytes(rep.bytes_freed),
            COLOR_RESET,
            rep.mem_before.percent_physical,
            rep.mem_after.percent_physical,
            rep.duration
        ));
    } else {
        out.push_str("  Ready to clean memory.\n\n");
    }

    // Action buttons / shortcuts footer
    out.push_str(&format!(
        "  {}[SPACE / C] Clean Memory   [F] Full Clean   [Q] Quit{}\n",
        COLOR_BOLD, COLOR_RESET
    ));

    print!("{}", out);
    io::stdout().flush().ok();
}
