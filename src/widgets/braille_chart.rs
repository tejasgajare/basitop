//! Reusable Braille-based chart with multiple visual modes.
//!
//! All modes write directly into the `Buffer` (no `ratatui::widgets::Chart`).
//! Vertical resolution is `4 * area.height` thanks to the Braille dot
//! matrix.
//!
//! Each cell holds a *pair of consecutive samples* — left sub-column =
//! older, right sub-column = newer. Adjacent cells share a sample at
//! the seam (cell K's right and cell K+1's left both render the same
//! sample), so once a cell is rendered its content is fixed and the
//! chart slides left by exactly one full 2×4 block per new sample.
//! This keeps sub-column curve detail while avoiding the half-cell
//! flicker of plain sub-column granularity.
//!
//! Pick a mode that visually matches the metric type:
//! - [`ChartMode::FilledArea`]: dense filled area, btop-style. Good for
//!   utilization (CPU/GPU/memory/disk/network).
//! - [`ChartMode::FilledAreaInverted`]: same as [`ChartMode::FilledArea`]
//!   but bars hang from the top edge instead of growing from the bottom.
//!   Pair with FilledArea above to get a btop-style mirror chart that
//!   shares an implicit x-axis at the boundary.
//! - [`ChartMode::CenteredWave`]: symmetric fill above/below a midline.
//!   Distinct silhouette for power or load metrics.
//! - [`ChartMode::Sparkline`]: thin connected Braille line trail. Compact
//!   per-row history (per-core CPU, fan, battery sub-metrics).
//! - [`ChartMode::HeatStrip`]: color-driven block strip where intensity is
//!   the message; height variation is secondary.
//! - [`ChartMode::PulseTrail`]: sparse vertical pulses with a dotted
//!   baseline. Visually distinct from filled charts so power doesn't look
//!   like utilization.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use crate::theme::GradientPalette;
use crate::widgets::braille::BrailleCanvas;
use crate::widgets::gradient;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ChartMode {
    FilledArea,
    FilledAreaInverted,
    CenteredWave,
    Sparkline,
    HeatStrip,
    PulseTrail,
}

pub struct BrailleChart<'a> {
    data: &'a [f32],
    max: f32,
    palette: Option<&'a GradientPalette>,
    base_color: Option<Color>,
    mode: ChartMode,
}

impl<'a> BrailleChart<'a> {
    pub fn new(data: &'a [f32], mode: ChartMode) -> Self {
        Self {
            data,
            max: 100.0,
            palette: None,
            base_color: None,
            mode,
        }
    }

    pub fn max(mut self, max: f32) -> Self {
        if max.is_finite() && max > 0.0 {
            self.max = max;
        }
        self
    }

    pub fn palette(mut self, palette: &'a GradientPalette) -> Self {
        self.palette = Some(palette);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.base_color = Some(color);
        self
    }
}

#[inline]
fn safe(v: f32) -> f32 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

/// Pair-window slice. Each visible cell holds (data[pair_start + i],
/// data[pair_start + i + 1]) — older sample on the left, newer on the
/// right. Right-aligned so newest data sits at the rightmost cell.
/// Returns None if there isn't enough data for even one pair.
fn pair_window(len: usize, cw: usize) -> Option<(usize, usize, usize)> {
    if len < 2 || cw == 0 {
        return None;
    }
    let cells_to_show = cw.min(len - 1);
    let cell_offset = cw - cells_to_show;
    let pair_start = (len - 1) - cells_to_show;
    Some((cell_offset, pair_start, cells_to_show))
}

fn pick_color(chart: &BrailleChart<'_>, t: f64) -> Color {
    if let Some(c) = chart.base_color {
        return c;
    }
    if let Some(pal) = chart.palette {
        return pal.color_at(t.clamp(0.0, 1.0) as f32);
    }
    gradient::gradient_for_value(t)
}

impl Widget for BrailleChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.data.is_empty() {
            return;
        }
        match self.mode {
            ChartMode::FilledArea => render_filled_area(self, area, buf),
            ChartMode::FilledAreaInverted => render_filled_area_inverted(self, area, buf),
            ChartMode::CenteredWave => render_centered_wave(self, area, buf),
            ChartMode::Sparkline => render_sparkline(self, area, buf),
            ChartMode::HeatStrip => render_heat_strip(self, area, buf),
            ChartMode::PulseTrail => render_pulse_trail(self, area, buf),
        }
    }
}

fn render_filled_area(chart: BrailleChart<'_>, area: Rect, buf: &mut Buffer) {
    let cw = area.width as usize;
    let ch = area.height as usize;
    let total_dots_y = ch * 4;

    let Some((cell_offset, pair_start, cells_to_show)) = pair_window(chart.data.len(), cw) else {
        return;
    };

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    for i in 0..cells_to_show {
        let cell_x = cell_offset + i;
        for sub in 0..2 {
            let v = chart.data[pair_start + i + sub];
            let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
            // Floor at 1 so quiet/zero samples still draw a dot at the
            // axis. The chart reads as a continuous dotted baseline
            // (btop style) instead of vanishing into blank gaps.
            let height_dots = ((ratio * total_dots_y as f32).round() as usize).max(1);
            let dx = cell_x * 2 + sub;
            for k in 0..height_dots {
                let dy = total_dots_y - 1 - k;
                canvas.set_dot(dx, dy);
            }
        }
    }

    // Vertical gradient: top of cell maps to higher intensity color.
    canvas.paint_cells(|_cx, cy| {
        let top_intensity = 1.0 - cy as f64 / ch.max(1) as f64;
        pick_color(&chart, top_intensity)
    });

    canvas.render(area, buf);
}

/// Mirror of `render_filled_area`: bars hang from the top edge downward,
/// so when you stack one above a regular FilledArea you get a btop-style
/// symmetric chart sharing an implicit x-axis at the seam.
fn render_filled_area_inverted(chart: BrailleChart<'_>, area: Rect, buf: &mut Buffer) {
    let cw = area.width as usize;
    let ch = area.height as usize;
    let total_dots_y = ch * 4;

    let Some((cell_offset, pair_start, cells_to_show)) = pair_window(chart.data.len(), cw) else {
        return;
    };

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    for i in 0..cells_to_show {
        let cell_x = cell_offset + i;
        for sub in 0..2 {
            let v = chart.data[pair_start + i + sub];
            let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
            // Floor at 1 — see render_filled_area for the rationale.
            let height_dots = ((ratio * total_dots_y as f32).round() as usize).max(1);
            let dx = cell_x * 2 + sub;
            for k in 0..height_dots {
                // Fill from the top (the shared x-axis) downward.
                canvas.set_dot(dx, k);
            }
        }
    }

    // Mirror gradient: dim at the top (axis side) and bright at the
    // bottom (extreme), so when stacked under a regular FilledArea
    // (bright at top/extreme, dim at bottom/axis) the full mirror chart
    // reads as "dim at the shared axis, bright at the outer edges".
    canvas.paint_cells(|_cx, cy| {
        let intensity = cy as f64 / ch.max(1) as f64;
        pick_color(&chart, intensity)
    });

    canvas.render(area, buf);
}

fn render_centered_wave(chart: BrailleChart<'_>, area: Rect, buf: &mut Buffer) {
    let cw = area.width as usize;
    let ch = area.height as usize;
    let total_dots_y = ch * 4;
    if total_dots_y == 0 {
        return;
    }
    // The symmetry axis sits BETWEEN dot rows (half-1) and half. Lighting
    // row `half` itself would bias the wave downward by a half-cell because
    // dot row `half` lives at the top of cell `half/4`, not at the visual
    // middle of the area.
    let half = total_dots_y / 2;

    let Some((cell_offset, pair_start, cells_to_show)) = pair_window(chart.data.len(), cw) else {
        return;
    };

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    for i in 0..cells_to_show {
        let cell_x = cell_offset + i;
        for sub in 0..2 {
            let v = chart.data[pair_start + i + sub];
            let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
            let amplitude = (ratio * half as f32).round() as usize;
            // Floor at 1 so the chart never visually disappears at zero.
            let amp = amplitude.max(1);
            let dx = cell_x * 2 + sub;
            for k in 1..=amp {
                if let Some(dy) = half.checked_sub(k) {
                    canvas.set_dot(dx, dy);
                }
                let dy_below = half + k - 1;
                if dy_below < total_dots_y {
                    canvas.set_dot(dx, dy_below);
                }
            }
        }
    }

    // Distance-from-center gradient: cool at midline, hot at the extremes.
    // The visual center sits at (ch - 1) / 2 (cell index space), and a
    // cell's distance to the boundary axis is |cy - (ch-1)/2|.
    canvas.paint_cells(|_cx, cy| {
        let center = (ch as f64 - 1.0) / 2.0;
        let half_h = (ch as f64 / 2.0).max(1.0);
        let intensity = ((cy as f64 - center).abs() / half_h).clamp(0.0, 1.0);
        pick_color(&chart, intensity)
    });

    canvas.render(area, buf);
}

fn render_sparkline(chart: BrailleChart<'_>, area: Rect, buf: &mut Buffer) {
    let cw = area.width as usize;
    let ch = area.height as usize;
    let total_dots_y = ch * 4;
    if total_dots_y == 0 {
        return;
    }
    let last_dot_y = total_dots_y - 1;

    let Some((cell_offset, pair_start, cells_to_show)) = pair_window(chart.data.len(), cw) else {
        return;
    };

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    let dy_for = |v: f32| -> usize {
        let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
        last_dot_y - (ratio * last_dot_y as f32).round() as usize
    };

    for i in 0..cells_to_show {
        let cell_x = cell_offset + i;
        let dy_left = dy_for(chart.data[pair_start + i]);
        let dy_right = dy_for(chart.data[pair_start + i + 1]);
        // Left sub-column: just the older sample's dot. Right
        // sub-column: vertical bridge from older to newer y so the
        // line stays continuous across the cell. Adjacent cells share
        // a sample at the seam (cell K's right == cell K+1's left)
        // so no inter-cell bridging is needed.
        canvas.set_dot(cell_x * 2, dy_left);
        let (lo, hi) = if dy_left < dy_right {
            (dy_left, dy_right)
        } else {
            (dy_right, dy_left)
        };
        for y in lo..=hi {
            canvas.set_dot(cell_x * 2 + 1, y);
        }
    }

    let last_ratio = chart.data.last().copied().map(safe).unwrap_or(0.0) / chart.max;
    let line_color = pick_color(&chart, last_ratio.clamp(0.0, 1.0) as f64);
    canvas.paint_cells(|_cx, _cy| line_color);

    canvas.render(area, buf);
}

fn render_heat_strip(chart: BrailleChart<'_>, area: Rect, buf: &mut Buffer) {
    let cw = area.width as usize;
    let ch = area.height as usize;
    if cw == 0 || ch == 0 {
        return;
    }

    let n = cw.min(chart.data.len());
    let start = chart.data.len().saturating_sub(n);
    let slice = &chart.data[start..];
    let offset = cw.saturating_sub(slice.len());

    let dim = Color::Rgb(35, 38, 55);

    for (i, &v) in slice.iter().enumerate() {
        let cx = area.x + (offset + i) as u16;
        let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
        let color = pick_color(&chart, ratio as f64);
        let filled_rows = (ratio * ch as f32).round() as usize;
        for cy in 0..ch {
            let y = area.y + (ch - 1 - cy) as u16;
            let lit = cy < filled_rows;
            let glyph = if lit { "\u{2588}" } else { "\u{2581}" };
            let cell_color = if lit { color } else { dim };
            if let Some(cell) = buf.cell_mut((cx, y)) {
                cell.set_symbol(glyph);
                cell.set_style(Style::default().fg(cell_color));
            }
        }
    }
}

fn render_pulse_trail(chart: BrailleChart<'_>, area: Rect, buf: &mut Buffer) {
    let cw = area.width as usize;
    let ch = area.height as usize;
    let total_dots_y = ch * 4;
    if total_dots_y == 0 || cw == 0 {
        return;
    }
    let last_dot_y = total_dots_y - 1;

    // One sample per *cell* (not per sub-column) yields a barcode-like
    // strip that is visually distinct from FilledArea.
    let n = cw.min(chart.data.len());
    let start = chart.data.len().saturating_sub(n);
    let slice = &chart.data[start..];
    let offset = cw.saturating_sub(slice.len());

    // Auto-scale to local peak so flat-near-zero series (ANE) still show
    // some shape; keep `chart.max` as a floor so quiet periods don't blow up.
    let local_max = slice
        .iter()
        .copied()
        .map(safe)
        .fold(chart.max, f32::max)
        .max(0.001);

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    // Sparse dotted baseline: every 4th sub-column on the bottom row.
    let dot_cols = cw * 2;
    let mut x = 0usize;
    while x < dot_cols {
        canvas.set_dot(x, last_dot_y);
        x += 4;
    }

    for (i, &v) in slice.iter().enumerate() {
        // Right sub-column gives each pulse a "tall thin" silhouette.
        let dx = (offset + i) * 2 + 1;
        let ratio = (safe(v) / local_max).clamp(0.0, 1.0);
        let height = (ratio * total_dots_y as f32).round() as usize;
        for k in 0..height {
            let dy = last_dot_y.saturating_sub(k);
            canvas.set_dot(dx, dy);
        }
    }

    canvas.paint_cells(|_cx, cy| {
        let top_intensity = 1.0 - cy as f64 / ch as f64;
        pick_color(&chart, top_intensity)
    });

    canvas.render(area, buf);
}
