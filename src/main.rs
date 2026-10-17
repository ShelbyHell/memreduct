mod cleaner;
mod config;
mod gui;
mod system;
mod tui;
mod ui;

use cleaner::{
    clean_memory, get_region_names, parse_region_name, REDUCT_MASK_ALL, REDUCT_MASK_DEFAULT,
};
use config::Config;
use std::env;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const APP_NAME: &str = "Mem Reduct";
const APP_VERSION: &str = "3.5.3";

fn print_help() {
    println!("{}{} v{} - Memory Management Utility{}", ui::COLOR_BOLD, APP_NAME, APP_VERSION, ui::COLOR_RESET);
    println!();
    println!("Usage: memreduct [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --ui, --tui            Launch interactive terminal user interface (TUI)");
    println!("  -g, --gui              Launch graphical user interface (Windows desktop window)");
    println!("  -clean                 Clean default memory regions");
    println!("  -clean:full, --full    Clean all memory regions (including standby & modified)");
    println!("  -s, --status           Show current memory statistics and exit");
    println!("  -m, --monitor          Run live memory monitor in terminal");
    println!("  -a, --auto             Run auto-clean daemon");
    println!("      --limit <pct>      Set memory usage threshold for auto-clean (e.g. 90)");
    println!("      --interval <sec>   Set periodic auto-clean interval in seconds");
    println!("      --regions <list>   Comma-separated regions to clean:");
    println!("                         workingset, cache, standbyprio0, standby,");
    println!("                         modified, combine, registry, volumecache");
    println!("  -h, --help             Show this help message");
    println!("  -v, --version          Show version information");
    println!();
    println!("Examples:");
    println!("  memreduct              # Opens interactive UI (GUI on Windows, TUI on Linux)");
    println!("  memreduct --ui         # Opens interactive TUI dashboard");
    println!("  memreduct -clean       # Quick clean without UI");
    println!("  memreduct --full       # Full clean without UI");
    println!("  memreduct --auto --limit 85 --interval 1800");
}

fn print_version() {
    println!("{} v{} (Rust)", APP_NAME, APP_VERSION);
    println!("Copyright (c) 2011-2026 Henry++, Rust port");
}

enum RunMode {
    Gui,
    Tui,
    Clean(u32),
    Status,
    Monitor,
    AutoDaemon { limit: u8, interval_secs: Option<u64>, mask: u32 },
    Help,
    Version,
}

fn parse_cli_args() -> Result<RunMode, String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        #[cfg(target_os = "windows")]
        return Ok(RunMode::Gui);

        #[cfg(not(target_os = "windows"))]
        return Ok(RunMode::Tui);
    }

    let mut clean_mask: Option<u32> = None;
    let mut is_auto = false;
    let mut is_monitor = false;
    let mut is_status = false;
    let mut custom_limit: Option<u8> = None;
    let mut custom_interval: Option<u64> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "-g" | "--gui" => {
                return Ok(RunMode::Gui);
            }
            "-u" | "--ui" | "--tui" => {
                return Ok(RunMode::Tui);
            }
            "-clean" | "--clean" => {
                clean_mask = Some(clean_mask.unwrap_or(0) | REDUCT_MASK_DEFAULT);
            }
            "-clean:full" | "--full" => {
                clean_mask = Some(REDUCT_MASK_ALL);
            }
            "-s" | "--status" => {
                is_status = true;
            }
            "-m" | "--monitor" => {
                is_monitor = true;
            }
            "-a" | "--auto" | "--daemon" => {
                is_auto = true;
            }
            "--limit" | "-l" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --limit".to_string());
                }
                let val: u8 = args[i].parse().map_err(|_| "Invalid --limit percentage".to_string())?;
                custom_limit = Some(val.min(100));
            }
            "--interval" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --interval".to_string());
                }
                let val: u64 = args[i].parse().map_err(|_| "Invalid --interval value".to_string())?;
                custom_interval = Some(val);
            }
            "--regions" => {
                i += 1;
                if i >= args.len() {
                    return Err("Missing value for --regions".to_string());
                }
                let mut mask = 0u32;
                for part in args[i].split(',') {
                    let part = part.trim();
                    if let Some(r) = parse_region_name(part) {
                        mask |= r;
                    } else {
                        return Err(format!("Unknown region: '{}'", part));
                    }
                }
                clean_mask = Some(mask);
            }
            "-h" | "--help" | "/?" => {
                return Ok(RunMode::Help);
            }
            "-v" | "--version" => {
                return Ok(RunMode::Version);
            }
            other => {
                if other.starts_with("-clean:") {
                    let sub = &other[7..];
                    if sub.eq_ignore_ascii_case("full") {
                        clean_mask = Some(REDUCT_MASK_ALL);
                    } else {
                        return Err(format!("Unknown clean modifier: '{}'", sub));
                    }
                } else {
                    return Err(format!("Unknown option: '{}'. Use --help for help.", other));
                }
            }
        }
        i += 1;
    }

    if is_auto {
        let config = Config::load();
        let limit = custom_limit.unwrap_or(config.autoreduct_limit);
        let interval_secs = custom_interval.or_else(|| {
            if config.autoreduct_interval_enable {
                Some(config.autoreduct_interval_minutes * 60)
            } else {
                None
            }
        });
        let mask = clean_mask.unwrap_or(config.reduct_mask);
        return Ok(RunMode::AutoDaemon {
            limit,
            interval_secs,
            mask,
        });
    }

    if is_monitor {
        return Ok(RunMode::Monitor);
    }

    if let Some(mask) = clean_mask {
        return Ok(RunMode::Clean(mask));
    }

    if is_status {
        return Ok(RunMode::Status);
    }

    Ok(RunMode::Status)
}

fn run_clean(mask: u32) {
    if !cleaner::is_elevated() {
        eprintln!(
            "{}Warning: Administrator / root privileges are recommended to clean memory.{}",
            ui::COLOR_YELLOW,
            ui::COLOR_RESET
        );
    }

    let regions = get_region_names(mask);
    println!(
        "{}Cleaning regions:{} {}",
        ui::COLOR_BOLD,
        ui::COLOR_RESET,
        regions.join(", ")
    );

    match clean_memory(mask) {
        Ok(report) => {
            ui::print_clean_report(&report);
        }
        Err(e) => {
            eprintln!("{}Error: {}{}", ui::COLOR_RED, e, ui::COLOR_RESET);
            std::process::exit(1);
        }
    }
}

fn run_monitor(running: Arc<AtomicBool>) {
    println!("\x1B[2J\x1B[H"); // Clear screen
    while running.load(Ordering::SeqCst) {
        match system::get_memory_info() {
            Ok(info) => {
                print!("\x1B[H"); // Move cursor to top-left
                println!(
                    "{}{}=== Mem Reduct Monitor (Ctrl+C to exit) ==={}",
                    ui::COLOR_BOLD,
                    ui::COLOR_CYAN,
                    ui::COLOR_RESET
                );
                let now = chrono_like_now();
                println!("{}Current Time:{} {}\n", ui::COLOR_GRAY, ui::COLOR_RESET, now);

                ui::print_status(&info);

                io::stdout().flush().ok();
            }
            Err(e) => {
                eprintln!("Error fetching memory info: {}", e);
            }
        }
        thread::sleep(Duration::from_millis(1000));
    }
}

fn run_auto_daemon(limit: u8, interval_secs: Option<u64>, mask: u32, running: Arc<AtomicBool>) {
    let config = Config::load();
    let cooldown = Duration::from_secs(config.cooldown_seconds);

    println!(
        "{}{}Mem Reduct Auto-Clean Daemon started{}",
        ui::COLOR_BOLD,
        ui::COLOR_GREEN,
        ui::COLOR_RESET
    );
    println!("Threshold limit:    {}%", limit);
    if let Some(sec) = interval_secs {
        println!("Interval cleanup:   every {} seconds", sec);
    }
    println!("Cooldown:           {} seconds", config.cooldown_seconds);
    println!("Regions mask:       0x{:02X}", mask);
    println!("Press Ctrl+C to stop.\n");

    let mut last_clean = Instant::now() - cooldown;
    let mut last_interval_clean = Instant::now();

    while running.load(Ordering::SeqCst) {
        if let Ok(info) = system::get_memory_info() {
            let mut should_clean = false;
            let mut reason = "";

            if info.percent_physical >= limit && last_clean.elapsed() >= cooldown {
                should_clean = true;
                reason = "Memory threshold reached";
            } else if let Some(sec) = interval_secs {
                if last_interval_clean.elapsed() >= Duration::from_secs(sec) {
                    should_clean = true;
                    reason = "Periodic interval timer";
                }
            }

            if should_clean {
                println!(
                    "[{}] Triggering cleanup ({}) - Current usage: {}%",
                    chrono_like_now(),
                    reason,
                    info.percent_physical
                );
                match clean_memory(mask) {
                    Ok(report) => {
                        println!(
                            "[{}] Cleaned: {} reclaimed ({}% -> {}%) in {:.2?}",
                            chrono_like_now(),
                            system::format_bytes(report.bytes_freed),
                            report.mem_before.percent_physical,
                            report.mem_after.percent_physical,
                            report.duration
                        );
                        last_clean = Instant::now();
                        last_interval_clean = Instant::now();
                    }
                    Err(e) => {
                        eprintln!("[{}] Cleanup failed: {}", chrono_like_now(), e);
                    }
                }
            }
        }

        thread::sleep(Duration::from_secs(1));
    }

    println!("\nDaemon stopped.");
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now();
    let duration = now.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = (rem / 3600) % 24;
    let minutes = (rem / 60) % 60;
    let seconds = rem % 60;

    // Estimate UTC timestamp representation
    let mut y = 1970;
    let mut d = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        let days_in_year = if leap { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0;
    while m < 12 && d >= month_days[m] {
        d -= month_days[m];
        m += 1;
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        y,
        m + 1,
        d + 1,
        hours,
        minutes,
        seconds
    )
}

fn main() {
    let mode = match parse_cli_args() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}Error: {}{}", ui::COLOR_RED, e, ui::COLOR_RESET);
            std::process::exit(1);
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    // Handle Ctrl+C gracefully
    let _ = ctrlc_setup(r);

    match mode {
        RunMode::Gui => {
            if let Err(e) = gui::run_gui() {
                eprintln!("{}GUI error: {}{}", ui::COLOR_RED, e, ui::COLOR_RESET);
                println!("Falling back to interactive TUI...");
                tui::run_interactive_tui(running);
            }
        }
        RunMode::Tui => {
            tui::run_interactive_tui(running);
        }
        RunMode::Help => {
            print_help();
        }
        RunMode::Version => {
            print_version();
        }
        RunMode::Status => match system::get_memory_info() {
            Ok(info) => {
                ui::print_status(&info);
            }
            Err(e) => {
                eprintln!("{}Error getting memory info: {}{}", ui::COLOR_RED, e, ui::COLOR_RESET);
                std::process::exit(1);
            }
        },
        RunMode::Clean(mask) => {
            run_clean(mask);
        }
        RunMode::Monitor => {
            run_monitor(running);
        }
        RunMode::AutoDaemon {
            limit,
            interval_secs,
            mask,
        } => {
            run_auto_daemon(limit, interval_secs, mask, running);
        }
    }
}

#[cfg(unix)]
fn ctrlc_setup(running: Arc<AtomicBool>) -> Result<(), ()> {
    static RUNNING_PTR: std::sync::atomic::AtomicPtr<AtomicBool> =
        std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

    let raw = Arc::into_raw(running);
    RUNNING_PTR.store(raw as *mut _, Ordering::SeqCst);

    extern "C" fn handle_sigint(_: i32) {
        let ptr = RUNNING_PTR.load(Ordering::SeqCst);
        if !ptr.is_null() {
            unsafe {
                (*ptr).store(false, Ordering::SeqCst);
            }
        }
    }

    extern "C" {
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    }

    unsafe {
        signal(2, handle_sigint);
    }
    Ok(())
}

#[cfg(not(unix))]
fn ctrlc_setup(_running: Arc<AtomicBool>) -> Result<(), ()> {
    Ok(())
}
