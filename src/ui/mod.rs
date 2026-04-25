mod cpu_panel;
mod footer;
mod gpu_panel;
mod header;
mod io_panel;
mod memory_panel;
mod power_panel;
mod thermal_panel;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::app::AppState;
use crate::theme::{BORDER_SELECTED, BRIGHT_TEXT, DIM_TEXT, HEADER_BG, LABEL_COLOR};

use cpu_panel::CpuPanel;
use footer::{Footer, PmState};
use gpu_panel::GpuPanel;
use header::Header;
use io_panel::IoPanel;
use memory_panel::MemoryPanel;
use power_panel::PowerPanel;
use thermal_panel::ThermalPanel;

pub fn draw(area: Rect, buf: &mut Buffer, state: &AppState) {
    let rows = Layout::vertical([
        Constraint::Length(1), // Header
        Constraint::Fill(1),   // Main content
        Constraint::Length(1), // Footer
    ])
    .split(area);

    // Header
    Header::new(
        &state.chip_name,
        state.memory_gb,
        state.ecpu_cores,
        &state.ecpu_label,
        state.pcpu_cores,
        &state.pcpu_label,
        state.gpu_cores,
    )
    .render(rows[0], buf);

    // Main content: two columns
    let cols =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(rows[1]);

    // Left column: CPU + GPU
    let left =
        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).split(cols[0]);

    // Right column: Power, Memory, I/O, compact Thermal strip.
    let right = Layout::vertical([
        Constraint::Fill(3),
        Constraint::Fill(2),
        Constraint::Fill(2),
        Constraint::Length(5),
    ])
    .split(cols[1]);

    let history = &state.collector.history;

    CpuPanel::new(
        history,
        &state.ecpu_label,
        &state.pcpu_label,
        state.ecpu_cores,
        state.pcpu_cores,
        state.selected_panel == 0,
    )
    .render(left[0], buf);

    GpuPanel::new(history, state.gpu_cores, state.selected_panel == 1).render(left[1], buf);

    PowerPanel::new(history, state.selected_panel == 2).render(right[0], buf);

    // Memory panel needs current absolute values
    let (ram_total, ram_usage, swap_total, swap_usage) = if let Some(ref m) = state.collector.latest
    {
        (
            m.memory.ram_total,
            m.memory.ram_usage,
            m.memory.swap_total,
            m.memory.swap_usage,
        )
    } else {
        (0, 0, 0, 0)
    };

    MemoryPanel::new(
        history,
        ram_total,
        ram_usage,
        swap_total,
        swap_usage,
        state.selected_panel == 3,
    )
    .render(right[1], buf);

    IoPanel::new(history, state.selected_panel == 4).render(right[2], buf);

    ThermalPanel::new(history, state.selected_panel == 5).render(right[3], buf);

    // Footer with powermetrics liveness indicator. We treat "spawned but
    // since died" as OFF — it lets the user see at a glance when sudo
    // creds expired or the daemon crashed.
    let pm_state = if !state.collector.pm_alive() {
        PmState::Off
    } else if state.collector.latest_power.is_some() {
        PmState::Running
    } else {
        PmState::Waiting
    };
    Footer {
        interval_ms: state.update_interval_ms,
        pm_state,
    }
    .render(rows[2], buf);

    // Help overlay
    if state.show_help {
        render_help_overlay(area, buf);
    }
}

fn render_help_overlay(area: Rect, buf: &mut Buffer) {
    let width = 40u16.min(area.width.saturating_sub(4));
    let height = 14u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    Clear.render(popup, buf);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_SELECTED))
        .title(Line::from(Span::styled(
            " Help ",
            Style::default().fg(BORDER_SELECTED).bold(),
        )));

    let inner = block.inner(popup);
    block.render(popup, buf);

    // Fill background
    for yy in inner.y..inner.y + inner.height {
        for xx in inner.x..inner.x + inner.width {
            if let Some(cell) = buf.cell_mut((xx, yy)) {
                cell.set_style(Style::default().bg(HEADER_BG));
            }
        }
    }

    let help_lines = [
        ("q / Ctrl+C", "Quit"),
        ("Tab / Shift+Tab", "Cycle panels"),
        ("\u{2190}\u{2191}\u{2192}\u{2193}", "Navigate panels"),
        ("+ / =", "Increase interval"),
        ("-", "Decrease interval"),
        ("h", "Toggle this help"),
        ("", ""),
        ("", "basitop v0.1.0"),
        ("", "Apple Silicon Monitor"),
    ];

    for (i, (key, desc)) in help_lines.iter().enumerate() {
        if i as u16 >= inner.height {
            break;
        }
        let y = inner.y + i as u16;
        let line = if key.is_empty() {
            Line::from(Span::styled(*desc, Style::default().fg(DIM_TEXT))).centered()
        } else {
            Line::from(vec![
                Span::styled(format!("  {:>16} ", key), Style::default().fg(LABEL_COLOR)),
                Span::styled(*desc, Style::default().fg(BRIGHT_TEXT)),
            ])
        };
        line.render(Rect::new(inner.x, y, inner.width, 1), buf);
    }
}
