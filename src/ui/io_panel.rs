//! Disk and network throughput panel. Both metrics come from
//! powermetrics; if it isn't available the lanes still render but show
//! a flat "no data" baseline.
//!
//! Each lane is a stacked pair (read/in on top, write/out on bottom)
//! sharing the lane's local peak as the chart max so quiet periods
//! still produce visible shape.

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

fn disk_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(210.0, 0.65, 0.50)),
        (1.0, Hsl::new(190.0, 0.85, 0.60)),
    ])
}

fn net_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(160.0, 0.55, 0.50)),
        (1.0, Hsl::new(140.0, 0.85, 0.60)),
    ])
}

pub struct IoPanel<'a> {
    history: &'a MetricsHistory,
    selected: bool,
}

impl<'a> IoPanel<'a> {
    pub fn new(history: &'a MetricsHistory, selected: bool) -> Self {
        Self { history, selected }
    }
}

impl Widget for IoPanel<'_> {
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
            .title(Line::from(Span::styled(" I/O ", title_style)));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 4 || inner.width < 16 {
            return;
        }

        let half = inner.height / 2;
        let disk_h = half.max(2);
        let net_h = inner.height.saturating_sub(disk_h).max(2);
        let lanes =
            Layout::vertical([Constraint::Length(disk_h), Constraint::Length(net_h)]).split(inner);

        let disk_r = self.history.disk_read_bps.to_vec();
        let disk_w = self.history.disk_write_bps.to_vec();
        let net_rx = self.history.net_rx_bps.to_vec();
        let net_tx = self.history.net_tx_bps.to_vec();

        render_pair(
            lanes[0],
            buf,
            "DISK",
            ("R", &disk_r),
            ("W", &disk_w),
            &disk_palette(),
            true,
        );
        render_pair(
            lanes[1],
            buf,
            "NET",
            ("\u{2193}", &net_rx),
            ("\u{2191}", &net_tx),
            &net_palette(),
            false,
        );
    }
}

fn render_pair(
    area: Rect,
    buf: &mut Buffer,
    label: &str,
    a: (&str, &[f32]),
    b: (&str, &[f32]),
    palette: &GradientPalette,
    show_separator: bool,
) {
    if area.height < 2 {
        return;
    }
    let cur_a = a.1.last().copied().unwrap_or(0.0).max(0.0);
    let cur_b = b.1.last().copied().unwrap_or(0.0).max(0.0);
    let peak =
        a.1.iter()
            .chain(b.1.iter())
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0f32, f32::max);
    let chart_max = peak.max(1024.0);

    let header = Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(LABEL_COLOR).bold(),
        ),
        Span::styled(format!("{} ", a.0), Style::default().fg(DIM_TEXT)),
        Span::styled(
            format!("{:>9} ", fmt_bps(cur_a)),
            Style::default().fg(BRIGHT_TEXT).bold(),
        ),
        Span::styled(format!("  {} ", b.0), Style::default().fg(DIM_TEXT)),
        Span::styled(
            format!("{:>9}", fmt_bps(cur_b)),
            Style::default().fg(BRIGHT_TEXT).bold(),
        ),
        Span::styled("   peak ", Style::default().fg(DIM_TEXT)),
        Span::styled(fmt_bps(peak), Style::default().fg(LABEL_COLOR)),
    ]);
    header.render(Rect::new(area.x, area.y, area.width, 1), buf);

    let chart_total = if show_separator {
        area.height.saturating_sub(2)
    } else {
        area.height.saturating_sub(1)
    };
    if chart_total < 2 {
        return;
    }
    let top_h = chart_total / 2;
    let bot_h = chart_total - top_h;

    let top_area = Rect::new(area.x, area.y + 1, area.width, top_h);
    let bot_area = Rect::new(area.x, area.y + 1 + top_h, area.width, bot_h);

    // btop-style mirror chart: rx/read above the implicit x-axis, tx/write
    // hanging below it. Both halves share `chart_max` so amplitudes are
    // comparable across the seam.
    BrailleChart::new(a.1, ChartMode::FilledArea)
        .max(chart_max)
        .palette(palette)
        .render(top_area, buf);
    BrailleChart::new(b.1, ChartMode::FilledAreaInverted)
        .max(chart_max)
        .palette(palette)
        .render(bot_area, buf);

    if show_separator {
        let sep_y = area.y + 1 + chart_total;
        if sep_y < area.y + area.height {
            let mut text = String::with_capacity(area.width as usize);
            for _ in 0..area.width {
                text.push('\u{2500}');
            }
            Line::from(Span::styled(text, Style::default().fg(BORDER_NORMAL)))
                .render(Rect::new(area.x, sep_y, area.width, 1), buf);
        }
    }
}

fn fmt_bps(v: f32) -> String {
    let units = [("B/s", 1.0), ("KB/s", 1024.0), ("MB/s", 1024.0 * 1024.0)];
    if v >= 1024.0 * 1024.0 * 1024.0 {
        return format!("{:.2} GB/s", v / (1024.0 * 1024.0 * 1024.0));
    }
    let mut out = format!("{:.0} B/s", v);
    for (label, base) in units.iter().rev() {
        if v >= *base {
            out = format!("{:.2} {}", v / base, label);
            break;
        }
    }
    out
}
