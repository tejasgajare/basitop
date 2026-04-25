use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::theme::{
    self, BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, DIM_TEXT, LABEL_COLOR, TITLE_COLOR,
};
use crate::widgets::{BrailleChart, ChartMode};

const TEMP_MAX: f32 = 110.0;

pub struct TempPanel<'a> {
    history: &'a MetricsHistory,
    selected: bool,
}

impl<'a> TempPanel<'a> {
    pub fn new(history: &'a MetricsHistory, selected: bool) -> Self {
        Self { history, selected }
    }
}

impl Widget for TempPanel<'_> {
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

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(Span::styled(" Temperature ", title_style)));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 4 || inner.width < 12 {
            return;
        }

        let palette = theme::temp_palette();

        // Two stacked thermal lanes, each with its own header + heat strip.
        let lane = inner.height / 2;
        let cpu_h = lane.max(2);
        let gpu_h = inner.height.saturating_sub(cpu_h).max(2);

        let lanes =
            Layout::vertical([Constraint::Length(cpu_h), Constraint::Length(gpu_h)]).split(inner);

        let cpu_temp = self.history.cpu_temp.last().copied().unwrap_or(0.0);
        let cpu_data = self.history.cpu_temp.to_vec();
        render_thermal_lane(lanes[0], buf, "CPU", cpu_temp, &cpu_data, &palette);

        let gpu_temp = self.history.gpu_temp.last().copied().unwrap_or(0.0);
        let gpu_data = self.history.gpu_temp.to_vec();
        render_thermal_lane(lanes[1], buf, "GPU", gpu_temp, &gpu_data, &palette);
    }
}

fn render_thermal_lane(
    area: Rect,
    buf: &mut Buffer,
    label: &str,
    current: f32,
    data: &[f32],
    palette: &theme::GradientPalette,
) {
    if area.height == 0 || area.width < 12 {
        return;
    }

    let ratio = (current / TEMP_MAX).clamp(0.0, 1.0);
    let header_color = palette.color_at(ratio);

    // Inline thermometer-style display: bulb glyph + filled bar segments.
    let header = Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(LABEL_COLOR).bold(),
        ),
        Span::styled(
            format!("{:>5.1}\u{00b0}C", current),
            Style::default().fg(BRIGHT_TEXT).bold(),
        ),
        Span::styled("   ", Style::default()),
        Span::styled("\u{25c9} ", Style::default().fg(header_color)),
        Span::styled(
            thermometer_bar(ratio, 12),
            Style::default().fg(header_color),
        ),
        Span::styled(
            format!("  {:.0}%", ratio * 100.0),
            Style::default().fg(DIM_TEXT),
        ),
    ]);
    header.render(Rect::new(area.x, area.y, area.width, 1), buf);

    if area.height < 2 {
        return;
    }

    // History rendered as a HeatStrip — color carries the meaning.
    let chart_h = area.height.saturating_sub(1);
    let chart_area = Rect::new(area.x, area.y + 1, area.width, chart_h);
    BrailleChart::new(data, ChartMode::HeatStrip)
        .max(TEMP_MAX)
        .palette(palette)
        .render(chart_area, buf);
}

/// Tiny inline bar made of horizontal block-eighth chars; reads like the
/// fluid level of a thermometer.
fn thermometer_bar(ratio: f32, width: usize) -> String {
    let eighths = [
        "\u{258f}", "\u{258e}", "\u{258d}", "\u{258c}", "\u{258b}", "\u{258a}", "\u{2589}",
        "\u{2588}",
    ];
    let total_eighths = (ratio.clamp(0.0, 1.0) as f64 * (width * 8) as f64).round() as usize;
    let mut s = String::with_capacity(width * 4);
    for i in 0..width {
        let level = total_eighths.saturating_sub(i * 8).min(8);
        if level == 0 {
            s.push(' ');
        } else {
            s.push_str(eighths[level - 1]);
        }
    }
    s
}
