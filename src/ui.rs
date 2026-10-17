use crate::cleaner::CleanReport;
use crate::system::{format_bytes, MemoryInfo};

pub const COLOR_RESET: &str = "\x1B[0m";
pub const COLOR_BOLD: &str = "\x1B[1m";
pub const COLOR_GREEN: &str = "\x1B[32m";
pub const COLOR_YELLOW: &str = "\x1B[33m";
pub const COLOR_RED: &str = "\x1B[31m";
pub const COLOR_CYAN: &str = "\x1B[36m";
pub const COLOR_GRAY: &str = "\x1B[90m";

pub fn get_level_color(percent: u8) -> &'static str {
    if percent >= 90 {
        COLOR_RED
    } else if percent >= 70 {
        COLOR_YELLOW
    } else {
        COLOR_GREEN
    }
}

pub fn render_bar(percent: u8, width: usize) -> String {
    let filled = ((percent as usize * width) + 50) / 100;
    let filled = filled.min(width);
    let empty = width - filled;

    let color = get_level_color(percent);
    let mut bar = String::new();
    bar.push_str(color);
    bar.push('[');
    for _ in 0..filled {
        bar.push('█');
    }
    bar.push_str(COLOR_GRAY);
    for _ in 0..empty {
        bar.push('░');
    }
    bar.push_str(color);
    bar.push(']');
    bar.push_str(COLOR_RESET);
    bar
}

pub fn print_status(info: &MemoryInfo) {
    println!("{}{}=== Mem Reduct - Memory Status ==={}", COLOR_BOLD, COLOR_CYAN, COLOR_RESET);
    println!();

    // Physical Memory
    let p_color = get_level_color(info.percent_physical);
    println!(
        "{}Physical Memory:{} {} {:>3}%",
        COLOR_BOLD,
        COLOR_RESET,
        render_bar(info.percent_physical, 25),
        info.percent_physical
    );
    println!(
        "  Used:      {}{:<10}{} ({})",
        p_color,
        format_bytes(info.used_physical),
        COLOR_RESET,
        format_bytes(info.total_physical)
    );
    println!("  Available: {:<10}", format_bytes(info.avail_physical));
    println!();

    // Pagefile / Swap
    if info.total_pagefile > 0 {
        let pf_color = get_level_color(info.percent_pagefile);
        println!(
            "{}Pagefile / Swap:{} {} {:>3}%",
            COLOR_BOLD,
            COLOR_RESET,
            render_bar(info.percent_pagefile, 25),
            info.percent_pagefile
        );
        println!(
            "  Used:      {}{:<10}{} ({})",
            pf_color,
            format_bytes(info.used_pagefile),
            COLOR_RESET,
            format_bytes(info.total_pagefile)
        );
        println!("  Available: {:<10}", format_bytes(info.avail_pagefile));
        println!();
    }

    // Cache / Buffers
    if let Some(cached) = info.cached {
        println!("{}File Cache:{}      {}", COLOR_BOLD, COLOR_RESET, format_bytes(cached));
    }
    if let Some(buffers) = info.buffers {
        println!("{}Buffers:{}         {}", COLOR_BOLD, COLOR_RESET, format_bytes(buffers));
    }
    if let Some(standby) = info.standby {
        println!("{}Standby List:{}    {}", COLOR_BOLD, COLOR_RESET, format_bytes(standby));
    }
    if let Some(modified) = info.modified {
        println!("{}Modified List:{}   {}", COLOR_BOLD, COLOR_RESET, format_bytes(modified));
    }
    println!();
}

pub fn print_clean_report(report: &CleanReport) {
    println!();
    println!("{}{}=== Memory Reduction Completed ==={}", COLOR_BOLD, COLOR_GREEN, COLOR_RESET);
    println!();

    if !report.regions_succeeded.is_empty() {
        println!("{}Cleaned regions:{}{}", COLOR_BOLD, COLOR_RESET, COLOR_GREEN);
        for reg in &report.regions_succeeded {
            println!("  [✓] {}", reg);
        }
        print!("{}", COLOR_RESET);
    }

    if !report.regions_failed.is_empty() {
        println!();
        println!("{}Failed regions:{}{}", COLOR_BOLD, COLOR_RESET, COLOR_RED);
        for (reg, err) in &report.regions_failed {
            println!("  [✗] {}: {}", reg, err);
        }
        print!("{}", COLOR_RESET);
    }

    println!();
    let before_p = report.mem_before.percent_physical;
    let after_p = report.mem_after.percent_physical;
    let b_color = get_level_color(before_p);
    let a_color = get_level_color(after_p);

    println!(
        "Memory Before: {}{:>3}%{} ({})",
        b_color,
        before_p,
        COLOR_RESET,
        format_bytes(report.mem_before.used_physical)
    );
    println!(
        "Memory After:  {}{:>3}%{} ({})",
        a_color,
        after_p,
        COLOR_RESET,
        format_bytes(report.mem_after.used_physical)
    );
    println!();

    if report.bytes_freed > 0 {
        println!(
            "{}{}Reclaimed: {}{}{} in {:.2?}",
            COLOR_BOLD,
            COLOR_GREEN,
            COLOR_BOLD,
            format_bytes(report.bytes_freed),
            COLOR_RESET,
            report.duration
        );
    } else {
        println!(
            "{}Reclaimed: 0 B{} (Memory was already clean)",
            COLOR_YELLOW,
            COLOR_RESET
        );
    }
    println!();
}
