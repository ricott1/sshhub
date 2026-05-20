use crate::client::AuthedSession;
use crate::trait_def::{SshGame, SshSession};
use crate::writer::SshWriterProxy;
use anyhow::anyhow;
use russh::server::Handle;
use russh::ChannelId;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct AppChannel<G: SshGame> {
    state: AppChannelState,
    authed: AuthedSession<G>,
    game: Arc<G>,
}

enum AppChannelState {
    AwaitingPty,
    /// PTY allocated; waiting for `shell_request` or `exec_request` to
    /// actually start the session.
    PtyReady {
        id: ChannelId,
        term: String,
        width: u32,
        height: u32,
        handle: Handle,
    },
    Ready {
        data_tx: mpsc::Sender<Vec<u8>>,
        resize_tx: mpsc::Sender<(u32, u32)>,
    },
}

impl<G: SshGame> AppChannel<G> {
    pub(crate) fn new(authed: AuthedSession<G>, game: Arc<G>) -> Self {
        Self {
            state: AppChannelState::AwaitingPty,
            authed,
            game,
        }
    }

    pub(crate) async fn data(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let AppChannelState::Ready { data_tx, .. } = &self.state else {
            return Err(anyhow!("session not started yet"));
        };
        data_tx
            .send(data.to_vec())
            .await
            .map_err(|_| anyhow!("session task gone"))?;
        Ok(())
    }

    pub(crate) async fn pty_request(
        &mut self,
        id: ChannelId,
        term: String,
        width: u32,
        height: u32,
        handle: Handle,
    ) -> anyhow::Result<()> {
        if !matches!(self.state, AppChannelState::AwaitingPty) {
            return Err(anyhow!("pty has been already allocated"));
        }
        self.state = AppChannelState::PtyReady {
            id,
            term,
            width,
            height,
            handle,
        };
        Ok(())
    }

    pub(crate) async fn shell_request(&mut self) -> anyhow::Result<()> {
        self.start_session(None).await
    }

    pub(crate) async fn exec_request(&mut self, command: String) -> anyhow::Result<()> {
        self.start_session(Some(command)).await
    }

    async fn start_session(&mut self, exec_command: Option<String>) -> anyhow::Result<()> {
        let state = std::mem::replace(&mut self.state, AppChannelState::AwaitingPty);
        let AppChannelState::PtyReady {
            id,
            term,
            width,
            height,
            handle,
        } = state
        else {
            self.state = state;
            return Err(anyhow!("shell/exec before pty allocation"));
        };

        let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(64);
        let (resize_tx, resize_rx) = mpsc::channel::<(u32, u32)>(8);

        let session = SshSession {
            username: self.authed.username.clone(),
            auth: self.authed.auth.clone(),
            term,
            writer: SshWriterProxy::new(id, handle.clone()),
            channel_id: id,
            handle,
            initial_size: (width, height),
            exec_command,
            data_rx,
            resize_rx,
        };

        let game = self.game.clone();
        tokio::spawn(async move {
            game.on_session(session).await;
        });

        self.state = AppChannelState::Ready { data_tx, resize_tx };
        Ok(())
    }

    pub(crate) async fn window_change_request(
        &mut self,
        width: u32,
        height: u32,
    ) -> anyhow::Result<()> {
        match &mut self.state {
            AppChannelState::Ready { resize_tx, .. } => resize_tx
                .send((width, height))
                .await
                .map_err(|_| anyhow!("session task gone")),
            AppChannelState::PtyReady {
                width: w,
                height: h,
                ..
            } => {
                *w = width;
                *h = height;
                Ok(())
            }
            AppChannelState::AwaitingPty => Err(anyhow!("pty hasn't been allocated yet")),
        }
    }
}
