use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::theme::{
    GradientPalette, Hsl, BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, DIM_TEXT, LABEL_COLOR,
    TITLE_COLOR,
};
use crate::widgets::{BrailleChart, ChartMode};

/// Each lane (CPU/GPU/ANE) gets its own palette so the three power streams
/// stay readable when they're stacked.
fn cpu_power_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(200.0, 0.85, 0.55)),
        (1.0, Hsl::new(195.0, 0.95, 0.65)),
    ])
}

fn gpu_power_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(150.0, 0.65, 0.50)),
        (1.0, Hsl::new(120.0, 0.85, 0.55)),
    ])
}

fn ane_power_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(290.0, 0.70, 0.60)),
        (1.0, Hsl::new(270.0, 0.85, 0.70)),
    ])
}

pub struct PowerPanel<'a> {
    history: &'a MetricsHistory,
    selected: bool,
}

impl<'a> PowerPanel<'a> {
    pub fn new(history: &'a MetricsHistory, selected: bool) -> Self {
        Self { history, selected }
    }
}

struct LaneStats {
    current: f32,
    avg: f32,
    peak: f32,
    chart_max: f32,
}

fn lane_stats(data: &[f32]) -> LaneStats {
    if data.is_empty() {
        return LaneStats {
            current: 0.0,
            avg: 0.0,
            peak: 0.0,
            chart_max: 1.0,
        };
    }
    let mut peak = 0.0f32;
    let mut sum = 0.0f32;
    let mut count = 0u32;
    for &v in data {
        if !v.is_finite() {
            continue;
        }
        let v = v.max(0.0);
        if v > peak {
            peak = v;
        }
        sum += v;
        count += 1;
    }
    let avg = if count > 0 { sum / count as f32 } else { 0.0 };
    let current = data
        .last()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| v.max(0.0))
        .unwrap_or(0.0);
    let chart_max = peak.max(1.0);
    LaneStats {
        current,
        avg,
        peak,
        chart_max,
    }
}

impl Widget for PowerPanel<'_> {
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

        let total_power = self.history.total_power.last().copied().unwrap_or(0.0);
        let sys_power = self.history.sys_power.last().copied().unwrap_or(0.0);
        let soc_power = self.history.combined_power.last().copied().unwrap_or(0.0);

        let mut title_spans = vec![
            Span::styled(" Power ", title_style),
            Span::styled("Total ", Style::default().fg(LABEL_COLOR)),
            Span::styled(
                format!("{:.2}W ", total_power),
                Style::default().fg(BRIGHT_TEXT).bold(),
            ),
            Span::styled("\u{2502} ", Style::default().fg(BORDER_NORMAL)),
            Span::styled("Sys ", Style::default().fg(LABEL_COLOR)),
            Span::styled(
                format!("{:.2}W ", sys_power),
                Style::default().fg(BRIGHT_TEXT),
            ),
        ];
        if soc_power > 0.0 {
            // SoC = combined_power from powermetrics (includes fabric/uncore
            // overhead that macmon's per-block totals miss).
            title_spans.push(Span::styled(
                "\u{2502} ",
                Style::default().fg(BORDER_NORMAL),
            ));
            title_spans.push(Span::styled("SoC ", Style::default().fg(LABEL_COLOR)));
            title_spans.push(Span::styled(
                format!("{:.2}W ", soc_power),
                Style::default().fg(BRIGHT_TEXT),
            ));
        }
        let title = Line::from(title_spans);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 4 || inner.width < 14 {
            return;
        }

        // Three lanes (CPU/GPU/ANE) — Apple Silicon doesn't expose DRAM
        // power via powermetrics, so the per-block readings from macmon are
        // the most granular signal we have.
        let lane = inner.height / 3;
        let cpu_h = lane.max(2);
        let gpu_h = lane.max(2);
        let ane_h = inner.height.saturating_sub(cpu_h + gpu_h).max(2);

        let lanes = Layout::vertical([
            Constraint::Length(cpu_h),
            Constraint::Length(gpu_h),
            Constraint::Length(ane_h),
        ])
        .split(inner);

        let cpu_data = self.history.cpu_power.to_vec();
        let gpu_data = self.history.gpu_power.to_vec();
        let ane_data = self.history.ane_power.to_vec();

        render_lane(
            lanes[0],
            buf,
            "CPU",
            &cpu_data,
            ChartMode::CenteredWave,
            &cpu_power_palette(),
            true,
        );
        render_lane(
            lanes[1],
            buf,
            "GPU",
            &gpu_data,
            ChartMode::FilledArea,
            &gpu_power_palette(),
            true,
        );
        render_lane(
            lanes[2],
            buf,
            "ANE",
            &ane_data,
            ChartMode::PulseTrail,
            &ane_power_palette(),
            false,
        );
    }
}

fn render_lane(
    area: Rect,
    buf: &mut Buffer,
    label: &str,
    data: &[f32],
    mode: ChartMode,
    palette: &GradientPalette,
    show_separator: bool,
) {
    if area.height == 0 || area.width < 12 {
        return;
    }

    let stats = lane_stats(data);

    // Header line with current/peak/avg.
    let header_rect = Rect::new(area.x, area.y, area.width, 1);
    let header = Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(LABEL_COLOR).bold(),
        ),
        Span::styled(
            format!("{:>5.2} W", stats.current),
            Style::default().fg(BRIGHT_TEXT).bold(),
        ),
        Span::styled("   peak ", Style::default().fg(DIM_TEXT)),
        Span::styled(
            format!("{:.2}W", stats.peak),
            Style::default().fg(LABEL_COLOR),
        ),
        Span::styled("   avg ", Style::default().fg(DIM_TEXT)),
        Span::styled(
            format!("{:.2}W", stats.avg),
            Style::default().fg(LABEL_COLOR),
        ),
    ]);
    header.render(header_rect, buf);

    if area.height < 2 {
        return;
    }

    // Optional faint separator below the chart so lanes are visually
    // distinct without spending another row on a heavy border.
    let chart_h = if show_separator {
        area.height.saturating_sub(2).max(1)
    } else {
        area.height.saturating_sub(1).max(1)
    };

    let chart_area = Rect::new(area.x, area.y + 1, area.width, chart_h);
    BrailleChart::new(data, mode)
        .max(stats.chart_max)
        .palette(palette)
        .render(chart_area, buf);

    if show_separator {
        let sep_y = area.y + 1 + chart_h;
        if sep_y < area.y + area.height {
            let sep_rect = Rect::new(area.x, sep_y, area.width, 1);
            // Thin dim divider made of box-drawing dashes.
            let mut text = String::with_capacity(area.width as usize);
            for _ in 0..area.width {
                text.push('\u{2500}');
            }
            Line::from(Span::styled(text, Style::default().fg(BORDER_NORMAL)))
                .render(sep_rect, buf);
        }
    }
}
