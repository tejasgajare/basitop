//! Optional companion to macmon that pulls extra metrics out of
//! `powermetrics(8)` — thermal pressure, disk/network throughput, DRAM and
//! package power. Requires root; if `sudo -n` fails the collector silently
//! marks itself unavailable and the rest of the app still works on macmon
//! data alone.
//!
//! powermetrics emits one plist document per sample on stdout, terminated
//! by a NUL byte. We consume the stream in a background thread and forward
//! parsed samples through a channel.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_PATH: &str = "/tmp/basitop_pm.log";

/// Wipe the log so stale messages from previous runs (e.g. an old
/// "sudo: password required" from a non-sudo run) don't confuse the user.
fn truncate_log() {
    let _ = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(LOG_PATH);
}

/// Append a timestamped diagnostic line. Best-effort: silently drops if the
/// file isn't writable. Format: `[HH:MM:SS.mmm] [basitop] msg`.
fn log_event(msg: &str) {
    let mut f = match OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        Ok(f) => f,
        Err(_) => return,
    };
    let _ = writeln!(f, "[{}] [basitop] {}", clock_ts(), msg);
}

fn clock_ts() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let ms = d.subsec_millis();
            let h = (secs / 3600) % 24;
            let m = (secs / 60) % 60;
            let s = secs % 60;
            format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
        }
        Err(_) => "??:??:??.???".into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThermalPressure {
    #[default]
    Unknown,
    Nominal,
    Fair,
    Serious,
    Critical,
}

impl ThermalPressure {
    pub fn parse(s: &str) -> Self {
        // powermetrics has shipped slightly different labels across macOS
        // versions; normalize to the four canonical buckets.
        match s.trim().to_ascii_lowercase().as_str() {
            "nominal" => Self::Nominal,
            "fair" | "moderate" => Self::Fair,
            "serious" | "heavy" => Self::Serious,
            "critical" | "trapping" | "sleeping" => Self::Critical,
            _ => Self::Unknown,
        }
    }

    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Nominal => "NOMINAL",
            Self::Fair => "FAIR",
            Self::Serious => "SERIOUS",
            Self::Critical => "CRITICAL",
            Self::Unknown => "—",
        }
    }

    #[allow(dead_code)]
    pub fn is_throttling(self) -> bool {
        matches!(self, Self::Serious | Self::Critical)
    }

    /// Heat-style ratio used to color the indicator (0=cool, 1=hot).
    #[allow(dead_code)]
    pub fn intensity(self) -> f32 {
        match self {
            Self::Unknown => 0.0,
            Self::Nominal => 0.15,
            Self::Fair => 0.5,
            Self::Serious => 0.8,
            Self::Critical => 1.0,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct PowerSample {
    pub thermal_pressure: Option<ThermalPressure>,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,
    pub dram_power_w: Option<f32>,
    pub package_power_w: Option<f32>,
    pub combined_power_w: Option<f32>,
    pub battery_drain_w: Option<f32>,
}

pub struct PowerMetricsCollector {
    rx: mpsc::Receiver<PowerSample>,
    running: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    /// True when the spawn call itself succeeded. Doesn't guarantee the
    /// child is still alive — see `is_dead()`.
    pub available: bool,
    /// Set by the reader thread when it exits (EOF / error). The most
    /// common cause is `sudo -n` failing because creds aren't cached:
    /// powermetrics never starts, sudo prints to stderr, child exits.
    dead: Arc<AtomicBool>,
}

impl PowerMetricsCollector {
    /// Try to spawn a long-running powermetrics process. If it can't start
    /// (no sudo, missing binary, etc.) returns a stub collector with
    /// `available = false`. All lifecycle events go to /tmp/basitop_pm.log.
    pub fn spawn(interval_ms: u32) -> Self {
        truncate_log();
        let root = is_root();
        log_event(&format!(
            "spawn start: interval_ms={} euid_is_root={}",
            interval_ms, root
        ));

        let (tx, rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let (program, mut full_args): (&str, Vec<String>) = if root {
            ("powermetrics", Vec::new())
        } else {
            // -n => non-interactive sudo; fail fast if no cached creds.
            ("sudo", vec!["-n".into(), "powermetrics".into()])
        };
        // NOTE: do NOT pass `-n 0`. The man page claims 0 means "until
        // interrupted", but on macOS 26 / M5 it actually exits with rc=0
        // and zero output. Omitting `-n` entirely is the documented
        // "run forever" path on every macOS version we care about.
        let pm_args: [String; 6] = [
            "--format".into(),
            "plist".into(),
            "--samplers".into(),
            "cpu_power,gpu_power,thermal,disk,network,battery".into(),
            "-i".into(),
            interval_ms.to_string(),
        ];
        full_args.extend(pm_args.iter().cloned());
        log_event(&format!("exec: {} {}", program, full_args.join(" ")));

        let mut cmd = Command::new(program);
        cmd.args(&full_args);
        cmd.stdout(Stdio::piped());
        // Mirror powermetrics' stderr to the same log so any sudo/version/
        // permission errors land alongside our diagnostic events. O_APPEND
        // is atomic on POSIX so the two writers can't tear each other.
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOG_PATH)
            .map(Stdio::from)
            .unwrap_or_else(|_| Stdio::null());
        cmd.stderr(stderr);
        cmd.stdin(Stdio::null());

        let dead = Arc::new(AtomicBool::new(false));

        let mut child = match cmd.spawn() {
            Ok(c) => {
                log_event(&format!("spawn ok: pid={}", c.id()));
                c
            }
            Err(e) => {
                log_event(&format!("spawn FAILED: {} (kind={:?})", e, e.kind()));
                dead.store(true, Ordering::Relaxed);
                return Self {
                    rx,
                    running,
                    child: Arc::new(Mutex::new(None)),
                    available: false,
                    dead,
                };
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                log_event("spawn FAILED: no stdout pipe attached, killing child");
                let _ = child.kill();
                let _ = child.wait();
                dead.store(true, Ordering::Relaxed);
                return Self {
                    rx,
                    running,
                    child: Arc::new(Mutex::new(None)),
                    available: false,
                    dead,
                };
            }
        };

        let child_handle: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
        let child_for_reader = child_handle.clone();
        let dead_clone = dead.clone();
        thread::spawn(move || {
            log_event("reader thread started");
            let count = run_reader(stdout, tx, running_clone);
            log_event(&format!("reader thread exiting: parsed={} samples", count));
            // Try to capture the child's exit status so the user can see
            // *why* it died (e.g. sudo: 1, missing binary: 127, killed: -9).
            // Use try_lock: if stop() is mid-kill+wait it already owns the
            // mutex and will surface the status itself — blocking here would
            // just deadlock on shutdown.
            match child_for_reader.try_lock() {
                Ok(mut guard) => match guard.as_mut() {
                    Some(c) => match c.try_wait() {
                        Ok(Some(status)) => log_event(&format!("child exit status: {}", status)),
                        Ok(None) => match c.wait() {
                            Ok(status) => {
                                log_event(&format!("child exit status (after wait): {}", status))
                            }
                            Err(e) => log_event(&format!("wait err: {}", e)),
                        },
                        Err(e) => log_event(&format!("try_wait err: {}", e)),
                    },
                    None => log_event("child handle already taken (stop() ran first)"),
                },
                Err(_) => log_event("reader: stop() in progress, skipping exit-status check"),
            }
            dead_clone.store(true, Ordering::Relaxed);
        });

        Self {
            rx,
            running,
            child: child_handle,
            available: true,
            dead,
        }
    }

    /// True when the reader thread has exited. The footer uses this to flip
    /// the indicator from WAIT (just spawned, no samples yet) to OFF (child
    /// is gone — usually a sudo / permissions issue).
    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }

    /// Drain pending samples; returns the most recent if any.
    pub fn drain_latest(&self) -> Option<PowerSample> {
        let mut latest = None;
        while let Ok(s) = self.rx.try_recv() {
            latest = Some(s);
        }
        latest
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut c) = guard.take() {
                log_event("stop(): killing child");
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

impl Drop for PowerMetricsCollector {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(unix)]
fn is_root() -> bool {
    // Safe: geteuid is a thread-safe, no-arg syscall.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn is_root() -> bool {
    false
}

/// Returns the number of plist samples successfully parsed before the
/// reader exited. Used purely for diagnostics in the spawn-side log line.
fn run_reader<R: Read>(
    stdout: R,
    tx: mpsc::Sender<PowerSample>,
    running: Arc<AtomicBool>,
) -> usize {
    let mut reader = BufReader::new(stdout);
    let mut buffer: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut parsed: usize = 0;
    let mut parse_errors: usize = 0;

    loop {
        if !running.load(Ordering::Relaxed) {
            log_event("reader: stop signal received");
            break;
        }
        buffer.clear();
        match reader.read_until(0u8, &mut buffer) {
            Ok(0) => {
                log_event("reader: EOF on stdout (child closed pipe)");
                break;
            }
            Ok(n) => {
                // Strip the trailing NUL terminator before parsing.
                if buffer.last() == Some(&0u8) {
                    buffer.pop();
                }
                if buffer.is_empty() {
                    continue;
                }
                match plist::from_bytes::<plist::Value>(&buffer) {
                    Ok(value) => {
                        let sample = parse_sample(&value);
                        parsed += 1;
                        if parsed == 1 {
                            log_event(&format!(
                                "reader: first sample parsed ({} bytes incl NUL)",
                                n
                            ));
                        }
                        if tx.send(sample).is_err() {
                            log_event("reader: receiver dropped, exiting");
                            break;
                        }
                    }
                    Err(e) => {
                        // Throttle: only log the first few errors. powermetrics
                        // occasionally emits a non-plist banner before the
                        // first real sample, so the first error is informative
                        // but a stream of them after is just noise.
                        parse_errors += 1;
                        if parse_errors <= 3 {
                            let preview: String = buffer
                                .iter()
                                .take(80)
                                .map(|&b| {
                                    if (32..127).contains(&b) {
                                        b as char
                                    } else {
                                        '.'
                                    }
                                })
                                .collect();
                            log_event(&format!(
                                "reader: parse err #{} ({} bytes): {} | preview={:?}",
                                parse_errors, n, e, preview
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                log_event(&format!("reader: read err: {}", e));
                break;
            }
        }
    }
    parsed
}

fn parse_sample(v: &plist::Value) -> PowerSample {
    let mut s = PowerSample::default();
    let dict = match v.as_dictionary() {
        Some(d) => d,
        None => return s,
    };

    // Elapsed window for energy → power conversion when only joules ship.
    let elapsed_s = num(dict.get("elapsed_ns"))
        .filter(|v| *v > 0.0)
        .map(|ns| ns / 1.0e9);

    if let Some(tp) = dict.get("thermal_pressure").and_then(|v| v.as_string()) {
        s.thermal_pressure = Some(ThermalPressure::parse(tp));
    }

    if let Some(disk) = dict.get("disk").and_then(|v| v.as_dictionary()) {
        s.disk_read_bps = num(disk.get("rbytes_per_s")).unwrap_or(0.0);
        s.disk_write_bps = num(disk.get("wbytes_per_s")).unwrap_or(0.0);
    }

    if let Some(net) = dict.get("network").and_then(|v| v.as_dictionary()) {
        // powermetrics' network sampler uses singular `ibyte_rate`/`obyte_rate`
        // (not `*bytes_per_s` like the disk sampler). Keep the *_per_s names
        // as fallbacks in case a future macOS version unifies them.
        s.net_rx_bps = num(net.get("ibyte_rate"))
            .or_else(|| num(net.get("ibytes_per_s")))
            .unwrap_or(0.0);
        s.net_tx_bps = num(net.get("obyte_rate"))
            .or_else(|| num(net.get("obytes_per_s")))
            .unwrap_or(0.0);
    }

    // On Apple Silicon, power readings are at the top level of the plist
    // (cpu_power, gpu_power, ane_power, combined_power — all in mW).
    // The legacy `processor.*` nesting and `package_power`/`dram_power`
    // fields are an Intel-era schema that Apple Silicon doesn't emit.
    s.combined_power_w = mw_to_w(num(dict.get("combined_power")));
    // Older macOS / Intel still expose package/dram via either layout —
    // keep the fallbacks so this code degrades cleanly on those builds.
    if let Some(proc_) = dict.get("processor").and_then(|v| v.as_dictionary()) {
        s.package_power_w = mw_to_w(num(proc_.get("package_power")))
            .or_else(|| joules_to_w(num(proc_.get("package_joules")), elapsed_s));
        s.dram_power_w = mw_to_w(num(proc_.get("dram_power")))
            .or_else(|| joules_to_w(num(proc_.get("dram_joules")), elapsed_s));
        if s.combined_power_w.is_none() {
            s.combined_power_w = mw_to_w(num(proc_.get("combined_power")))
                .or_else(|| mw_to_w(num(proc_.get("soc_power"))));
        }
    }

    if let Some(batt) = dict.get("battery").and_then(|v| v.as_dictionary()) {
        // Apple reports drain as a positive number when discharging; varies.
        s.battery_drain_w = num(batt.get("drain_now"))
            .or_else(|| num(batt.get("drain")))
            .map(|v| v as f32);
    }

    s
}

fn mw_to_w(v: Option<f64>) -> Option<f32> {
    v.map(|mw| (mw / 1000.0) as f32)
}

fn joules_to_w(joules: Option<f64>, elapsed_s: Option<f64>) -> Option<f32> {
    let j = joules?;
    let s = elapsed_s?;
    if s > 0.0 {
        Some((j / s) as f32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live end-to-end test: spawn the real collector, wait a few seconds,
    /// confirm at least one sample was parsed. Requires root, so it's gated
    /// behind PM_LIVE=1. Run with:
    ///   PM_LIVE=1 sudo -E cargo test pm_live -- --nocapture --test-threads=1
    #[test]
    fn pm_live() {
        if std::env::var("PM_LIVE").is_err() {
            eprintln!("(skipped — set PM_LIVE=1 to run; needs root)");
            return;
        }
        let pm = PowerMetricsCollector::spawn(500);
        assert!(pm.available, "spawn() returned unavailable");
        let mut samples = 0;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if pm.drain_latest().is_some() {
                samples += 1;
            }
            if pm.is_dead() {
                break;
            }
        }
        // Dump the log file to test output for triage.
        if let Ok(text) = std::fs::read_to_string(LOG_PATH) {
            eprintln!("--- {} ---\n{}", LOG_PATH, text);
        }
        assert!(
            samples > 0,
            "no samples parsed in 4s — see {} for diagnostics",
            LOG_PATH
        );
    }

    /// Sanity check: point PM_DUMP at a NUL-separated plist dump and
    /// confirm the parser populates every field we render. Run with:
    ///   PM_DUMP=/tmp/pm_out2.xml cargo test pm_dump_smoketest -- --nocapture
    #[test]
    fn pm_dump_smoketest() {
        let path = match std::env::var("PM_DUMP") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("(skipped — set PM_DUMP=/path/to/plist-dump to run)");
                return;
            }
        };
        let data = std::fs::read(&path).expect("read PM_DUMP");
        let mut samples = 0;
        for (i, chunk) in data.split(|&b| b == 0u8).enumerate() {
            if chunk.is_empty() {
                continue;
            }
            match plist::from_bytes::<plist::Value>(chunk) {
                Ok(v) => {
                    let s = parse_sample(&v);
                    samples += 1;
                    println!(
                        "sample {}: tp={:?} combined={:?}W disk_r/w={}/{} net_rx/tx={}/{}",
                        i + 1,
                        s.thermal_pressure,
                        s.combined_power_w,
                        s.disk_read_bps,
                        s.disk_write_bps,
                        s.net_rx_bps,
                        s.net_tx_bps,
                    );
                }
                Err(e) => println!("sample {}: parse error: {}", i + 1, e),
            }
        }
        assert!(samples > 0, "no samples parsed from {}", path);
    }
}

fn num(v: Option<&plist::Value>) -> Option<f64> {
    let v = v?;
    if let Some(i) = v.as_signed_integer() {
        return Some(i as f64);
    }
    if let Some(u) = v.as_unsigned_integer() {
        return Some(u as f64);
    }
    if let Some(f) = v.as_real() {
        return Some(f);
    }
    if let Some(s) = v.as_string() {
        return s.parse().ok();
    }
    None
}
