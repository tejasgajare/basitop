use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::theme::{
    self, BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, DIM_TEXT, LABEL_COLOR, TITLE_COLOR,
};
use crate::widgets::{BrailleChart, ChartMode};

/// Cycling palette so per-core sparklines stay visually distinct without
/// every row collapsing into the same color band.
const CORE_COLORS: [Color; 12] = [
    Color::Rgb(97, 214, 214),
    Color::Rgb(86, 182, 94),
    Color::Rgb(219, 170, 82),
    Color::Rgb(204, 102, 102),
    Color::Rgb(152, 118, 210),
    Color::Rgb(78, 154, 230),
    Color::Rgb(214, 130, 97),
    Color::Rgb(140, 210, 140),
    Color::Rgb(210, 150, 200),
    Color::Rgb(120, 200, 200),
    Color::Rgb(200, 200, 120),
    Color::Rgb(180, 140, 220),
];

pub struct CpuPanel<'a> {
    history: &'a MetricsHistory,
    ecpu_label: &'a str,
    pcpu_label: &'a str,
    ecpu_cores: u8,
    #[allow(dead_code)]
    pcpu_cores: u8,
    selected: bool,
}

impl<'a> CpuPanel<'a> {
    pub fn new(
        history: &'a MetricsHistory,
        ecpu_label: &'a str,
        pcpu_label: &'a str,
        ecpu_cores: u8,
        pcpu_cores: u8,
        selected: bool,
    ) -> Self {
        Self {
            history,
            ecpu_label,
            pcpu_label,
            ecpu_cores,
            pcpu_cores,
            selected,
        }
    }
}

impl Widget for CpuPanel<'_> {
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

        let cpu_pct = self.history.cpu_usage.last().copied().unwrap_or(0.0);
        let title = Line::from(vec![
            Span::styled(" CPU ", title_style),
            Span::styled(
                format!("{:.0}% ", cpu_pct),
                Style::default().fg(BRIGHT_TEXT).bold(),
            ),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        let core_count = self.history.core_count;
        let core_rows_needed = if core_count > 0 {
            // Cores split evenly across two columns; +1 for the cluster summary line.
            core_count.div_ceil(2) as u16 + 1
        } else {
            3
        };

        // Reserve enough vertical space for the centered-wave chart so it
        // can express both upper and lower halves; collapse cores if tight.
        let max_cores = inner.height.saturating_sub(3);
        let core_rows_needed = core_rows_needed.min(max_cores).max(1);
        let chart_h = inner.height.saturating_sub(core_rows_needed).max(2);

        let rows =
            Layout::vertical([Constraint::Length(chart_h), Constraint::Fill(1)]).split(inner);

        // Main CPU graph: green → red so load is instantly readable.
        let cpu_palette = theme::cpu_palette();
        let data = self.history.cpu_usage.to_vec();
        BrailleChart::new(&data, ChartMode::CenteredWave)
            .max(100.0)
            .palette(&cpu_palette)
            .render(rows[0], buf);

        if core_count > 0 {
            render_per_core(
                rows[1],
                buf,
                self.history,
                self.ecpu_label,
                self.pcpu_label,
                self.ecpu_cores,
            );
        } else {
            render_cluster_summary(
                rows[1],
                buf,
                self.history,
                self.ecpu_label,
                self.pcpu_label,
                &cpu_palette,
            );
        }
    }
}

/// Expand a macmon cluster letter to a human-readable prefix.
/// "E" (M1-M4 efficiency) → "Eff(E)", "P" (performance) → "Perf(P)",
/// "S" (M5 super-performance) → "Super(S)".
fn cluster_label(label: &str) -> String {
    match label {
        "E" => "Eff(E)".into(),
        "P" => "Perf(P)".into(),
        "S" => "Super(S)".into(),
        _ => label.into(),
    }
}

fn render_per_core(
    area: Rect,
    buf: &mut Buffer,
    history: &MetricsHistory,
    ecpu_label: &str,
    pcpu_label: &str,
    ecpu_cores: u8,
) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    let core_count = history.core_count;
    let ecpu_n = ecpu_cores as usize;

    let ecpu_freq = history.ecpu_freq.last().copied().unwrap_or(0);
    let pcpu_freq = history.pcpu_freq.last().copied().unwrap_or(0);
    let ecpu_pct = history.ecpu_usage.last().copied().unwrap_or(0.0);
    let pcpu_pct = history.pcpu_usage.last().copied().unwrap_or(0.0);

    let summary = Line::from(vec![
        Span::styled(
            format!(
                " {}: {:.0}% {}MHz",
                cluster_label(ecpu_label),
                ecpu_pct,
                ecpu_freq
            ),
            Style::default().fg(LABEL_COLOR),
        ),
        Span::styled(" \u{2502} ", Style::default().fg(BORDER_NORMAL)),
        Span::styled(
            format!(
                "{}: {:.0}% {}MHz",
                cluster_label(pcpu_label),
                pcpu_pct,
                pcpu_freq
            ),
            Style::default().fg(LABEL_COLOR),
        ),
    ]);
    summary.render(Rect::new(area.x, area.y, area.width, 1), buf);

    let remaining = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if remaining.height == 0 {
        return;
    }

    let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(remaining);

    // Split evenly by count: left = first half, right = second half.
    // Labels use cluster-relative indices so P9 stays "P9" and the first
    // S-core shows as "S0" even when it lands in the right column.
    let cores_per_col = core_count.div_ceil(2);

    for col_idx in 0..2usize {
        let col_area = cols[col_idx];
        let col_start = col_idx * cores_per_col;

        for row in 0..col_area.height as usize {
            let core_idx = col_start + row;
            if core_idx >= core_count {
                break;
            }

            // Cluster-relative label
            let (label_prefix, local_idx) = if core_idx < ecpu_n {
                (ecpu_label, core_idx)
            } else {
                (pcpu_label, core_idx - ecpu_n)
            };

            let y = col_area.y + row as u16;
            let usage = history
                .per_core_usage
                .get(core_idx)
                .and_then(|rb| rb.last().copied())
                .unwrap_or(0.0);
            let color = CORE_COLORS[core_idx % CORE_COLORS.len()];
            let label = format!("{:<4}", format!("{}{}", label_prefix, local_idx));
            let label_span = Span::styled(label, Style::default().fg(DIM_TEXT));
            Line::from(label_span).render(Rect::new(col_area.x, y, 4.min(col_area.width), 1), buf);

            let spark_start = col_area.x + 4;
            let pct_width: u16 = 5; // " XXX%"
            let spark_width = col_area.width.saturating_sub(4 + pct_width);

            if spark_width > 2 {
                let core_data = history
                    .per_core_usage
                    .get(core_idx)
                    .map(|rb| rb.to_vec())
                    .unwrap_or_default();
                BrailleChart::new(&core_data, ChartMode::Sparkline)
                    .max(100.0)
                    .color(color)
                    .render(Rect::new(spark_start, y, spark_width, 1), buf);
            }

            let pct_x = col_area.x + col_area.width - pct_width;
            let pct_color = if usage > 80.0 {
                Color::Rgb(230, 100, 100)
            } else if usage > 50.0 {
                Color::Rgb(220, 180, 80)
            } else {
                BRIGHT_TEXT
            };
            let pct_str = format!("{:>3.0}%", usage);
            Line::from(Span::styled(pct_str, Style::default().fg(pct_color)))
                .render(Rect::new(pct_x, y, pct_width, 1), buf);
        }
    }
}

fn render_cluster_summary(
    area: Rect,
    buf: &mut Buffer,
    history: &MetricsHistory,
    ecpu_label: &str,
    pcpu_label: &str,
    palette: &theme::GradientPalette,
) {
    if area.height < 2 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .split(area);

    let ecpu_pct = history.ecpu_usage.last().copied().unwrap_or(0.0);
    let ecpu_freq = history.ecpu_freq.last().copied().unwrap_or(0);
    crate::widgets::GradientGauge::new(
        ecpu_pct / 100.0,
        format!("{}-cores {:5.1}% @ {} MHz", ecpu_label, ecpu_pct, ecpu_freq),
        palette,
    )
    .render(rows[0], buf);

    let pcpu_pct = history.pcpu_usage.last().copied().unwrap_or(0.0);
    let pcpu_freq = history.pcpu_freq.last().copied().unwrap_or(0);
    crate::widgets::GradientGauge::new(
        pcpu_pct / 100.0,
        format!("{}-cores {:5.1}% @ {} MHz", pcpu_label, pcpu_pct, pcpu_freq),
        palette,
    )
    .render(rows[1], buf);
}
