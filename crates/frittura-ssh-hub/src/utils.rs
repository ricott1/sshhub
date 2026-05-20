use crate::AppResult;
use anyhow::anyhow;
use image::{Pixel, RgbaImage};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::path::PathBuf;

pub const FRITTURA_YELLOW: Color = Color::Rgb(0xc1, 0xc5, 0x40);

pub fn store_path(filename: &str) -> AppResult<PathBuf> {
    let dirs = directories::ProjectDirs::from("org", "frittura", "sshhub")
        .ok_or_else(|| anyhow!("Failed to get directories"))?;
    let config_dir = dirs.config_dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir)?;
    }
    Ok(config_dir.join(filename))
}

pub fn img_to_lines<'a>(img: &RgbaImage) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = vec![];
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return lines;
    }

    const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
    let pixel = |x: u32, y: u32| -> [u8; 4] {
        if y < height {
            img.get_pixel(x, y).to_rgba().0
        } else {
            TRANSPARENT
        }
    };

    let row_pairs = height.div_ceil(2);
    for row in 0..row_pairs {
        let y = row * 2;
        let mut spans: Vec<Span> = Vec::with_capacity(width as usize);
        for x in 0..width {
            spans.push(half_block(pixel(x, y), pixel(x, y + 1)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn half_block<'a>(top: [u8; 4], btm: [u8; 4]) -> Span<'a> {
    match (top[3] > 0, btm[3] > 0) {
        (false, false) => Span::raw(" "),
        (true, false) => Span::styled(
            "▀",
            Style::default().fg(Color::Rgb(top[0], top[1], top[2])),
        ),
        (false, true) => Span::styled(
            "▄",
            Style::default().fg(Color::Rgb(btm[0], btm[1], btm[2])),
        ),
        (true, true) => Span::styled(
            "▀",
            Style::default()
                .fg(Color::Rgb(top[0], top[1], top[2]))
                .bg(Color::Rgb(btm[0], btm[1], btm[2])),
        ),
    }
}

pub const TITLE: [&str; 6] = [
    "███████╗██████╗ ██╗████████╗████████╗██╗   ██╗██████╗  █████╗     ███████╗███████╗██╗  ██╗██╗   ██╗██████╗ ",
    "██╔════╝██╔══██╗██║╚══██╔══╝╚══██╔══╝██║   ██║██╔══██╗██╔══██╗    ██╔════╝██╔════╝██║  ██║██║   ██║██╔══██╗",
    "█████╗  ██████╔╝██║   ██║      ██║   ██║   ██║██████╔╝███████║    ███████╗███████╗███████║██║   ██║██████╔╝",
    "██╔══╝  ██╔══██╗██║   ██║      ██║   ██║   ██║██╔══██╗██╔══██║    ╚════██║╚════██║██╔══██║██║   ██║██╔══██╗",
    "██║     ██║  ██║██║   ██║      ██║   ╚██████╔╝██║  ██║██║  ██║    ███████║███████║██║  ██║╚██████╔╝██████╔╝",
    "╚═╝     ╚═╝  ╚═╝╚═╝   ╚═╝      ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝    ╚══════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ",
];

pub const TITLE_HEIGHT: u16 = TITLE.len() as u16;

pub fn big_text<'a>(lines_in: &[&str], block: Color, accent: Color) -> Paragraph<'a> {
    let lines: Vec<Line> = lines_in
        .iter()
        .map(|line| {
            let spans: Vec<Span> = line
                .chars()
                .map(|c| {
                    if c == '█' {
                        Span::styled("█", Style::default().fg(block))
                    } else {
                        Span::styled(c.to_string(), Style::default().fg(accent))
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect();
    Paragraph::new(lines).centered()
}
