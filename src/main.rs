mod app;
mod metrics;
mod powermetrics;
mod theme;
mod ui;
mod widgets;

use std::io;

use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

use app::AppState;
use metrics::MetricsCollector;

#[derive(Parser)]
#[command(
    name = "basitop",
    about = "Beautiful Apple Silicon performance monitor"
)]
struct Cli {
    /// Update interval in milliseconds
    #[arg(short, long, default_value = "1000")]
    interval: u32,

    /// History buffer size (number of samples). Each Braille sub-column
    /// renders one sample, so a buffer narrower than (chart_width * 2)
    /// leaves the left edge blank until the buffer fills.
    #[arg(long, default_value = "1024")]
    history: usize,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Get SoC info before entering TUI
    let soc = match macmon::SocInfo::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read SoC info: {}", e);
            eprintln!("basitop requires Apple Silicon (M1+) running macOS.");
            std::process::exit(1);
        }
    };

    // Initialize metrics collector (spawns sampling thread)
    let sample_ms = (cli.interval / 2).max(250);
    let collector = match MetricsCollector::new(cli.history, sample_ms) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize metrics: {}", e);
            std::process::exit(1);
        }
    };

    let mut state = AppState::new(collector, cli.interval);
    state.chip_name = soc.chip_name.clone();
    state.memory_gb = soc.memory_gb;
    state.ecpu_cores = soc.ecpu_cores;
    state.pcpu_cores = soc.pcpu_cores;
    state.ecpu_label = soc.ecpu_label.clone();
    state.pcpu_label = soc.pcpu_label.clone();
    state.gpu_cores = soc.gpu_cores;

    // Set panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).expect("Failed to create terminal");

    // Run
    let result = state.run(&mut terminal);

    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}
