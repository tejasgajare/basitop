//! Compact thermal strip: CPU/GPU temperatures with a per-block gradient
//! heat bar (green → yellow → red, coloured block-by-block directly into
//! the Buffer) and a throttle indicator from powermetrics' thermal_pressure.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Widget};

use crate::metrics::MetricsHistory;
use crate::powermetrics::ThermalPressure;
use crate::theme::{
    self, GradientPalette, BORDER_NORMAL, BORDER_SELECTED, BRIGHT_TEXT, LABEL_COLOR, TITLE_COLOR,
};

const TEMP_MAX: f32 = 110.0;
// Dark background for unfilled blocks
const DIM_BAR: Color = Color::Rgb(38, 40, 55);

pub struct ThermalPanel<'a> {
    history: &'a MetricsHistory,
    selected: bool,
}

impl<'a> ThermalPanel<'a> {
    pub fn new(history: &'a MetricsHistory, selected: bool) -> Self {
        Self { history, selected }
    }
}

impl Widget for ThermalPanel<'_> {
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

        let pressure = self.history.thermal_pressure;

        // Title: ` Thermal ` + `Throttle: No/Yes` in the top-left corner.
        // Skip the throttle status entirely if powermetrics isn't available
        // (Unknown) — better to omit than show a meaningless placeholder.
        let mut title_spans = vec![Span::styled(" Thermal ", title_style)];
        if let Some((label, color)) = throttle_status(pressure) {
            title_spans.push(Span::styled("Throttle: ", Style::default().fg(LABEL_COLOR)));
            title_spans.push(Span::styled(
                format!("{} ", label),
                Style::default().fg(color).bold(),
            ));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Line::from(title_spans));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width < 14 {
            return;
        }

        let palette = theme::temp_palette();
        let cpu_temp = self.history.cpu_temp.last().copied().unwrap_or(0.0);
        let gpu_temp = self.history.gpu_temp.last().copied().unwrap_or(0.0);

        // Row 0: CPU, row 1: gap, row 2: GPU — the gap keeps the two bars
        // from reading as a single double-height block.
        render_row(
            Rect::new(inner.x, inner.y, inner.width, 1),
            buf,
            "CPU",
            cpu_temp,
            &palette,
        );
        let gpu_y = if inner.height >= 3 {
            inner.y + 2
        } else {
            inner.y + 1
        };
        if inner.height >= 2 {
            render_row(
                Rect::new(inner.x, gpu_y, inner.width, 1),
                buf,
                "GPU",
                gpu_temp,
                &palette,
            );
        }
    }
}

fn render_row(area: Rect, buf: &mut Buffer, label: &str, temp: f32, palette: &GradientPalette) {
    // Layout: " CPU " (5) + bar (fill) + " 35.6°C" (8 chars right-aligned).
    const PREFIX: u16 = 5;
    const SUFFIX: u16 = 8;

    let ratio = (temp / TEMP_MAX).clamp(0.0, 1.0);
    let temp_str = format!("{:>5.1}\u{00b0}C", temp);

    // Prefix: label only.
    Line::from(Span::styled(
        format!(" {:3} ", label),
        Style::default().fg(LABEL_COLOR).bold(),
    ))
    .render(Rect::new(area.x, area.y, PREFIX.min(area.width), 1), buf);

    // Suffix: temperature on the right.
    if area.width >= SUFFIX {
        let sx = area.x + area.width - SUFFIX;
        Line::from(Span::styled(
            temp_str,
            Style::default().fg(BRIGHT_TEXT).bold(),
        ))
        .render(Rect::new(sx, area.y, SUFFIX, 1), buf);
    }

    // Heat bar between prefix and suffix
    if area.width <= PREFIX + SUFFIX {
        return;
    }
    let bar_x = area.x + PREFIX;
    let bar_w = area.width - PREFIX - SUFFIX;
    let filled = (ratio * bar_w as f32).round() as u16;

    for i in 0..bar_w {
        let cx = bar_x + i;
        if let Some(cell) = buf.cell_mut((cx, area.y)) {
            if i < filled {
                // Colour each block by its position through the palette so
                // the bar shows a cool→hot gradient across its filled length.
                let t = i as f32 / bar_w.max(1) as f32;
                let color = palette.color_at(t);
                cell.set_symbol("\u{2588}");
                cell.set_style(Style::default().fg(color));
            } else {
                cell.set_symbol("\u{2591}");
                cell.set_style(Style::default().fg(DIM_BAR));
            }
        }
    }
}

/// Returns (label, color) for the throttle indicator, or `None` if we
/// don't have a usable pressure reading (powermetrics not running).
fn throttle_status(p: ThermalPressure) -> Option<(&'static str, Color)> {
    match p {
        ThermalPressure::Unknown => None,
        ThermalPressure::Nominal | ThermalPressure::Fair => Some(("No", Color::Rgb(120, 200, 140))),
        ThermalPressure::Serious | ThermalPressure::Critical => {
            Some(("Yes", Color::Rgb(230, 90, 90)))
        }
    }
}
