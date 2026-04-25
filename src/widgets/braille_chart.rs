//! Reusable Braille-based chart with multiple visual modes.
//!
//! All modes write directly into the `Buffer` (no `ratatui::widgets::Chart`).
//! The horizontal resolution is `2 * area.width` thanks to the 2-column
//! Braille matrix; the vertical resolution is `4 * area.height`.
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
    let dot_cols = cw * 2;

    // Right-align the most recent `dot_cols` samples so the chart slides
    // left by exactly one sub-column per new sample.
    let n = dot_cols.min(chart.data.len());
    let start = chart.data.len().saturating_sub(n);
    let slice = &chart.data[start..];
    let offset = dot_cols.saturating_sub(slice.len());

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    for (i, &v) in slice.iter().enumerate() {
        let dx = offset + i;
        let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
        let height_dots = (ratio * total_dots_y as f32).round() as usize;
        for k in 0..height_dots {
            let dy = total_dots_y - 1 - k;
            canvas.set_dot(dx, dy);
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
    let dot_cols = cw * 2;

    let n = dot_cols.min(chart.data.len());
    let start = chart.data.len().saturating_sub(n);
    let slice = &chart.data[start..];
    let offset = dot_cols.saturating_sub(slice.len());

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    for (i, &v) in slice.iter().enumerate() {
        let dx = offset + i;
        let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
        let height_dots = (ratio * total_dots_y as f32).round() as usize;
        for k in 0..height_dots {
            // Fill from the top (the shared x-axis) downward.
            canvas.set_dot(dx, k);
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
    let dot_cols = cw * 2;

    // One sample per Braille sub-column for full horizontal granularity
    // (matches btop). Right-aligned so the chart slides left as new samples
    // arrive — left edge stays blank until the ring buffer fills.
    let n = dot_cols.min(chart.data.len());
    let start = chart.data.len().saturating_sub(n);
    let slice = &chart.data[start..];
    let offset = dot_cols.saturating_sub(slice.len());

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    for (i, &v) in slice.iter().enumerate() {
        let dx = offset + i;
        let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
        let amplitude = (ratio * half as f32).round() as usize;
        // Floor at 1 so the chart never visually disappears at zero.
        let amp = amplitude.max(1);
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
    let dot_cols = cw * 2;

    let n = dot_cols.min(chart.data.len());
    let start = chart.data.len().saturating_sub(n);
    let slice = &chart.data[start..];
    let offset = dot_cols.saturating_sub(slice.len());

    let mut canvas = BrailleCanvas::new(area.width, area.height);

    let mut prev: Option<usize> = None;
    for (i, &v) in slice.iter().enumerate() {
        let dx = offset + i;
        let ratio = (safe(v) / chart.max).clamp(0.0, 1.0);
        let dy = last_dot_y - (ratio * last_dot_y as f32).round() as usize;
        // Bridge to the previous dot vertically so the line is continuous.
        if let Some(py) = prev {
            let (lo, hi) = if py < dy { (py, dy) } else { (dy, py) };
            for y in lo..=hi {
                canvas.set_dot(dx, y);
            }
        } else {
            canvas.set_dot(dx, dy);
        }
        prev = Some(dy);
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
