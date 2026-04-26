//! Disk and network throughput panel. Both metrics come from
//! powermetrics; if it isn't available the lanes still render but show
//! a flat "no data" baseline.
//!
//! Each lane is a stacked pair (read/in on top, write/out on bottom)
//! sharing an implicit x-axis. Each side normalizes independently
//! against its own visible-window peak so a long-past spike never
//! squashes the current signal into invisibility.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::theme::{BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, LABEL_COLOR, TITLE_COLOR};
use crate::widgets::{BrailleChart, ChartMode};

const DISK_READ_COLOR: Color = Color::Rgb(97, 214, 214);
const DISK_WRITE_COLOR: Color = Color::Rgb(220, 180, 80);
const NET_DL_COLOR: Color = Color::Rgb(86, 182, 94);
const NET_UL_COLOR: Color = Color::Rgb(214, 130, 200);

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
            Side {
                tag: "R",
                data: &disk_r,
                color: DISK_READ_COLOR,
            },
            Side {
                tag: "W",
                data: &disk_w,
                color: DISK_WRITE_COLOR,
            },
            true,
        );
        render_pair(
            lanes[1],
            buf,
            "NET",
            Side {
                tag: "\u{2193}",
                data: &net_rx,
                color: NET_DL_COLOR,
            },
            Side {
                tag: "\u{2191}",
                data: &net_tx,
                color: NET_UL_COLOR,
            },
            false,
        );
    }
}

struct Side<'a> {
    tag: &'a str,
    data: &'a [f32],
    color: Color,
}

fn render_pair(
    area: Rect,
    buf: &mut Buffer,
    label: &str,
    a: Side<'_>,
    b: Side<'_>,
    show_separator: bool,
) {
    if area.height < 2 {
        return;
    }
    let cur_a = a.data.last().copied().unwrap_or(0.0).max(0.0);
    let cur_b = b.data.last().copied().unwrap_or(0.0).max(0.0);

    // Normalize each side against the peak of just the visible window
    // (2 samples per cell column thanks to the Braille sub-grid). Using
    // the full ring-buffer peak makes a single old spike collapse all
    // current samples to zero dots.
    let visible = (area.width as usize).saturating_mul(2).max(1);
    let visible_peak = |data: &[f32]| -> f32 {
        let start = data.len().saturating_sub(visible);
        data[start..]
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0f32, f32::max)
    };
    let peak_a = visible_peak(a.data);
    let peak_b = visible_peak(b.data);
    let max_a = peak_a.max(1024.0);
    let max_b = peak_b.max(1024.0);

    let header = Line::from(vec![
        Span::styled(
            format!(" {} ", label),
            Style::default().fg(LABEL_COLOR).bold(),
        ),
        Span::styled(format!("{} ", a.tag), Style::default().fg(a.color).bold()),
        Span::styled(fmt_bps(cur_a), Style::default().fg(BRIGHT_TEXT).bold()),
        Span::styled("    ", Style::default()),
        Span::styled(format!("{} ", b.tag), Style::default().fg(b.color).bold()),
        Span::styled(fmt_bps(cur_b), Style::default().fg(BRIGHT_TEXT).bold()),
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

    // btop-style mirror chart with each side independently normalized:
    // top half grows up from the seam, bottom half hangs down. Solid
    // per-direction colors make it instantly clear which lane is which.
    BrailleChart::new(a.data, ChartMode::FilledArea)
        .max(max_a)
        .color(a.color)
        .render(top_area, buf);
    BrailleChart::new(b.data, ChartMode::FilledAreaInverted)
        .max(max_b)
        .color(b.color)
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

/// Returns a fixed 10-char-wide string so values don't shift columns
/// when the magnitude crosses a unit boundary. The number gains/loses
/// decimal places as it grows so the unit column stays aligned.
fn fmt_bps(v: f32) -> String {
    const KB: f32 = 1024.0;
    const MB: f32 = 1024.0 * 1024.0;
    const GB: f32 = 1024.0 * 1024.0 * 1024.0;
    let (val, unit) = if v >= GB {
        (v / GB, "GB/s")
    } else if v >= MB {
        (v / MB, "MB/s")
    } else if v >= KB {
        (v / KB, "KB/s")
    } else {
        (v, "B/s ")
    };
    let body = if val >= 100.0 {
        format!("{:>5.0} {}", val, unit)
    } else if val >= 10.0 {
        format!("{:>5.1} {}", val, unit)
    } else {
        format!("{:>5.2} {}", val, unit)
    };
    format!("{:>10}", body)
}
