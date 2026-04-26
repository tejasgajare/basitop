# basitop

[![CI](https://github.com/tejasgajare/basitop/actions/workflows/ci.yml/badge.svg)](https://github.com/tejasgajare/basitop/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Apple Silicon](https://img.shields.io/badge/Apple%20Silicon-M1%E2%80%93M5-black?logo=apple)](#requirements)

A btop-style, real-time performance monitor for Apple Silicon Macs — built in Rust.

Combines **macmon** (no-root IOKit metrics) with **powermetrics** (root-level thermal, SoC power, and I/O) to give you the most complete picture of your M-series chip possible in a terminal.

---

## Preview

![basitop demo](demo.gif)

---

## Features

| Metric | Source | Requires sudo |
|--------|--------|:---:|
| CPU usage (per-cluster + per-core sparklines) | macmon | No |
| CPU / E-cluster / P-cluster / S-cluster frequency | macmon | No |
| GPU utilization & frequency | macmon | No |
| CPU, GPU, ANE power | macmon | No |
| SoC / combined power | powermetrics | Yes |
| RAM & swap usage | macmon | No |
| Disk read/write throughput | powermetrics | Yes |
| Network rx/tx throughput | powermetrics | Yes |
| CPU & GPU temperature | macmon | No |
| Thermal pressure (Nominal / Fair / Serious / Critical) | powermetrics | Yes |

**macmon-only mode** (no sudo): CPU, GPU, memory, power, and temperature still work.  
**Full mode** (sudo): adds thermal pressure, disk/network I/O, and more precise SoC power.

### Visual highlights

- **Braille-resolution charts** — 2× horizontal and 4× vertical sub-cell resolution using Unicode Braille patterns, matching btop's density.
- **Mirror I/O charts** — download above / upload below a shared x-axis, so both directions are instantly comparable.
- **Per-core sparklines** — every P-core and S-core gets its own one-line history, laid out in two columns.
- **HSL gradient palettes** — each panel has a distinct color theme; utilization gauges shift from cool to warm as load rises.
- **Ring-buffer history** — configurable sample count (default 1024) keeps charts smooth across intervals.
- **Powermetrics state indicator** — footer shows `pm: WAIT → ON → OFF` so you always know whether root-level data is flowing.

---

## Requirements

- **macOS 13+** (Ventura or later recommended)
- **Apple Silicon Mac** — M1, M2, M3, M4, or M5 series
- **Rust 1.75+** — install via [rustup](https://rustup.rs)
- `powermetrics` — ships with macOS, found at `/usr/bin/powermetrics`

---

## Installation

### Homebrew (recommended)

```bash
brew install tejasgajare/tap/basitop
```

### From source

```bash
git clone https://github.com/tejasgajare/basitop.git
cd basitop
cargo build --release
```

The binary is at `target/release/basitop`. Copy it wherever you like:

```bash
sudo cp target/release/basitop /usr/local/bin/
```

### Pre-built binary (manual)

Each tagged release publishes a pre-built Apple Silicon binary:

```bash
curl -L https://github.com/tejasgajare/basitop/releases/latest/download/basitop-aarch64-apple-darwin.tar.gz \
  | tar -xz
sudo mv basitop-aarch64-apple-darwin/basitop /usr/local/bin/
```

---

## Usage

### macmon-only (no sudo required)

```bash
basitop
```

CPU, GPU, memory, power, and temperature metrics are available immediately without elevated privileges.

### Full metrics (sudo required)

```bash
sudo basitop
```

Running as root activates the `powermetrics` companion process, which adds:

- Thermal pressure level
- Disk read/write throughput
- Network rx/tx throughput
- More precise SoC / combined power

The `pm:` indicator in the footer shows the powermetrics state:

| Indicator | Meaning |
|-----------|---------|
| `pm: WAIT` | powermetrics launched, first sample not yet parsed |
| `pm: ON` | streaming live data |
| `pm: OFF` | powermetrics not running (no sudo, or it crashed) |

> **Tip:** if you want full metrics without typing `sudo` each time, you can configure a sudoers rule:
> ```
> username ALL=(root) NOPASSWD: /usr/bin/powermetrics
> ```

---

## Keybindings

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `Tab` / `Shift+Tab` | Cycle through panels |
| `←` `↑` `→` `↓` | Navigate panels spatially |
| `+` / `=` | Increase update interval (+500 ms) |
| `-` | Decrease update interval (−500 ms) |
| `h` | Toggle help overlay |

---

## CLI options

```
basitop [OPTIONS]

Options:
  -i, --interval <ms>   Update interval in milliseconds [default: 1000]
      --history <N>     History ring-buffer size in samples  [default: 1024]
  -h, --help            Print help
```

**Interval** controls how often the TUI redraws. The macmon sampler runs at roughly half the interval (clamped to ≥ 250 ms), and powermetrics runs at the same half-interval.

**History** controls how many samples each chart holds. The Braille canvas renders 2 sub-columns per character cell, so a chart 60 columns wide displays 120 data points. Setting `--history` below that will show blank space at the left edge until the buffer fills; setting it much higher just costs a little memory.

---

## Troubleshooting

If `pm:` stays `OFF` after running with `sudo`, check the diagnostic log:

```bash
cat /tmp/basitop_pm.log
```

The log records the exact command used, the child PID, first-sample timestamp, and any powermetrics stderr. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to run the live integration test.

---

## Acknowledgements

- [macmon](https://github.com/vladkens/macmon) — Rust crate that reads Apple Silicon power/frequency/temp via IOKit without root.
- [btop](https://github.com/aristocratos/btop) — visual inspiration for layout, color style, and the mirror I/O chart.
- [asitop](https://github.com/tlkh/asitop) — reference implementation for powermetrics plist parsing on Apple Silicon.
- [htop](https://github.com/htop-dev/htop) / [btop](https://github.com/aristocratos/btop) — the TUI monitor genre that inspired the layout and interaction model.
- [ratatui](https://github.com/ratatui-org/ratatui) — terminal UI framework.

---

## License

MIT — see [LICENSE](LICENSE).
