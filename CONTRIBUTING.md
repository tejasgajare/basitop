# Contributing to basitop

Thanks for your interest! This document covers everything you need to get from zero to a merged pull request.

---

## Table of contents

- [Prerequisites](#prerequisites)
- [Getting started](#getting-started)
- [Project layout](#project-layout)
- [Development workflow](#development-workflow)
- [Testing](#testing)
- [Submitting a pull request](#submitting-a-pull-request)
- [Code style](#code-style)
- [Areas that need help](#areas-that-need-help)

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.75+ | `curl https://sh.rustup.rs -sSf \| sh` |
| macOS | 13+ | — |
| Apple Silicon Mac | M1–M5 | — |

Optional but useful:

- `sudo` access — needed to run the `powermetrics` integration tests.
- `cargo-watch` — auto-rebuilds on file change: `cargo install cargo-watch`.

---

## Getting started

```bash
git clone https://github.com/tejasgajare/basitop.git
cd basitop

# Confirm it compiles and all tests pass
cargo test

# Run in dev mode (no root — macmon-only metrics)
cargo run

# Run with full powermetrics support
sudo cargo run
```

---

## Project layout

```
src/
├── main.rs              CLI entry point, terminal setup / teardown
├── app.rs               AppState, event loop, key handling
├── metrics.rs           MetricsCollector: ring buffers, sample aggregation
├── powermetrics.rs      powermetrics child-process management + plist parser
├── theme.rs             Color palettes, HSL gradient, shared style constants
│
├── ui/
│   ├── mod.rs           Top-level draw() — layout split, panel dispatch
│   ├── header.rs        Chip name / memory / core-topology strip
│   ├── footer.rs        Keybindings + pm: state indicator
│   ├── cpu_panel.rs     Main braille chart + per-core sparklines
│   ├── gpu_panel.rs     GPU utilization and frequency
│   ├── power_panel.rs   CPU / GPU / ANE / SoC power gauges
│   ├── memory_panel.rs  RAM and swap bars
│   ├── io_panel.rs      Disk and network mirror charts
│   └── thermal_panel.rs Temperature bars + throttle indicator in title
│
└── widgets/
    ├── mod.rs           Public re-exports
    ├── braille.rs       BrailleCanvas — 2×4 dot grid, Unicode encoding
    ├── braille_chart.rs BrailleChart widget (FilledArea, Sparkline, …)
    ├── gradient.rs      Fallback color helper
    └── gradient_gauge.rs Horizontal bar gauge with gradient fill
```

### Data flow

```
macmon::Sampler  ─┐
sysinfo::System  ─┤──► MetricsHistory (RingBuffer<f32> per metric)
powermetrics(8)  ─┘

AppState::run()  ──► poll() drains channels → MetricsHistory updated
                 ──► terminal.draw() → ui::draw() → panel widgets
```

---

## Development workflow

### Live reload

```bash
cargo watch -x run
```

### Check without running

```bash
cargo check                  # fast type-check
cargo clippy -- -D warnings  # lints, zero-warning policy
```

### Release build

```bash
cargo build --release
```

The `[profile.release]` in `Cargo.toml` enables `lto = true` for a significantly smaller binary.

---

## Testing

### Unit / parser tests

```bash
cargo test
```

The `pm_dump_smoketest` test is skipped unless `PM_DUMP` is set:

```bash
# Capture a real powermetrics dump (needs sudo)
sudo powermetrics --format plist --samplers cpu_power,gpu_power,thermal \
  -i 1000 > /tmp/pm_test.xml &
sleep 3 && kill %1

PM_DUMP=/tmp/pm_test.xml cargo test pm_dump_smoketest -- --nocapture
```

### Live powermetrics integration test

```bash
# Requires root; spawns the real powermetrics process
PM_LIVE=1 sudo -E cargo test pm_live -- --nocapture --test-threads=1
```

### Adding tests

- **Parser / pure logic** — add `#[test]` functions inside the relevant module's `mod tests` block.
- **Rendering** — ratatui provides `TestBackend` / `Buffer::assert_eq` for snapshot-style widget tests. See the [ratatui testing docs](https://ratatui.rs/concepts/testing/).
- **Powermetrics** — gate real-subprocess tests behind an env var (`PM_LIVE`, `PM_DUMP`) so CI doesn't need root.

---

## Submitting a pull request

1. **Fork** the repository and create a feature branch:

   ```bash
   git checkout -b feat/your-feature-name
   ```

2. **Make your changes**, keeping commits focused and the history clean.

3. **Ensure all checks pass locally:**

   ```bash
   cargo test
   cargo clippy -- -D warnings
   ```

4. **Open a PR** against the `master` branch. Include:
   - What the change does and why.
   - Any screenshots or terminal recordings if it's a visual change.
   - Notes on testing steps, especially if root is needed.

### PR size

Small, focused PRs merge faster. If you're working on something large, open a draft PR early so design decisions can be discussed before the full implementation is done.

---

## Code style

- **No unnecessary comments** — code should be self-documenting. Add a comment only when the *why* is non-obvious (a workaround, a hidden constraint, a subtle invariant).
- **No docstrings on obvious things** — short single-line module docs (`//!`) for context, not mechanical descriptions of what functions do.
- **Clippy clean** — the project enforces zero warnings under `cargo clippy -- -D warnings`. Please keep it that way.
- **No feature creep** — avoid adding abstractions for hypothetical future needs. Three similar call sites is not a reason to extract a helper.
- **No unsafe without justification** — the only `unsafe` in the codebase is the `geteuid()` syscall in `powermetrics.rs`, which is documented. New unsafe blocks need a clear comment explaining why they are sound.

### Error handling

- Prefer early returns over deep nesting.
- `powermetrics.rs` uses best-effort I/O (`let _ = ...`) for log writes — this is intentional; a broken log must not crash the monitor.
- Chart / widget rendering: silently return early on degenerate sizes (zero-width, zero-height areas). Never panic in render paths.

---

## Areas that need help

The following are known gaps where contributions are especially welcome:

- **Automated screenshots / demo recording** — a `vhs`-based terminal recording for the README would make a big difference.
- **CI workflow** — a GitHub Actions pipeline that runs `cargo test` and `cargo clippy` on every PR (macOS runner).
- **Homebrew formula** — for easy installation via `brew install basitop`.
- **More samplers** — battery charge/health, fan speed (where exposed by IOKit), process-level breakdown.
- **Configurable layout** — allow users to hide panels they don't need via a config file or CLI flags.
- **Color themes** — currently the palette is hard-coded in `theme.rs`; a TOML config for colors would be nice.
