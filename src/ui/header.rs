use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{BRIGHT_TEXT, DIM_TEXT, HEADER_BG, TITLE_COLOR};

pub struct Header<'a> {
    chip_name: &'a str,
    memory_gb: u8,
    ecpu_cores: u8,
    ecpu_label: &'a str,
    pcpu_cores: u8,
    pcpu_label: &'a str,
    gpu_cores: u8,
}

impl<'a> Header<'a> {
    pub fn new(
        chip_name: &'a str,
        memory_gb: u8,
        ecpu_cores: u8,
        ecpu_label: &'a str,
        pcpu_cores: u8,
        pcpu_label: &'a str,
        gpu_cores: u8,
    ) -> Self {
        Self {
            chip_name,
            memory_gb,
            ecpu_cores,
            ecpu_label,
            pcpu_cores,
            pcpu_label,
            gpu_cores,
        }
    }
}

fn local_time_string() -> String {
    // Use libc to get local time
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now);
        let tm = libc::localtime(&now);
        if tm.is_null() {
            return "??:??:??".to_string();
        }
        let tm = &*tm;
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

impl Widget for Header<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background
        for x in area.x..area.x + area.width {
            for y in area.y..area.y + area.height {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(Style::default().bg(HEADER_BG));
                }
            }
        }

        let cols = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(area);

        // Left: chip info
        let chip_info = format!(
            " {} \u{2502} {}GB \u{2502} {}{}+{}{} \u{2502} {}GPU",
            self.chip_name,
            self.memory_gb,
            self.ecpu_cores,
            self.ecpu_label,
            self.pcpu_cores,
            self.pcpu_label,
            self.gpu_cores
        );
        let left = Line::from(Span::styled(chip_info, Style::default().fg(BRIGHT_TEXT)));
        left.render(cols[0], buf);

        // Center: branding
        let brand = Line::from(vec![
            Span::styled("\u{25c6} ", Style::default().fg(BORDER_SELECTED)),
            Span::styled("basi", Style::default().fg(TITLE_COLOR)),
            Span::styled("top", Style::default().fg(DIM_TEXT)),
            Span::styled(" \u{25c6}", Style::default().fg(BORDER_SELECTED)),
        ])
        .centered();
        brand.render(cols[1], buf);

        // Right: time
        let time_str = format!("{} ", local_time_string());
        let right =
            Line::from(Span::styled(time_str, Style::default().fg(DIM_TEXT))).right_aligned();
        right.render(cols[2], buf);
    }
}

use crate::theme::BORDER_SELECTED;
