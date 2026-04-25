//! Lightweight gradient helpers used by Braille charts.
//!
//! These are intentionally simple and self-contained so they can be used
//! without any palette object. Panels that want a richer palette can keep
//! using `theme::GradientPalette`; `BrailleChart` accepts either.

use ratatui::style::Color;

/// Linearly blend two RGB colors. Non-RGB colors are first projected to an
/// approximate RGB triple so blending always returns something sensible.
pub fn blend_rgb(a: Color, b: Color, t: f64) -> Color {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let (r1, g1, b1) = rgb_components(a);
    let (r2, g2, b2) = rgb_components(b);
    Color::Rgb(
        (r1 as f32 * (1.0 - t) + r2 as f32 * t) as u8,
        (g1 as f32 * (1.0 - t) + g2 as f32 * t) as u8,
        (b1 as f32 * (1.0 - t) + b2 as f32 * t) as u8,
    )
}

fn rgb_components(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (200, 60, 60),
        Color::Green => (60, 200, 60),
        Color::Yellow => (220, 200, 60),
        Color::Blue => (60, 110, 220),
        Color::Magenta => (200, 60, 200),
        Color::Cyan => (60, 200, 200),
        Color::Gray => (160, 160, 160),
        Color::DarkGray => (80, 80, 80),
        Color::LightRed => (240, 120, 120),
        Color::LightGreen => (120, 240, 120),
        Color::LightYellow => (240, 240, 120),
        Color::LightBlue => (120, 160, 240),
        Color::LightMagenta => (240, 120, 240),
        Color::LightCyan => (120, 240, 240),
        Color::White => (240, 240, 240),
        _ => (180, 180, 180),
    }
}

/// Cool→warm gradient for an intensity in `[0, 1]`:
/// 0.0 → cool teal, 0.5 → amber, 1.0 → red.
/// Stays readable on a dark terminal background.
pub fn gradient_for_value(t: f64) -> Color {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if t < 0.5 {
        blend_rgb(
            Color::Rgb(80, 220, 170), // cool teal
            Color::Rgb(230, 200, 90), // warm amber
            t / 0.5,
        )
    } else {
        blend_rgb(
            Color::Rgb(230, 200, 90),
            Color::Rgb(230, 80, 80), // hot red
            (t - 0.5) / 0.5,
        )
    }
}

/// Variant intended for vertical-position coloring of filled charts:
/// `y_ratio = 0` at the chart bottom, `y_ratio = 1` at the top.
/// Currently identical to [`gradient_for_value`] but kept distinct so a
/// caller can swap in a different ramp later without touching call sites.
#[allow(dead_code)]
pub fn gradient_for_y(y_ratio: f64) -> Color {
    gradient_for_value(y_ratio)
}
