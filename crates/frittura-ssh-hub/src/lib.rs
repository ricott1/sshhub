pub mod config;
pub mod ssh;
mod tui;
mod ui;
mod utils;

pub use utils::store_path;

pub type AppResult<T> = Result<T, anyhow::Error>;
