//! The PICO-8 screen in half-block pixels (`▀`: the foreground colour is
//! the top pixel, the background the bottom one). 128x128 pixels make
//! exactly 128 columns by 64 rows at 1:1; bigger windows get whole-number
//! zoom, smaller ones a nearest-pixel shrink so the game still fits.

use crate::pico::{Pico, W};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Widget};

const DIM: Color = Color::Rgb(0x5f, 0x57, 0x4f);

/// Cell height over cell width, from the terminal's reported pixel size;
/// 2.0 (square half-blocks) when the terminal doesn't say.
pub fn cell_aspect() -> f32 {
    match ratatui::crossterm::terminal::window_size() {
        Ok(w) if w.width > 0 && w.height > 0 && w.columns > 0 && w.rows > 0 => {
            let a = (w.height as f32 / w.rows as f32) / (w.width as f32 / w.columns as f32);
            if (1.0..4.0).contains(&a) {
                a
            } else {
                2.0
            }
        }
        _ => 2.0,
    }
}

/// Where the 128x128 screen lands in the terminal.
///
/// Sharp: every PICO-8 pixel is one column by one half-row (or a whole
/// multiple), which is exact but only square when the font's cells are 1:2.
/// Square (`square` true): widen the picture by the font's real cell shape
/// so pixels come out square; a few columns are then doubled. Used only when
/// it fits without dropping any pixel, else sharp.
pub fn screen_rect(area: Rect, square: bool, aspect: f32) -> Rect {
    let (w, h) = (area.width as usize, area.height as usize);
    if square && (aspect - 2.0).abs() > 0.02 {
        // the most rows (leaving the status line if possible) whose square
        // width fits; at least 64 rows and 128 columns so nothing is lost
        for avail in [h.saturating_sub(1), h] {
            let mut rows = avail;
            while rows >= W / 2 {
                let cols = (rows as f32 * aspect).round() as usize;
                if cols <= w && cols >= W {
                    return place(area, cols, rows);
                }
                rows -= 1;
            }
        }
    }
    // whole-number zoom when it fits, leaving a row for the status line
    let zoom = (w / W).min(h.saturating_sub(1) / (W / 2)).max((w / W).min(h / (W / 2)));
    let (cols, rows) = if zoom >= 1 {
        (W * zoom, W / 2 * zoom)
    } else {
        let f = (w as f32 / W as f32).min(h as f32 * 2.0 / W as f32);
        ((W as f32 * f) as usize, (W as f32 * f / 2.0) as usize)
    };
    place(area, cols, rows)
}

fn place(area: Rect, cols: usize, rows: usize) -> Rect {
    let (cols, rows) = (cols.max(1) as u16, rows.max(1) as u16);
    Rect::new(area.x + (area.width - cols) / 2, area.y + (area.height.saturating_sub(rows + 1)) / 2, cols, rows.min(area.height))
}

pub fn draw_screen(buf: &mut Buffer, area: Rect, e: &Pico) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let px = ((x - area.x) as usize * W / area.width as usize).min(W - 1);
            let row = (y - area.y) as usize * 2;
            let top = (row * W / (area.height as usize * 2)).min(W - 1);
            let bot = ((row + 1) * W / (area.height as usize * 2)).min(W - 1);
            let (t, b) = (e.rgb(px, top), e.rgb(px, bot));
            buf[(x, y)].set_char('▀').set_fg(Color::Rgb(t.0, t.1, t.2)).set_bg(Color::Rgb(b.0, b.1, b.2));
        }
    }
}

/// One dim line under the screen, if there is room for it.
pub fn draw_status(buf: &mut Buffer, full: Rect, screen: Rect, left: &str, right: &str) {
    let y = screen.bottom();
    if y >= full.bottom() {
        return;
    }
    let row = Rect::new(screen.x, y, screen.width, 1);
    Paragraph::new(Line::from(left.to_string())).style(Style::new().fg(DIM)).render(row, buf);
    Paragraph::new(Line::from(right.to_string())).style(Style::new().fg(DIM)).alignment(Alignment::Right).render(row, buf);
}

/// A boxed message in the middle of the screen.
pub fn draw_box(buf: &mut Buffer, over: Rect, title: &str, lines: &[String]) {
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(title.chars().count() + 2) as u16 + 4;
    let h = lines.len() as u16 + 2;
    let (w, h) = (w.min(over.width), h.min(over.height));
    let r = Rect::new(over.x + (over.width - w) / 2, over.y + (over.height - h) / 2, w, h);
    Clear.render(r, buf);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {title} "))
        .style(Style::new().fg(Color::Rgb(0xff, 0xf1, 0xe8)).bg(Color::Rgb(0x1d, 0x2b, 0x53)));
    // pad to one width so the lines centre as a block and stay aligned
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let text: Vec<Line> = lines.iter().map(|l| Line::from(format!("{l:<width$}"))).collect();
    Paragraph::new(text).alignment(Alignment::Center).block(block).render(r, buf);
}
