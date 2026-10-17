#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::run_gui;

#[cfg(not(target_os = "windows"))]
pub fn run_gui() -> Result<(), String> {
    Err("Native GUI is only supported on Windows. On Linux/Unix, use the interactive TUI interface (--tui).".to_string())
}
