use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Hsl {
    pub h: f32, // 0..360
    pub s: f32, // 0..1
    pub l: f32, // 0..1
}

impl Hsl {
    pub fn new(h: f32, s: f32, l: f32) -> Self {
        Self { h, s, l }
    }

    pub fn to_rgb(self) -> (u8, u8, u8) {
        let c = (1.0 - (2.0 * self.l - 1.0).abs()) * self.s;
        let h_prime = self.h / 60.0;
        let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
        let m = self.l - c / 2.0;

        let (r1, g1, b1) = if h_prime < 1.0 {
            (c, x, 0.0)
        } else if h_prime < 2.0 {
            (x, c, 0.0)
        } else if h_prime < 3.0 {
            (0.0, c, x)
        } else if h_prime < 4.0 {
            (0.0, x, c)
        } else if h_prime < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        (
            ((r1 + m) * 255.0) as u8,
            ((g1 + m) * 255.0) as u8,
            ((b1 + m) * 255.0) as u8,
        )
    }

    pub fn to_color(self) -> Color {
        let (r, g, b) = self.to_rgb();
        Color::Rgb(r, g, b)
    }
}

fn lerp_hue(h1: f32, h2: f32, t: f32) -> f32 {
    let mut diff = h2 - h1;
    if diff > 180.0 {
        diff -= 360.0;
    } else if diff < -180.0 {
        diff += 360.0;
    }
    (h1 + diff * t).rem_euclid(360.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[derive(Clone)]
pub struct GradientPalette {
    stops: Vec<(f32, Hsl)>,
}

impl GradientPalette {
    pub fn new(stops: Vec<(f32, Hsl)>) -> Self {
        Self { stops }
    }

    pub fn color_at(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);

        if self.stops.is_empty() {
            return Color::White;
        }
        if self.stops.len() == 1 {
            return self.stops[0].1.to_color();
        }

        // Find bracketing stops
        let mut lo = 0;
        let mut hi = self.stops.len() - 1;
        for (i, &(pos, _)) in self.stops.iter().enumerate() {
            if pos <= t {
                lo = i;
            }
            if pos >= t && i < hi {
                hi = i;
                break;
            }
        }

        if lo == hi {
            return self.stops[lo].1.to_color();
        }

        let (pos_lo, col_lo) = self.stops[lo];
        let (pos_hi, col_hi) = self.stops[hi];
        let local_t = if (pos_hi - pos_lo).abs() < f32::EPSILON {
            0.0
        } else {
            (t - pos_lo) / (pos_hi - pos_lo)
        };

        Hsl::new(
            lerp_hue(col_lo.h, col_hi.h, local_t),
            lerp(col_lo.s, col_hi.s, local_t),
            lerp(col_lo.l, col_hi.l, local_t),
        )
        .to_color()
    }
}

// Predefined palettes

/// Green -> Yellow -> Red (CPU utilization)
pub fn cpu_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(130.0, 0.75, 0.45)),
        (0.5, Hsl::new(80.0, 0.85, 0.48)),
        (0.75, Hsl::new(30.0, 0.90, 0.50)),
        (1.0, Hsl::new(0.0, 0.88, 0.50)),
    ])
}

/// Blue -> Cyan -> Green -> Yellow -> Red
pub fn thermal_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(210.0, 0.8, 0.55)),
        (0.25, Hsl::new(180.0, 0.85, 0.50)),
        (0.5, Hsl::new(120.0, 0.75, 0.45)),
        (0.75, Hsl::new(50.0, 0.90, 0.50)),
        (1.0, Hsl::new(0.0, 0.85, 0.50)),
    ])
}

/// Green -> Yellow -> Red
#[allow(dead_code)]
pub fn power_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(130.0, 0.70, 0.42)),
        (0.5, Hsl::new(55.0, 0.90, 0.48)),
        (1.0, Hsl::new(0.0, 0.80, 0.48)),
    ])
}

/// Cyan -> Purple
pub fn memory_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(190.0, 0.80, 0.50)),
        (1.0, Hsl::new(280.0, 0.70, 0.55)),
    ])
}

/// Blue -> Orange (for temperature)
pub fn temp_palette() -> GradientPalette {
    GradientPalette::new(vec![
        (0.0, Hsl::new(210.0, 0.70, 0.55)),
        (0.4, Hsl::new(50.0, 0.80, 0.50)),
        (1.0, Hsl::new(0.0, 0.90, 0.50)),
    ])
}

// UI colors
pub const BORDER_NORMAL: Color = Color::Rgb(60, 60, 80);
pub const BORDER_SELECTED: Color = Color::Rgb(80, 200, 220);
pub const TITLE_COLOR: Color = Color::Rgb(180, 200, 220);
pub const HEADER_BG: Color = Color::Rgb(20, 20, 35);
pub const DIM_TEXT: Color = Color::Rgb(100, 110, 130);
pub const BRIGHT_TEXT: Color = Color::Rgb(210, 220, 235);
pub const LABEL_COLOR: Color = Color::Rgb(140, 155, 175);
pub const GAUGE_BG: Color = Color::Rgb(30, 32, 45);
