use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{DIM_TEXT, HEADER_BG, LABEL_COLOR};

#[derive(Clone, Copy)]
pub enum PmState {
    Off,
    Waiting,
    Running,
}

pub struct Footer {
    pub interval_ms: u32,
    pub pm_state: PmState,
}

impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer) {
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

        // Right side: btop-style "─  - 1000ms +  ─"
        // Fixed width: longest interval "10000ms" = 7 chars → total = 14
        let interval_str = format!("{}ms", self.interval_ms);
        let right_width: u16 = 14;

        let [left_area, right_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(right_width)])
                .areas(area);

        // Left: keybindings + pm state
        let mut left_spans = vec![
            Span::styled(" q", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Quit  ", Style::default().fg(DIM_TEXT)),
            Span::styled("Tab", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Navigate  ", Style::default().fg(DIM_TEXT)),
            Span::styled("h", Style::default().fg(LABEL_COLOR)),
            Span::styled(" Help  ", Style::default().fg(DIM_TEXT)),
            Span::styled("pm:", Style::default().fg(LABEL_COLOR)),
            Span::styled(format!(" {}", pm_label), Style::default().fg(pm_color).bold()),
        ];
        if matches!(self.pm_state, PmState::Off) {
            left_spans.push(Span::styled(
                "  (run with sudo for thermal/IO/SoC)",
                Style::default().fg(DIM_TEXT),
            ));
        }
        Line::from(left_spans).render(left_area, buf);

        // Right: btop-style interval control
        //   " - 1000ms + "
        //   red dash · bright interval · green plus
        let padding = right_width.saturating_sub(2 + interval_str.len() as u16 + 4) / 2;
        let pad = " ".repeat(padding as usize);
        Line::from(vec![
            Span::styled(format!("{} ", pad), Style::default().fg(DIM_TEXT)),
            Span::styled("-", Style::default().fg(Color::Rgb(220, 80, 80)).bold()),
            Span::styled(
                format!(" {} ", interval_str),
                Style::default().fg(Color::Rgb(220, 220, 220)).bold(),
            ),
            Span::styled("+", Style::default().fg(Color::Rgb(80, 200, 100)).bold()),
            Span::styled(format!(" {}", pad), Style::default().fg(DIM_TEXT)),
        ])
        .render(right_area, buf);
    }
}
