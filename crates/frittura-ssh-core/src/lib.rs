//! SSH server runtime for ratatui-based terminal games.
//!
//! Implement the [`SshGame`] trait on your game type and pass it to
//! [`run_server`] - the runtime accepts SSH connections, authenticates them
//! via [`SshGame::authenticate`], allocates a PTY, and hands the game a
//! [`SshSession`] containing a ratatui-ready [`SshWriterProxy`] plus raw
//! `data_rx` / `resize_rx` receivers.
//!
//! Most games convert the raw receivers into a single
//! [`Receiver<TerminalEvent>`](TerminalEvent) by calling
//! [`spawn_event_converter`]; bridge-style consumers can read raw bytes
//! directly.
//!
//! Closing a session cleanly: own [`SshWriterProxy`], call
//! [`SshWriterProxy::send_and_close`] when you're done. The writer's `Drop`
//! is a panic-safe fallback that flushes a terminal-restore sequence and
//! closes the channel.
//!
//! ```ignore
//! use frittura_ssh_core::{run_server, Credential, SshGame, SshSession};
//! use std::{sync::Arc, time::Duration};
//!
//! struct MyGame;
//!
//! impl SshGame for MyGame {
//!     type Auth = ();
//!     const SCREEN_SIZE: (u16, u16) = (80, 24);
//!     const TITLE: &'static str = "MyGame";
//!     const SERVER_INACTIVITY: Duration = Duration::from_secs(60);
//!
//!     async fn authenticate(&self, _: &str, _: Credential) -> anyhow::Result<()> { Ok(()) }
//!     async fn on_session(self: Arc<Self>, _session: SshSession<()>) { /* ... */ }
//! }
//!
//! # async fn run() -> anyhow::Result<()> {
//! run_server(Arc::new(MyGame), 2222).await
//! # }
//! ```

pub(crate) mod channel;
pub(crate) mod client;
pub mod event;
pub mod idle;
pub mod input;
pub mod keys;
pub mod server;
pub mod trait_def;
pub mod writer;

pub use event::TerminalEvent;
pub use idle::kick_warning_secs;
pub use input::{convert_data_to_terminal_event, CMD_RESIZE};
pub use keys::load_or_generate;
pub use russh::keys::{HashAlg, PublicKey};
pub use server::run_server;
pub use trait_def::{spawn_event_converter, Credential, SshGame, SshSession};
pub use writer::SshWriterProxy;
