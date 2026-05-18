use russh::server::Handle;
use russh::ChannelId;

/// Buffer of bytes destined for the SSH client. Flushed via `Handle::data`
/// when ratatui calls `flush()` at the end of a draw. Implements
/// `std::io::Write` so a ratatui crossterm backend can write into it.
#[derive(Clone)]
pub struct SSHWriterProxy {
    flushing: bool,
    channel_id: ChannelId,
    handle: Handle,
    sink: Vec<u8>,
}

impl std::fmt::Debug for SSHWriterProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SSHWriterProxy")
            .field("flushing", &self.flushing)
            .field("channel_id", &self.channel_id)
            .field("sink_len", &self.sink.len())
            .finish()
    }
}

impl SSHWriterProxy {
    pub fn new(channel_id: ChannelId, handle: Handle) -> Self {
        Self {
            flushing: false,
            channel_id,
            handle,
            sink: vec![],
        }
    }

    /// Drain the sink to the SSH client. No-op if `flush()` hasn't been
    /// called since the previous send.
    pub async fn send(&mut self) -> std::io::Result<usize> {
        if !self.flushing {
            return Ok(0);
        }
        let data_length = self.sink.len();
        if let Err(e) = self
            .handle
            .data(self.channel_id, std::mem::take(&mut self.sink))
            .await
        {
            log::error!("Flushing error: {e:?}");
            let _ = self.handle.close(self.channel_id).await;
        }
        self.flushing = false;
        Ok(data_length)
    }

    /// Hand the current sink off to a background task. Lets `Drop` impls
    /// (which can't await) still get the final alt-screen-cleanup bytes out
    /// before the channel closes. Callers that may re-create another `Tui`
    /// on the same channel (e.g. the hub re-entering the lobby after a
    /// recoverable bridge failure) should prefer this over the close
    /// variant below.
    pub fn send_in_background(&mut self) {
        let (handle, channel_id, data) = self.take_background_payload();
        if data.is_empty() {
            return;
        }
        tokio::spawn(async move {
            let _ = handle.data(channel_id, data).await;
        });
    }

    /// Like `send_in_background`, but the spawned task closes the SSH
    /// channel after the final flush (sending `eof` then `close`). Use this
    /// from a `Drop` impl whose firing means "the session is over" - a
    /// game whose `Tui` drop is the user leaving wants the channel to
    /// close so the ssh client doesn't hang until russh's
    /// `inactivity_timeout` fires.
    ///
    /// Bundling data + eof + close inside one `tokio::spawn` guarantees
    /// they're queued on the russh `Handle` in order.
    pub fn send_and_close_in_background(&mut self) {
        let (handle, channel_id, data) = self.take_background_payload();
        tokio::spawn(async move {
            if !data.is_empty() {
                let _ = handle.data(channel_id, data).await;
            }
            let _ = handle.eof(channel_id).await;
            let _ = handle.close(channel_id).await;
        });
    }

    fn take_background_payload(&mut self) -> (Handle, ChannelId, Vec<u8>) {
        let data = std::mem::take(&mut self.sink);
        self.flushing = false;
        (self.handle.clone(), self.channel_id, data)
    }
}

impl std::io::Write for SSHWriterProxy {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flushing = true;
        Ok(())
    }
}
