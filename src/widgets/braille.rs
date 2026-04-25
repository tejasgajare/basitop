//! Braille canvas — direct buffer manipulation for high-resolution charts.
//!
//! Each terminal cell holds an 8-dot Braille glyph laid out as a 2×4 matrix:
//!
//! ```text
//!   col 0 | col 1
//!   ------+------
//!   dot 1 | dot 4    row 0
//!   dot 2 | dot 5    row 1
//!   dot 3 | dot 6    row 2
//!   dot 7 | dot 8    row 3
//! ```
//!
//! Unicode places the Braille block at U+2800. Each of the 8 dots toggles
//! a single bit; OR all the bits and add to 0x2800 to form the glyph.
//!
//! Bit values:
//! - dot 1 = 0x01    dot 4 = 0x08
//! - dot 2 = 0x02    dot 5 = 0x10
//! - dot 3 = 0x04    dot 6 = 0x20
//! - dot 7 = 0x40    dot 8 = 0x80
//!
//! Note that dots 7/8 are NOT contiguous with dots 1–6 in bit value: that
//! ordering predates the 8-dot extension and is what makes the lookup
//! table in [`DOT_BITS`] non-monotonic in the row dimension.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

/// Unicode block start for Braille glyphs.
pub const BRAILLE_BASE: u32 = 0x2800;

/// Braille dot-to-bit lookup, indexed as `DOT_BITS[col][row]`
/// where `col ∈ {0,1}` (left/right) and `row ∈ {0,1,2,3}` (top→bottom).
///
/// Left column carries dots 1, 2, 3, 7 ; right column carries dots 4, 5, 6, 8.
pub const DOT_BITS: [[u8; 4]; 2] = [
    // left column: rows 0..3 → dots 1, 2, 3, 7
    [0x01, 0x02, 0x04, 0x40],
    // right column: rows 0..3 → dots 4, 5, 6, 8
    [0x08, 0x10, 0x20, 0x80],
];

/// In-memory Braille canvas measured in *cells*. The implicit dot grid is
/// `(2 * width)` columns by `(4 * height)` rows.
///
/// Use [`set_dot`] to toggle dots and [`paint_cells`] / [`set_fg`] to color
/// individual cells, then call [`render`] to flush into a `ratatui::Buffer`.
pub struct BrailleCanvas {
    width: u16,
    height: u16,
    bits: Vec<u8>,
    fg: Vec<Option<Color>>,
}

impl BrailleCanvas {
    pub fn new(width: u16, height: u16) -> Self {
        let cells = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            bits: vec![0u8; cells],
            fg: vec![None; cells],
        }
    }

    #[inline]
    pub fn dot_width(&self) -> usize {
        self.width as usize * 2
    }

    #[inline]
    pub fn dot_height(&self) -> usize {
        self.height as usize * 4
    }

    /// Light up a single dot at dot-coordinates `(dx, dy)` where the origin
    /// is the top-left dot. Out-of-bounds calls are silently ignored.
    pub fn set_dot(&mut self, dx: usize, dy: usize) {
        if dx >= self.dot_width() || dy >= self.dot_height() {
            return;
        }
        let cell_x = dx / 2;
        let cell_y = dy / 4;
        let sub_x = dx & 1;
        let sub_y = dy & 3;
        let idx = cell_y * self.width as usize + cell_x;
        // OR the dot bit into the cell glyph (see DOT_BITS docs).
        self.bits[idx] |= DOT_BITS[sub_x][sub_y];
    }

    /// Set the foreground color of a single cell.
    #[allow(dead_code)]
    pub fn set_fg(&mut self, cell_x: usize, cell_y: usize, color: Color) {
        if cell_x >= self.width as usize || cell_y >= self.height as usize {
            return;
        }
        let idx = cell_y * self.width as usize + cell_x;
        self.fg[idx] = Some(color);
    }

    /// Paint every cell using a callback that receives `(cell_x, cell_y)`.
    pub fn paint_cells<F: FnMut(usize, usize) -> Color>(&mut self, mut f: F) {
        let w = self.width as usize;
        let h = self.height as usize;
        for cy in 0..h {
            for cx in 0..w {
                let idx = cy * w + cx;
                self.fg[idx] = Some(f(cx, cy));
            }
        }
    }

    /// Render this canvas into the buffer at `area`, clipped to the area's
    /// bounds. Cells with no dots and no color are left untouched.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let w = self.width.min(area.width) as usize;
        let h = self.height.min(area.height) as usize;
        for cy in 0..h {
            for cx in 0..w {
                let idx = cy * self.width as usize + cx;
                let bits = self.bits[idx];
                let fg = self.fg[idx];
                if bits == 0 && fg.is_none() {
                    continue;
                }
                // 0x2800 + bits is always within the Braille block (256 glyphs).
                let glyph = char::from_u32(BRAILLE_BASE + bits as u32).unwrap_or(' ');
                let x = area.x + cx as u16;
                let y = area.y + cy as u16;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(glyph);
                    if let Some(color) = fg {
                        cell.set_style(Style::default().fg(color));
                    }
                }
            }
        }
    }
}
