use crate::utils::img_to_lines;
use crate::AppResult;
use anyhow::{anyhow, Context};
use image::{ImageBuffer, ImageReader, RgbaImage};
use log::warn;
use ratatui::text::Line;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Browsers historically clamp 0-delay GIF frames to ~100ms; we follow suit so
/// busted GIFs don't render at infinite FPS.
const DEFAULT_GIF_FRAME_MS: u64 = 100;

#[derive(Debug, Clone)]
pub struct AnimatedFrame {
    pub lines: Vec<Line<'static>>,
    pub delay: Duration,
}

#[derive(Debug, Clone)]
pub struct AnimatedPreview {
    pub frames: Vec<AnimatedFrame>,
    pub total: Duration,
}

#[derive(Debug, Clone)]
pub enum Preview {
    Static(Vec<Line<'static>>),
    Animated(AnimatedPreview),
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameMetadata {
    pub key: String,
    pub name: String,
    pub description: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(skip)]
    pub preview: Option<Preview>,
}

#[derive(Debug, Deserialize)]
struct TopLevel {
    games: Vec<GameMetadata>,
}

pub fn load_games(path: impl AsRef<Path>) -> AppResult<Vec<GameMetadata>> {
    let path = path.as_ref();
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("reading games config at {}", path.display()))?;
    let top: TopLevel = toml::from_str(&contents)
        .with_context(|| format!("parsing games config at {}", path.display()))?;
    let base = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(PathBuf::new);
    let mut games = top.games;
    for g in games.iter_mut() {
        let Some(rel) = g.image.as_deref() else { continue };
        let img_path = base.join(rel);
        let is_gif = img_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("gif"))
            .unwrap_or(false);
        let result = if is_gif {
            load_gif(&img_path).map(Preview::Animated)
        } else {
            load_static(&img_path).map(Preview::Static)
        };
        match result {
            Ok(p) => g.preview = Some(p),
            Err(e) => warn!(
                "skipping preview for {}: failed to load {}: {}",
                g.key,
                img_path.display(),
                e
            ),
        }
    }
    Ok(games)
}

fn load_static(path: &Path) -> AppResult<Vec<Line<'static>>> {
    let img = ImageReader::open(path)
        .with_context(|| format!("opening preview image {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("detecting format of {}", path.display()))?
        .decode()
        .with_context(|| format!("decoding {}", path.display()))?
        .into_rgba8();
    Ok(img_to_lines(&img))
}

fn load_gif(path: &Path) -> AppResult<AnimatedPreview> {
    let file = File::open(path).with_context(|| format!("opening gif {}", path.display()))?;
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(BufReader::new(file))
        .with_context(|| format!("reading gif header from {}", path.display()))?;

    let mut frames = Vec::new();
    while let Some(frame) = decoder
        .read_next_frame()
        .with_context(|| format!("decoding gif frame from {}", path.display()))?
    {
        let img: RgbaImage = ImageBuffer::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.buffer.to_vec(),
        )
        .ok_or_else(|| anyhow!("invalid gif frame in {}", path.display()))?;
        let delay_ms = u64::from(frame.delay) * 10;
        let delay = Duration::from_millis(if delay_ms == 0 {
            DEFAULT_GIF_FRAME_MS
        } else {
            delay_ms
        });
        frames.push(AnimatedFrame {
            lines: img_to_lines(&img),
            delay,
        });
    }
    if frames.is_empty() {
        return Err(anyhow!("gif has no frames: {}", path.display()));
    }
    let total: Duration = frames.iter().map(|f| f.delay).sum();
    Ok(AnimatedPreview { frames, total })
}
