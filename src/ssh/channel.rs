use crate::config::GameMetadata;
use crate::ssh::session::{spawn_session, SessionInbound};
use crate::ssh::UserCredential;
use crate::AppResult;
use anyhow::anyhow;
use russh::server::Handle;
use russh::ChannelId;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Buffer of bytes destined for the SSH client. Flushed via `Handle::data`
/// when ratatui calls `flush()` at the end of a draw.
#[derive(Clone)]
pub struct SSHWriterProxy {
    flushing: bool,
    channel_id: ChannelId,
    handle: Handle,
    sink: Vec<u8>,
}

impl Debug for SSHWriterProxy {
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

    pub fn send_in_background(&mut self) {
        if self.sink.is_empty() {
            return;
        }
        let handle = self.handle.clone();
        let channel_id = self.channel_id;
        let data = std::mem::take(&mut self.sink);
        self.flushing = false;
        tokio::spawn(async move {
            let _ = handle.data(channel_id, data).await;
        });
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

#[derive(Debug)]
pub struct AppChannel {
    state: AppChannelState,
    username: String,
    credential: Option<UserCredential>,
    games: Arc<Vec<GameMetadata>>,
}

#[derive(Debug)]
enum AppChannelState {
    AwaitingPty,
    Ready {
        data_tx: mpsc::Sender<Vec<u8>>,
        resize_tx: mpsc::Sender<(u32, u32)>,
    },
}

impl AppChannel {
    pub fn new(
        username: String,
        credential: Option<UserCredential>,
        games: Arc<Vec<GameMetadata>>,
    ) -> Self {
        Self {
            state: AppChannelState::AwaitingPty,
            username,
            credential,
            games,
        }
    }

    pub async fn data(&mut self, data: &[u8]) -> AppResult<()> {
        let AppChannelState::Ready { data_tx, .. } = &self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };
        // Channel full or session task gone: treat as a disconnect.
        data_tx
            .send(data.to_vec())
            .await
            .map_err(|_| anyhow!("session task gone"))?;
        Ok(())
    }

    pub async fn pty_request(
        &mut self,
        id: ChannelId,
        width: u32,
        height: u32,
        term: String,
        handle: Handle,
    ) -> AppResult<()> {
        if !matches!(self.state, AppChannelState::AwaitingPty) {
            return Err(anyhow!("pty has been already allocated"));
        }
        let credential = self
            .credential
            .clone()
            .ok_or_else(|| anyhow!("session has no captured credential"))?;
        let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(64);
        let (resize_tx, resize_rx) = mpsc::channel::<(u32, u32)>(8);

        spawn_session(SessionInbound {
            channel_id: id,
            handle,
            username: self.username.clone(),
            credential,
            games: self.games.clone(),
            term,
            initial_width: width,
            initial_height: height,
            data_rx,
            resize_rx,
        });

        self.state = AppChannelState::Ready { data_tx, resize_tx };
        Ok(())
    }

    pub async fn window_change_request(&mut self, width: u32, height: u32) -> AppResult<()> {
        let AppChannelState::Ready { resize_tx, .. } = &self.state else {
            return Err(anyhow!("pty hasn't been allocated yet"));
        };
        resize_tx
            .send((width, height))
            .await
            .map_err(|_| anyhow!("session task gone"))?;
        Ok(())
    }
}
