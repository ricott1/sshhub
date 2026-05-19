use crossterm::event::{KeyEvent, MouseEvent};

/// Parsed input from the SSH client. Produced by
/// [`crate::spawn_event_converter`] from the raw `data_rx` / `resize_rx`
/// streams in an [`crate::SshSession`].
#[derive(Clone, Copy, Debug)]
pub enum TerminalEvent {
    /// A keypress decoded from the inbound byte stream.
    Key(KeyEvent),
    /// A mouse event decoded from SGR-encoded mouse reports (requires the
    /// game to have enabled mouse capture).
    Mouse(MouseEvent),
    /// Window-size change `(cols, rows)`.
    Resize(u16, u16),
    /// The client disconnected; both raw receivers have closed.
    Quit,
}
