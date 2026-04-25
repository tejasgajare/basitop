use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::theme::{
    self, BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, DIM_TEXT, LABEL_COLOR, TITLE_COLOR,
};
use crate::widgets::{BrailleChart, ChartMode, GradientGauge};

pub struct GpuPanel<'a> {
    history: &'a MetricsHistory,
    gpu_cores: u8,
    selected: bool,
}

impl<'a> GpuPanel<'a> {
    pub fn new(history: &'a MetricsHistory, gpu_cores: u8, selected: bool) -> Self {
        Self {
            history,
            gpu_cores,
            selected,
        }
    }
}

impl Widget for GpuPanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.selected {
            BORDER_SELECTED
        } else {
            BORDER_NORMAL
        };
        let title_style = if self.selected {
            Style::default().fg(BORDER_SELECTED).bold()
        } else {
            Style::default().fg(TITLE_COLOR)
        };

        let gpu_pct = self.history.gpu_usage.last().copied().unwrap_or(0.0);
        let gpu_freq = self.history.gpu_freq.last().copied().unwrap_or(0);
        let title = Line::from(vec![
            Span::styled(" GPU ", title_style),
            Span::styled(
                format!("({} cores) ", self.gpu_cores),
                Style::default().fg(LABEL_COLOR),
            ),
            Span::styled(
                format!("{:.0}%", gpu_pct),
                Style::default().fg(BRIGHT_TEXT).bold(),
            ),
            Span::styled(
                format!(" @ {}MHz ", gpu_freq),
                Style::default().fg(LABEL_COLOR),
            ),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        let rows = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        let palette = theme::thermal_palette();
        let data = self.history.gpu_usage.to_vec();
        BrailleChart::new(&data, ChartMode::FilledArea)
            .max(100.0)
            .palette(&palette)
            .render(rows[0], buf);

        GradientGauge::new(
            gpu_pct / 100.0,
            format!("{:5.1}% @ {} MHz", gpu_pct, gpu_freq),
            &palette,
        )
        .render(rows[1], buf);

        let gpu_power = self.history.gpu_power.last().copied().unwrap_or(0.0);
        let peak = data.iter().copied().fold(0.0f32, f32::max);
        let info = Line::from(vec![
            Span::styled("  Power ", Style::default().fg(LABEL_COLOR)),
            Span::styled(
                format!("{:.2}W", gpu_power),
                Style::default().fg(BRIGHT_TEXT),
            ),
            Span::styled(
                format!("  peak {:.0}%", peak),
                Style::default().fg(DIM_TEXT),
            ),
        ]);
        info.render(rows[2], buf);
    }
}
