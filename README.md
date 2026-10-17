<h1 align="center">Mem Reduct (Rust)</h1>

<p align="center">
	<a href="https://github.com/henrypp/memreduct/blob/master/LICENSE"><img src="https://img.shields.io/github/license/henrypp/memreduct?style=flat-square" /></a>
	<img src="https://img.shields.io/badge/language-Rust-orange.svg?style=flat-square" />
	<img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux-blue.svg?style=flat-square" />
</p>

-------

### Description:
Lightweight, high-performance real-time memory management application written in **Rust** to monitor and clean system memory.

On Windows, the program uses undocumented internal system features (Native NT API) to clear system cache, working sets, standby page lists, and modified page lists (~10-50% memory reduction). On Linux, it interfaces with kernel cache dropping (`drop_caches`), compaction (`compact_memory`), and page cache flush (`sync`).

Administrator / root privileges are required for memory reduction operations.

### Features:
- **Fast & Safe**: Zero external dependencies, pure Rust implementation.
- **Cross-platform**: Full support for both Windows and Linux.
- **Multiple Memory Regions**:
  - Process Working Sets (`REDUCT_WORKINGSET`)
  - System File Cache (`REDUCT_SYSTEMFILECACHE`)
  - Standby List Priority-0 (`REDUCT_STANDBYPRIORITY0LIST`)
  - Standby List Full (`REDUCT_STANDBYLIST`)
  - Modified Page List (`REDUCT_MODIFIEDLIST`)
  - Registry Cache (`REDUCT_REGISTRYCACHE`)
  - Combine Memory Lists (`REDUCT_COMBINEMEMORYLISTS`)
  - Volume Cache Flush (`REDUCT_MODIFIEDFILECACHE`)
- **CLI & Automation**:
  - Fast one-shot cleaning (`-clean`, `--full`, `--regions ...`)
  - Live terminal monitoring dashboard (`--monitor`)
  - Background daemon with auto-clean threshold (`--auto --limit 85 --interval 1800`)
  - Memory statistics reporting (`--status`)
  - Configuration support via `memreduct.toml` or `memreduct.ini`

### Build:
Ensure you have the Rust toolchain installed:
```bash
cargo build --release
```
The compiled binary will be placed at `target/release/memreduct` (or `memreduct.exe` on Windows).

### Usage:
```
Usage: memreduct [OPTIONS]

Options:
  -clean                 Clean default memory regions
  -clean:full, --full    Clean all memory regions (including standby & modified)
  -s, --status           Show current memory statistics and exit
  -m, --monitor          Run live memory monitor in terminal
  -a, --auto             Run auto-clean daemon
      --limit <pct>      Set memory usage threshold for auto-clean (e.g. 90)
      --interval <sec>   Set periodic auto-clean interval in seconds
      --regions <list>   Comma-separated regions to clean:
                         workingset, cache, standbyprio0, standby,
                         modified, combine, registry, volumecache
  -h, --help             Show this help message
  -v, --version          Show version information
```

### Examples:
```bash
# Clean default regions (Run with sudo / Administrator)
sudo memreduct -clean

# Clean all memory regions
sudo memreduct --full

# Live terminal monitor
memreduct --monitor

# Run auto-clean daemon when memory usage exceeds 85% or every 30 minutes
sudo memreduct --auto --limit 85 --interval 1800
```

---
- Website: [github.com/henrypp](https://github.com/henrypp)
- Original author: Henry++
- Ported to Rust (c) 2011-2026
