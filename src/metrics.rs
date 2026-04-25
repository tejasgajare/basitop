use macmon::Metrics;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use sysinfo::System;

use crate::powermetrics::{PowerMetricsCollector, PowerSample, ThermalPressure};

// --- RingBuffer ---

#[derive(Clone)]
pub struct RingBuffer<T: Clone + Default> {
    data: Vec<T>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T: Clone + Default> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![T::default(); capacity],
            capacity,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    pub fn last(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        let idx = (self.head + self.capacity - 1) % self.capacity;
        Some(&self.data[idx])
    }

    /// Returns data from oldest to newest as a Vec
    pub fn to_vec(&self) -> Vec<T> {
        if self.len == 0 {
            return vec![];
        }
        let start = (self.head + self.capacity - self.len) % self.capacity;
        let mut result = Vec::with_capacity(self.len);
        for i in 0..self.len {
            let idx = (start + i) % self.capacity;
            result.push(self.data[idx].clone());
        }
        result
    }
}

// --- SampleData (macmon metrics + per-core CPU) ---

pub struct SampleData {
    pub metrics: Metrics,
    pub per_core_usage: Vec<f32>, // per-core CPU usage percentage (0-100)
}

// --- MetricsHistory ---

pub struct MetricsHistory {
    pub ecpu_usage: RingBuffer<f32>,
    pub pcpu_usage: RingBuffer<f32>,
    pub cpu_usage: RingBuffer<f32>,
    pub ecpu_freq: RingBuffer<u32>,
    pub pcpu_freq: RingBuffer<u32>,
    pub gpu_usage: RingBuffer<f32>,
    pub gpu_freq: RingBuffer<u32>,
    pub cpu_power: RingBuffer<f32>,
    pub gpu_power: RingBuffer<f32>,
    pub ane_power: RingBuffer<f32>,
    pub total_power: RingBuffer<f32>,
    pub sys_power: RingBuffer<f32>,
    pub ram_percent: RingBuffer<f32>,
    pub swap_percent: RingBuffer<f32>,
    pub cpu_temp: RingBuffer<f32>,
    pub gpu_temp: RingBuffer<f32>,
    // Per-core CPU history. `core_count` is sticky: once we observe N cores
    // we never shrink the buffer set, so a transient zero-core read from
    // sysinfo (e.g. during sleep/wake) doesn't wipe history.
    pub per_core_usage: Vec<RingBuffer<f32>>,
    pub core_count: usize,
    // --- powermetrics-sourced fields (zero / Unknown if unavailable) ---
    pub dram_power: RingBuffer<f32>,
    pub package_power: RingBuffer<f32>,
    pub combined_power: RingBuffer<f32>,
    pub disk_read_bps: RingBuffer<f32>,
    pub disk_write_bps: RingBuffer<f32>,
    pub net_rx_bps: RingBuffer<f32>,
    pub net_tx_bps: RingBuffer<f32>,
    pub thermal_pressure: ThermalPressure,
    pub battery_drain_w: f32,
}

impl MetricsHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            ecpu_usage: RingBuffer::new(capacity),
            pcpu_usage: RingBuffer::new(capacity),
            cpu_usage: RingBuffer::new(capacity),
            ecpu_freq: RingBuffer::new(capacity),
            pcpu_freq: RingBuffer::new(capacity),
            gpu_usage: RingBuffer::new(capacity),
            gpu_freq: RingBuffer::new(capacity),
            cpu_power: RingBuffer::new(capacity),
            gpu_power: RingBuffer::new(capacity),
            ane_power: RingBuffer::new(capacity),
            total_power: RingBuffer::new(capacity),
            sys_power: RingBuffer::new(capacity),
            ram_percent: RingBuffer::new(capacity),
            swap_percent: RingBuffer::new(capacity),
            cpu_temp: RingBuffer::new(capacity),
            gpu_temp: RingBuffer::new(capacity),
            per_core_usage: Vec::new(),
            core_count: 0,
            dram_power: RingBuffer::new(capacity),
            package_power: RingBuffer::new(capacity),
            combined_power: RingBuffer::new(capacity),
            disk_read_bps: RingBuffer::new(capacity),
            disk_write_bps: RingBuffer::new(capacity),
            net_rx_bps: RingBuffer::new(capacity),
            net_tx_bps: RingBuffer::new(capacity),
            thermal_pressure: ThermalPressure::Unknown,
            battery_drain_w: 0.0,
        }
    }

    /// Merge a powermetrics sample into the history. Missing fields fall
    /// back to 0.0 so chart rendering still has a clean signal.
    pub fn update_power(&mut self, ps: &PowerSample) {
        self.dram_power
            .push(ps.dram_power_w.unwrap_or(0.0).max(0.0));
        self.package_power
            .push(ps.package_power_w.unwrap_or(0.0).max(0.0));
        self.combined_power
            .push(ps.combined_power_w.unwrap_or(0.0).max(0.0));
        self.disk_read_bps.push(ps.disk_read_bps.max(0.0) as f32);
        self.disk_write_bps.push(ps.disk_write_bps.max(0.0) as f32);
        self.net_rx_bps.push(ps.net_rx_bps.max(0.0) as f32);
        self.net_tx_bps.push(ps.net_tx_bps.max(0.0) as f32);
        if let Some(tp) = ps.thermal_pressure {
            self.thermal_pressure = tp;
        }
        if let Some(d) = ps.battery_drain_w {
            self.battery_drain_w = d;
        }
    }

    pub fn update(&mut self, sample: &SampleData) {
        let m = &sample.metrics;
        self.ecpu_usage.push(m.ecpu_usage.1 * 100.0);
        self.pcpu_usage.push(m.pcpu_usage.1 * 100.0);
        self.cpu_usage.push(m.cpu_usage_pct * 100.0);
        self.ecpu_freq.push(m.ecpu_usage.0);
        self.pcpu_freq.push(m.pcpu_usage.0);
        self.gpu_usage.push(m.gpu_usage.1 * 100.0);
        self.gpu_freq.push(m.gpu_usage.0);
        self.cpu_power.push(m.cpu_power);
        self.gpu_power.push(m.gpu_power);
        self.ane_power.push(m.ane_power);
        self.total_power.push(m.all_power);
        self.sys_power.push(m.sys_power);

        let ram_pct = if m.memory.ram_total > 0 {
            (m.memory.ram_usage as f32 / m.memory.ram_total as f32) * 100.0
        } else {
            0.0
        };
        let swap_pct = if m.memory.swap_total > 0 {
            (m.memory.swap_usage as f32 / m.memory.swap_total as f32) * 100.0
        } else {
            0.0
        };
        self.ram_percent.push(ram_pct);
        self.swap_percent.push(swap_pct);
        self.cpu_temp.push(m.temp.cpu_temp_avg);
        self.gpu_temp.push(m.temp.gpu_temp_avg);

        // Per-core CPU. We grow the buffer set the first time we see N
        // cores and after that only grow (never shrink) — sysinfo can
        // briefly return 0 cores during sleep/wake transitions and we
        // don't want that to nuke history.
        let n = sample.per_core_usage.len();
        let cap = self.ecpu_usage.capacity;
        if n > self.per_core_usage.len() {
            self.per_core_usage
                .resize_with(n, || RingBuffer::new(cap));
        }
        if n > self.core_count {
            self.core_count = n;
        }
        // Push exactly one sample to every buffer so all per-core charts
        // stay aligned in time with the main metrics. If sysinfo returned
        // fewer cores than we've previously seen, the missing tail falls
        // back to carry-last on a per-buffer basis.
        for (i, rb) in self.per_core_usage.iter_mut().enumerate() {
            let usage = sample
                .per_core_usage
                .get(i)
                .copied()
                .or_else(|| rb.last().copied())
                .unwrap_or(0.0);
            rb.push(usage);
        }
    }
}

// --- MetricsCollector ---

pub struct MetricsCollector {
    pub history: MetricsHistory,
    pub latest: Option<Metrics>,
    pub latest_power: Option<PowerSample>,
    #[allow(dead_code)]
    pub powermetrics_available: bool,
    receiver: mpsc::Receiver<SampleData>,
    pm: Option<PowerMetricsCollector>,
    _running: Arc<AtomicBool>,
}

impl MetricsCollector {
    pub fn new(
        history_capacity: usize,
        sample_duration_ms: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut sampler = match macmon::Sampler::new() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to initialize sampler: {}", e);
                    return;
                }
            };

            let mut sys = System::new();
            // Establish a baseline snapshot, then wait out sysinfo's
            // MINIMUM_CPU_UPDATE_INTERVAL (200ms) so the first real refresh
            // inside the loop produces a non-stale delta.
            sys.refresh_cpu_usage();
            thread::sleep(std::time::Duration::from_millis(200));

            while running_clone.load(Ordering::Relaxed) {
                match sampler.get_metrics(sample_duration_ms) {
                    Ok(metrics) => {
                        // One refresh per iteration. The window is the gap
                        // between successive calls here, which is always
                        // >= sample_duration_ms (clamped to >= 250ms in
                        // main.rs) and therefore safely above sysinfo's
                        // 200ms staleness threshold.
                        sys.refresh_cpu_usage();
                        let per_core_usage: Vec<f32> =
                            sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();

                        let sample = SampleData {
                            metrics,
                            per_core_usage,
                        };

                        if tx.send(sample).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        });

        // Try to bring up powermetrics in parallel. If sudo isn't cached
        // and we aren't root the spawn fails fast and we silently fall
        // back to macmon-only mode.
        let pm = PowerMetricsCollector::spawn(sample_duration_ms.max(500));
        let pm_available = pm.available;
        let pm = if pm_available { Some(pm) } else { None };

        Ok(Self {
            history: MetricsHistory::new(history_capacity),
            latest: None,
            latest_power: None,
            powermetrics_available: pm_available,
            receiver: rx,
            pm,
            _running: running,
        })
    }

    /// Drain all pending metrics, keeping only the latest
    pub fn poll(&mut self) {
        while let Ok(sample) = self.receiver.try_recv() {
            self.history.update(&sample);
            self.latest = Some(sample.metrics);
        }
        if let Some(pm) = &self.pm {
            if let Some(ps) = pm.drain_latest() {
                self.history.update_power(&ps);
                self.latest_power = Some(ps);
            }
        }
    }

    pub fn stop(&mut self) {
        self._running.store(false, Ordering::Relaxed);
        if let Some(mut pm) = self.pm.take() {
            pm.stop();
        }
    }

    /// True when powermetrics is running and producing samples. False if it
    /// never started (no sudo, missing binary) OR if the child has since
    /// died (sudo creds expired, OOM, manual kill, etc.).
    pub fn pm_alive(&self) -> bool {
        match &self.pm {
            Some(pm) => !pm.is_dead(),
            None => false,
        }
    }
}
