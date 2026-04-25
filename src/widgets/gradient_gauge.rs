use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::theme::{GradientPalette, GAUGE_BG};

const EIGHTHS: [&str; 9] = [
    " ", "\u{258f}", "\u{258e}", "\u{258d}", "\u{258c}", "\u{258b}", "\u{258a}", "\u{2589}",
    "\u{2588}",
];

pub struct GradientGauge<'a> {
    ratio: f32,
    label: String,
    palette: &'a GradientPalette,
}

impl<'a> GradientGauge<'a> {
    pub fn new(ratio: f32, label: String, palette: &'a GradientPalette) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
            label,
            palette,
        }
    }
}

impl Widget for GradientGauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let width = area.width as f32;
        let filled_width = self.ratio * width;
        let full_cols = filled_width as usize;
        let remainder = ((filled_width - full_cols as f32) * 8.0) as usize;

        for col in 0..area.width as usize {
            let x = area.x + col as u16;
            let y = area.y;
            let cell = buf.cell_mut((x, y)).unwrap();

            if col < full_cols {
                let t = col as f32 / width;
                let color = self.palette.color_at(t.min(self.ratio));
                cell.set_symbol(EIGHTHS[8]);
                cell.set_style(Style::default().fg(color).bg(GAUGE_BG));
            } else if col == full_cols && remainder > 0 {
                let color = self.palette.color_at(self.ratio);
                cell.set_symbol(EIGHTHS[remainder]);
                cell.set_style(Style::default().fg(color).bg(GAUGE_BG));
            } else {
                cell.set_symbol(" ");
                cell.set_style(Style::default().bg(GAUGE_BG));
            }
        }

        // Overlay the label centered
        if !self.label.is_empty() {
            let label_start = if self.label.len() < area.width as usize {
                (area.width as usize - self.label.len()) / 2
            } else {
                0
            };

            for (i, ch) in self.label.chars().enumerate() {
                let col = label_start + i;
                if col >= area.width as usize {
                    break;
                }
                let x = area.x + col as u16;
                let cell = buf.cell_mut((x, area.y)).unwrap();
                let is_filled = col < full_cols || (col == full_cols && remainder > 0);
                let fg = if is_filled {
                    ratatui::style::Color::Rgb(255, 255, 255)
                } else {
                    ratatui::style::Color::Rgb(180, 190, 210)
                };
                cell.set_char(ch);
                let bg = cell.style().bg.unwrap_or(GAUGE_BG);
                cell.set_style(Style::default().fg(fg).bg(bg));
            }
        }
    }
}
