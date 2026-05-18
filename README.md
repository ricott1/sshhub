# sshhub

SSH hub to pass on connection to other ssh games.

## Just try it out!

`ssh frittura.org`

## For game devs

The crate also exposes the SSH/ratatui scaffolding I use across my games as a library, behind the `core` feature. Add to your `Cargo.toml`:

```toml
sshhub = { git = "https://github.com/ricott1/sshhub", default-features = false }
```

and implement the `SshGame` trait:

```rust
use sshhub::core::{run_server, Credential, SshGame, SshSession};

struct MyGame { /* ... */ }

impl SshGame for MyGame {
    type Auth = ();
    const SCREEN_SIZE: (u16, u16) = (160, 50);
    const TITLE: &'static str = "My Game";
    const SERVER_INACTIVITY: Duration = Duration::from_secs(3600);

    async fn authenticate(&self, _: &str, _: Credential) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_session(self: Arc<Self>, session: SshSession<()>) {
        // drive a ratatui Tui on `session.writer`, consume `session.data_rx`...
    }
}
```

See [stonks](https://github.com/ricott1/stonks) for a real example with credential-based save lookup.

## License

GPLv3.
