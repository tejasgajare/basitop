//! btop-inspired memory panel: each line gets a label + absolute value,
//! followed by a percentage and a horizontal block bar. The history chart
//! sits between the RAM and Swap sections so the panel reads top-to-bottom
//! as: current state, history, swap.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::theme::{
    self, BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, DIM_TEXT, GAUGE_BG, LABEL_COLOR, TITLE_COLOR,
};
use crate::widgets::{BrailleChart, ChartMode};

const EIGHTHS: [&str; 9] = [
    " ", "\u{258f}", "\u{258e}", "\u{258d}", "\u{258c}", "\u{258b}", "\u{258a}", "\u{2589}",
    "\u{2588}",
];

pub struct MemoryPanel<'a> {
    history: &'a MetricsHistory,
    ram_total: u64,
    ram_usage: u64,
    swap_total: u64,
    swap_usage: u64,
    selected: bool,
}

impl<'a> MemoryPanel<'a> {
    pub fn new(
        history: &'a MetricsHistory,
        ram_total: u64,
        ram_usage: u64,
        swap_total: u64,
        swap_usage: u64,
        selected: bool,
    ) -> Self {
        Self {
            history,
            ram_total,
            ram_usage,
            swap_total,
            swap_usage,
            selected,
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{:.1} GB", gb)
    } else {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{:.0} MB", mb)
    }
}

impl Widget for MemoryPanel<'_> {
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
            .title(Line::from(Span::styled(" Memory ", title_style)));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 14 {
            return;
        }

        let ram_pct = self.history.ram_percent.last().copied().unwrap_or(0.0);
        let used_ratio = (ram_pct / 100.0).clamp(0.0, 1.0);
        let free = self.ram_total.saturating_sub(self.ram_usage);
        let free_ratio = if self.ram_total > 0 {
            free as f32 / self.ram_total as f32
        } else {
            0.0
        };

        // Used uses the green→red severity ramp (low=safe, high=hot). Free
        // is statically green so a healthy machine looks healthy at a glance.
        let used_color = theme::cpu_palette().color_at(used_ratio);
        let free_color = Color::Rgb(120, 200, 140);
        let swap_color = Color::Rgb(220, 180, 80);

        // Each metric block is 2 rows: label/value on top, pct+bar below.
        // Lay out conditionally based on available height so we never spill.
        let h = inner.height;
        let mut constraints: Vec<Constraint> = Vec::new();
        constraints.push(Constraint::Length(2)); // Used
        let want_free = h >= 4;
        let want_chart = h >= 7;
        let want_swap = h >= 6;
        if want_free {
            constraints.push(Constraint::Length(2));
        }
        if want_chart {
            constraints.push(Constraint::Fill(1));
        }
        if want_swap {
            constraints.push(Constraint::Length(2));
        }

        let rows = Layout::vertical(constraints).split(inner);
        let mut i = 0;

        render_metric(
            rows[i],
            buf,
            "Used",
            &format_bytes(self.ram_usage),
            used_ratio,
            used_color,
        );
        i += 1;

        if want_free {
            render_metric(
                rows[i],
                buf,
                "Free",
                &format_bytes(free),
                free_ratio,
                free_color,
            );
            i += 1;
        }

        if want_chart {
            let palette = theme::memory_palette();
            let data = self.history.ram_percent.to_vec();
            BrailleChart::new(&data, ChartMode::FilledArea)
                .max(100.0)
                .palette(&palette)
                .render(rows[i], buf);
            i += 1;
        }

        if want_swap {
            let swap_pct = self.history.swap_percent.last().copied().unwrap_or(0.0);
            let swap_value = format!(
                "{} / {}",
                format_bytes(self.swap_usage),
                format_bytes(self.swap_total),
            );
            render_metric(
                rows[i],
                buf,
                "Swap",
                &swap_value,
                (swap_pct / 100.0).clamp(0.0, 1.0),
                swap_color,
            );
        }
    }
}

/// Two-row metric block: ` Label              VALUE` over `  XX% ████░░░░`.
fn render_metric(area: Rect, buf: &mut Buffer, label: &str, value: &str, ratio: f32, color: Color) {
    if area.height < 2 || area.width < 12 {
        return;
    }

    // Row 0: label left, value right — built as a single Line with explicit
    // padding so the two halves can never desync alignment-wise.
    let label_str = format!(" {}", label);
    let value_str = format!("{} ", value);
    let used = label_str.chars().count() + value_str.chars().count();
    let pad = (area.width as usize).saturating_sub(used);
    let header = Line::from(vec![
        Span::styled(label_str, Style::default().fg(LABEL_COLOR).bold()),
        Span::raw(" ".repeat(pad)),
        Span::styled(value_str, Style::default().fg(BRIGHT_TEXT).bold()),
    ]);
    header.render(Rect::new(area.x, area.y, area.width, 1), buf);

    // Row 1: percentage on the left, then a flush-right bar.
    let pct = (ratio * 100.0).clamp(0.0, 100.0);
    let pct_str = format!("  {:>3.0}% ", pct);
    let pct_w = pct_str.chars().count() as u16;
    Line::from(Span::styled(pct_str, Style::default().fg(DIM_TEXT)))
        .render(Rect::new(area.x, area.y + 1, pct_w.min(area.width), 1), buf);

    if area.width <= pct_w + 1 {
        return;
    }
    // 1-col right padding so the bar doesn't kiss the panel border.
    let bar_x = area.x + pct_w;
    let bar_w = area.width - pct_w - 1;
    render_horizontal_bar(Rect::new(bar_x, area.y + 1, bar_w, 1), buf, ratio, color);
}

/// Sub-cell-precise horizontal block bar. Uses 1/8-block glyphs at the fill
/// edge so the bar advances smoothly even when the ratio is small.
fn render_horizontal_bar(area: Rect, buf: &mut Buffer, ratio: f32, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let ratio = ratio.clamp(0.0, 1.0);
    let filled_eighths = (ratio * area.width as f32 * 8.0).round() as u32;
    let full_cols = (filled_eighths / 8) as u16;
    let remainder = (filled_eighths % 8) as usize;

    for i in 0..area.width {
        let cx = area.x + i;
        let Some(cell) = buf.cell_mut((cx, area.y)) else {
            continue;
        };
        if i < full_cols {
            cell.set_symbol("\u{2588}");
            cell.set_style(Style::default().fg(color).bg(GAUGE_BG));
        } else if i == full_cols && remainder > 0 {
            cell.set_symbol(EIGHTHS[remainder]);
            cell.set_style(Style::default().fg(color).bg(GAUGE_BG));
        } else {
            cell.set_symbol(" ");
            cell.set_style(Style::default().bg(GAUGE_BG));
        }
    }
}
