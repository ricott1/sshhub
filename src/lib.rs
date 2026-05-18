pub mod config;
pub mod ssh;
mod tui;
mod ui;
mod utils;

pub use utils::store_path;

pub type AppResult<T> = Result<T, anyhow::Error>;

#[derive(Clone, Copy, Debug)]
pub enum TerminalEvent {
    Key(crossterm::event::KeyEvent),
    Resize(u16, u16),
    Quit,
}
