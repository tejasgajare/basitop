use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{DIM_TEXT, HEADER_BG, LABEL_COLOR};

#[derive(Clone, Copy)]
pub enum PmState {
    Off,     // spawn failed (no sudo, missing binary, etc.)
    Waiting, // process spawned but no samples parsed yet
    Running, // at least one sample successfully parsed
}

pub struct Footer {
    pub interval_ms: u32,
    pub pm_state: PmState,
}

impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, area.y)) {
                cell.set_style(Style::default().bg(HEADER_BG));
            }
        }

        let (pm_label, pm_color) = match self.pm_state {
            PmState::Off => ("OFF", Color::Rgb(230, 90, 90)),
            PmState::Waiting => ("WAIT", Color::Rgb(220, 180, 80)),
            PmState::Running => ("ON", Color::Rgb(120, 200, 140)),
        };

        let mut spans = vec![
            Span::styled(" q", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Quit  ", Style::default().fg(DIM_TEXT)),
            Span::styled("Tab", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Navigate  ", Style::default().fg(DIM_TEXT)),
            Span::styled("+/-", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Interval  ", Style::default().fg(DIM_TEXT)),
            Span::styled("h", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Help  ", Style::default().fg(DIM_TEXT)),
            Span::styled(
                format!("  [{:.1}s]  ", self.interval_ms as f32 / 1000.0),
                Style::default().fg(DIM_TEXT),
            ),
            Span::styled("pm:", Style::default().fg(LABEL_COLOR)),
            Span::styled(
                format!(" {}", pm_label),
                Style::default().fg(pm_color).bold(),
            ),
        ];
        // Actionable hint when off: thermal pressure / SoC power / disk /
        // network all need root. Tell the user how to fix it.
        if matches!(self.pm_state, PmState::Off) {
            spans.push(Span::styled(
                "  (run with sudo for thermal/IO/SoC)",
                Style::default().fg(DIM_TEXT),
            ));
        }
        Line::from(spans).render(area, buf);
    }
}
