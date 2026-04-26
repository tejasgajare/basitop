use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::DefaultTerminal;

use crate::metrics::MetricsCollector;

const PANEL_COUNT: u8 = 6;

pub struct AppState {
    pub running: bool,
    pub selected_panel: u8,
    pub show_help: bool,
    pub update_interval_ms: u32,
    pub collector: MetricsCollector,
    pub chip_name: String,
    pub memory_gb: u8,
    pub ecpu_cores: u8,
    pub pcpu_cores: u8,
    pub ecpu_label: String,
    pub pcpu_label: String,
    pub gpu_cores: u8,
}

impl AppState {
    pub fn new(collector: MetricsCollector, update_interval_ms: u32) -> Self {
        Self {
            running: true,
            selected_panel: 0,
            show_help: false,
            update_interval_ms,
            collector,
            chip_name: String::new(),
            memory_gb: 0,
            ecpu_cores: 0,
            pcpu_cores: 0,
            ecpu_label: "E".into(),
            pcpu_label: "P".into(),
            gpu_cores: 0,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut last_tick = Instant::now();

        while self.running {
            // Refresh per-iteration so +/- key updates take effect
            // immediately — both for the redraw cadence and for the
            // sampler thread's window length.
            let tick_duration = Duration::from_millis(self.update_interval_ms as u64);

            terminal.draw(|frame| {
                crate::ui::draw(frame.area(), frame.buffer_mut(), self);
            })?;

            let timeout = tick_duration
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::ZERO);

            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => self.handle_key(key),
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }

            if last_tick.elapsed() >= tick_duration {
                self.collector.poll();
                last_tick = Instant::now();
            }
        }

        self.collector.stop();
        Ok(())
    }

    fn apply_interval_change(&self) {
        // Sampler window is half the redraw interval (matches main.rs
        // initial wiring) so the chart never shows the same sample
        // twice in a row. Floor at 250 ms — sysinfo and macmon both
        // need at least that to produce non-stale deltas.
        let sample_ms = (self.update_interval_ms / 2).max(250);
        self.collector.set_sample_interval(sample_ms);
    }

    fn handle_key(&mut self, key: event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Tab => {
                self.selected_panel = (self.selected_panel + 1) % PANEL_COUNT;
            }
            KeyCode::BackTab => {
                self.selected_panel = (self.selected_panel + PANEL_COUNT - 1) % PANEL_COUNT;
            }
            KeyCode::Char('h') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char('+') | KeyCode::Char('=') if self.update_interval_ms < 10000 => {
                self.update_interval_ms += 100;
                self.apply_interval_change();
            }
            KeyCode::Char('-') if self.update_interval_ms > 200 => {
                self.update_interval_ms -= 100;
                self.apply_interval_change();
            }
            // Arrow key navigation (spatial). Right column stack:
            // 2 power -> 3 memory -> 4 io -> 5 thermal.
            KeyCode::Up => {
                self.selected_panel = match self.selected_panel {
                    3 => 2,
                    4 => 3,
                    5 => 4,
                    _ => self.selected_panel,
                };
            }
            KeyCode::Down => {
                self.selected_panel = match self.selected_panel {
                    2 => 3,
                    3 => 4,
                    4 => 5,
                    _ => self.selected_panel,
                };
            }
            KeyCode::Left => {
                self.selected_panel = match self.selected_panel {
                    2 | 3 => 0, // top of right column -> cpu
                    4 | 5 => 1, // bottom of right column -> gpu
                    _ => self.selected_panel,
                };
            }
            KeyCode::Right => {
                self.selected_panel = match self.selected_panel {
                    0 => 2, // cpu -> power
                    1 => 4, // gpu -> io
                    _ => self.selected_panel,
                };
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, _mouse: event::MouseEvent) {
        // Mouse support: clicking in a panel area selects it
        // This is a simplified version - in practice you'd check coordinates
        // against panel rects, but we'd need to track those from the draw phase
    }
}
