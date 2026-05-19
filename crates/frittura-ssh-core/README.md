# frittura-ssh-core

Shared SSH server runtime for [ratatui](https://ratatui.rs/)-based terminal
games. Built on top of [russh](https://docs.rs/russh) and
[crossterm](https://docs.rs/crossterm). Implement one trait, get a working
multi-client SSH server that drives a ratatui TUI per connection.

## Usage

Implement [`SshGame`] on your game type and hand it to [`run_server`]:

```rust,ignore
use frittura_ssh_core::{run_server, Credential, SshGame, SshSession};
use std::{sync::Arc, time::Duration};

struct MyGame;

impl SshGame for MyGame {
    type Auth = ();
    const SCREEN_SIZE: (u16, u16) = (80, 24);
    const TITLE: &'static str = "MyGame";
    const SERVER_INACTIVITY: Duration = Duration::from_secs(60);

    async fn authenticate(&self, _u: &str, _c: Credential) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_session(self: Arc<Self>, session: SshSession<()>) {
        // Build a ratatui Terminal over session.writer, drive your TUI...
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_server(Arc::new(MyGame), 2222).await
}
```

For a typical `Receiver<TerminalEvent>` stream (keys, mouse, resizes, quit),
call `frittura_ssh_core::spawn_event_converter(session.data_rx, session.resize_rx)`
inside `on_session`.

## Examples in the wild

- [sshattrick](https://github.com/ricott1/sshattrick) — hockey
- [asterion](https://github.com/ricott1/asterion) — minotaur maze
- [stonks](https://github.com/ricott1/stonks) — market sim
- [frittura-ssh-hub](https://github.com/ricott1/frittura-ssh) — multi-game SSH lobby

## License

GPL-3.0-only.
