# Game Mode - Rust Port w/new improvements

> **Disclaimer**: This project was ported from C# to Rust using Claude Code with Opus 4.5. All functionality has been thoroughly tested and verified to work the same—if not better—than the original C# version. Additional features, tweaks, and optimizations were added by myself based on personal experience and what I actively use on my own system. **Please create a system restore point before using, just in case.** Call it AI slop or whatever—I don't care. I use this myself daily and the results speak for themselves & you can Benchmark it yourself.
>
> Credits to [x1lly](https://x.com/x1lly) for the original program. Peace.

A high-performance Windows optimizer, fully rewritten in Rust from the original C# implementation. This tool maximizes gaming performance by intelligently managing system resources, suspending background processes, and applying low-level Windows optimizations.

## Overview

Game Mode is a complete 1:1 recreation of gamemode.exe by x1lly  in Rust, featuring significant/further improvements in memory efficiency, and overall system overhead. The Rust port maintains 1:1 feature parity with the original while adding new advanced modules and optimizations.

![Xilly Game Mode Screenshot](screenshots.png)

## Performance Benchmarks on Victus 15 Laptop (AMD Ryzen 5 8645HS + RTX 4050 6GB w/ 16GB DDR5)

**Benchmark:** Fortnite Creative Map "Martoz 1v1" on DX12 Performance Mode - Epic Textures, High Meshes, High View Distance, everything else Low settings.

### Hardware Sensor Comparison

Real-world benchmark data comparing baseline (no optimizer), original C# version, and Our Rust fork:

| Metric | Baseline | Original (C#) | Rust Fork | Improvement |
|--------|----------|---------------|-----------|-------------|
| **CPU Load (avg)** | 36% | 27% | 27% | -25% load |
| **CPU Max Thread Load** | 65% | 61% | **55%** | -15% vs original |
| **CPU Max Clock** | 4658 MHz | 4720 MHz | 4696 MHz | Stable boost |
| **CPU Power** | 25W | 22W | 24W | Optimized |
| **CPU Temperature** | 55°C | 46°C | **46°C** | -9°C cooler |
| **GPU Load** | 36% | 37% | 39% | Better utilization |
| **GPU Clock** | 2153 MHz | 2151 MHz | **2256 MHz** | +105 MHz |
| **GPU Power** | 25W | 25W | 27W | Higher performance |
| **GPU Temperature** | 38°C | 37°C | 37°C | Stable |
| **RAM Usage** | 0.96 GB | 0.67 GB | **0.57 GB** | -40% vs baseline |

### Key Findings

- **Thermal Benefits**: 9°C CPU temperature reduction (55°C → 46°C)
- **Memory Efficiency**: 40% lower RAM usage compared to baseline (0.96GB → 0.57GB)
- **Lower Overhead**: 15% reduction in max CPU thread load vs original C# version
- **GPU Performance**: +105 MHz higher GPU clocks with better utilization

> **Verdict**: The Rust fork provides identical thermal benefits to the original while achieving the lowest RAM usage (0.57 GB) and lowest average CPU thread load (55%), making it the most efficient choice for system overhead.

### Frame Time / FPS Analysis

Raw performance averages showing frame stability improvements:

| Metric | Baseline | Original (C#) | Rust Fork | 
|--------|----------|---------------|-----------|
| **Average FPS** | 164.13 | 164.20 | 164.53 |
| **P5 (95% of frames)** | 126.97 | 128.05 | **135.80** |
| **P1 (99% of frames)** | 108.20 | 109.85 | **115.40** |
| **1% Low Average** | 97.47 | 97.35 | **100.97** |
| **P0.2** | 95.07 | 96.55 | **103.60** |
| **P0.1** | 87.60 | 91.25 | 90.40 |
| **0.1% Low Average** | 71.67 | 66.15 | 59.43 |

#### Comparative Performance Improvements

| Comparison | P1 Stability | P5 Smoothness | 0.1% Low Change |
|------------|--------------|---------------|-----------------|
| **Original vs Baseline** | +1.52% | +0.85% | -7.70% |
| **Fork vs Baseline** | **+6.65%** | **+6.96%** | -17.08% |
| **Fork vs Original** | **+5.05%** | **+6.05%** | -10.16% |

> **Verdict**: The Rust fork with all toggles (explorer off) is the definitive leader in general fluidity. By raising the P5 average to 135.80 FPS and the P1 average to 115.40 FPS, it provides the highest frame floor for 99% of gameplay. While baseline maintains a technically superior 0.1% Low Average (71.67 FPS), the Fork's ability to pull the 1% and 5% lows significantly higher creates a far more consistent and responsive experience for the vast majority of frames rendered.

### Frame Time Consistency

Distribution of frame times showing smoothness quality:

| Category | Placebo | Original (C#) | Rust Fork |
|----------|---------|---------------|-----------|
| **< 2ms (Ultra Smooth)** | 78.98% | 81.85% | **90.41%** |
| **< 4ms (Stable)** | 16.81% | 14.91% | **8.08%** |
| **< 8ms (Jittery)** | 4.09% | 3.17% | **1.38%** |
| **< 12ms (Micro-stutter)** | 0.08% | 0.04% | 0.08% |
| **> 12ms (Heavy Stutter)** | 0.04% | 0.03% | 0.05% |

#### Smoothness & Micro-Stutter Improvements

| Comparison | Smoothness Improvement | Micro-Stutter Reduction |
|------------|------------------------|-------------------------|
| **Original vs Placebo** | +3.63% | 13.65% |
| **Fork vs Placebo** | **+14.47%** | **54.38%** |
| **Fork vs Original** | **+10.46%** | **47.16%** |

> **Verdict**: The Rust fork achieves **90.41% ultra-smooth frames** (< 2ms) compared to just 78.98% on placebo—a **14.47% smoothness improvement**. Micro-stutter (jittery frames) is reduced by **54.38%** vs placebo and **47.16%** vs original. This translates to noticeably smoother gameplay with far fewer frame time spikes.

## Features

### Core Game Mode
- **Explorer Suspension**: Safely kills Windows Explorer to free ~200MB+ RAM and reduce DWM overhead
- **Browser Suspension**: Terminates Chrome, Firefox, Edge, Brave, Opera, Vivaldi, Thorium
- **Game Launcher Management**: Closes Epic, Battle.net, Origin, GOG Galaxy when gaming
- **Shell UX Suspension**: Suspends SearchHost, TextInputHost, ShellExperienceHost, etc.
- **Bloatware Termination**: Kills SmartScreen, Cortana, Widgets, OneDrive, GameBar, NVIDIA overlay
- **Peripheral Software**: Closes iCUE, Logitech G Hub, Razer Synapse, Armoury Crate

### Power Optimization
- **High Performance Mode**: Automatically switches to high-performance power plan
- **Laptop Boost Optimization**: Intelligent CPU boost management for laptops
- **Power Setting Unlock**: Exposes hidden Windows power settings

### Advanced Modules (New in Rust Edition)
- **Core Parking Disable**: Prevents micro-stutter from core wake latency
- **MMCSS Priority Boost**: Maximum CPU priority for game threads (SystemResponsiveness=0)
- **Large System Pages**: Better TLB efficiency for reduced memory access latency
- **HAGS (Hardware-Accelerated GPU Scheduling)**: GPU-side scheduling for lower latency
- **Process Idle Demotion**: Demotes background processes to idle priority

### ReviOS-Style Tweaks
When "Advanced Tweaks" is enabled, applies performance optimizations Ported from AME ReviOS Playbook:

**Services Disabled:**
- DiagTrack (Telemetry)
- WerSvc (Windows Error Reporting)
- DPS, WdiServiceHost, WdiSystemHost (Diagnostics)
- PcaSvc (Program Compatibility Assistant)
- WSearch, SysMain (Indexing/Prefetch)
- And 10+ more background services

**Registry Optimizations:**
- VBS/HVCI disabled for gaming performance
- Spectre/Meltdown mitigations disabled (optional performance boost)
- MMCSS Game priority maximized
- Network throttling disabled
- Power throttling disabled
- Telemetry completely disabled

### System Tray Integration
- Minimizes to system tray for zero desktop footprint
- One-click toggle from tray icon
- Prevents exit while game mode is active (safety feature)
- Memory trimming when hidden for minimal idle footprint

### Additional Features
- **MPO (Multi-Plane Overlay) Toggle**: Disable MPO for better frame pacing
- **Fullscreen Game Detection**: Automatically detects and focuses fullscreen games
- **Hardware Specs Export**: One-click system specs to clipboard

## Rust vs C# Comparison

| Aspect | Original C# | Rust Edition |
|--------|-------------|--------------|
| **Runtime** | .NET 8.0 (~150MB) | Native binary |
| **Binary Size** | 144MB | 4MB standalone |
| **UI Framework** | WPF | Slint (Qt backend) |
| **Service Management** | C# P/Invoke | Native Windows API |
| **Parallelization** | Async/Await | std::thread + Mutex |

### Why Rust?

1. **Zero Runtime Overhead**: No .NET runtime, no garbage collection pauses
2. **Memory Safety**: Guaranteed memory safety without GC
3. **Native Performance**: Direct Windows API calls without marshaling overhead
4. **Smaller Footprint**: Single executable, no dependencies
5. **Predictable Performance**: No GC pauses during critical game moments

## Installation

### Requirements
- Windows 10/11 (64-bit)
- Administrator privileges (required for system optimizations)

### Download
Download the latest `gamemode.exe` from the [Releases](https://github.com/SanGraphic/gamemode/releases) page.

### Building from Source

```bash
# Clone the repository
git clone https://github.com/SanGraphic/gamemode.git
cd gamemode

# Build release binary
cargo build --release

# Binary will be at target/release/gamemode.exe
```

**Build Dependencies:**
- Rust 1.70+ (stable)
- Qt 6.x (for Slint UI backend)

## Usage

1. **Run as Administrator** - Right-click `gamemode.exe` → "Run as administrator"
2. **Configure Settings**:
   - Toggle "Suspend Explorer" for maximum RAM savings
   - Enable "Suspend Browsers" if you don't need browser during gaming
   - Enable "Advanced Tweaks" for ReviOS-style optimizations
   - Configure Advanced Modules for hardware-specific tweaks
3. **Activate Game Mode** - Click the power button or toggle from system tray
4. **Launch Your Game** - The tool will detect fullscreen games automatically
5. **Deactivate** - Click toggle again.

## Project Structure

```
GameMode-Rust/
├── src/
│   ├── main.rs                 # Application entry, UI setup, event handling
│   └── services/
│       ├── mod.rs              # Module exports
│       ├── gamemode.rs         # Core game mode logic
│       ├── advanced_modules.rs # Hardware-aware tweaks
│       ├── revi_tweaks.rs      # ReviOS playbook port
│       ├── detector.rs         # Fullscreen game detection
│       ├── power.rs            # Power plan management
│       ├── memory.rs           # Memory optimization
│       ├── network.rs          # Network isolation
│       ├── process.rs          # Process management
│       ├── registry.rs         # Registry operations
│       ├── settings.rs         # Settings persistence
│       ├── update.rs           # Update checker
│       └── windows.rs          # Windows service manager
├── ui/
│   ├── app-window.slint        # Main UI definition
│   └── components/             # Reusable UI components
├── assets/
│   └── app.ico                 # Application icon
├── Cargo.toml                  # Rust dependencies
└── build.rs                    # Build script (icon embedding)
```

## Safety & Reversibility

- **All changes are temporary**: Tweaks are applied only during game mode
- **Original state saved**: All registry values and service states are saved before modification
- **Automatic restore**: Everything is restored when game mode is deactivated
- **Safe exit**: Cannot exit while game mode is active (prevents orphaned state)
- **Explorer restart**: Explorer is automatically restarted on deactivation

## Disclaimer

This tool modifies Windows system settings, power plans, and registry values. While all changes are reversible:

- Use at your own risk
- Test in a non-production environment first
- The tool requires administrator privileges

## License

MIT License - See [LICENSE](LICENSE) for details.

## Credits

- Original concept and C# implementation: https://x.com/x1lly
- ReviOS Playbook inspiration: [ReviOS](https://revi.cc/)
- UI Framework: [Slint](https://slint.dev/)

---

**Built with Rust for maximum performance and minimal overhead.**
