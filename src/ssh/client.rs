use crate::config::GameMetadata;
use crate::ssh::channel::AppChannel;
use crate::ssh::UserCredential;
use crate::AppResult;
use anyhow::{anyhow, Context};
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId, Pty};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AppClient {
    username: String,
    credential: Option<UserCredential>,
    games: Arc<Vec<GameMetadata>>,
    channels: HashMap<ChannelId, AppChannel>,
    /// Most recent PTY term name per channel, captured at `pty_request` so
    /// the session task can mirror it on the outbound side.
    pty_terms: HashMap<ChannelId, String>,
}

impl AppClient {
    pub fn new(games: Arc<Vec<GameMetadata>>) -> Self {
        Self {
            username: String::new(),
            credential: None,
            games,
            channels: HashMap::new(),
            pty_terms: HashMap::new(),
        }
    }

    fn channel_mut(&mut self, id: ChannelId) -> AppResult<&mut AppChannel> {
        self.channels
            .get_mut(&id)
            .with_context(|| format!("unknown channel: {id}"))
    }
}

impl server::Handler for AppClient {
    type Error = anyhow::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> AppResult<Auth> {
        self.username = user.to_string();
        self.credential = Some(UserCredential::Password(password.to_string()));
        Ok(Auth::Accept)
    }

    async fn auth_publickey(
        &mut self,
        user: &str,
        public_key: &russh::keys::PublicKey,
    ) -> AppResult<Auth> {
        self.username = user.to_string();
        // The string form is what rebels/stonks hash to identify the save.
        // The user already proved possession of the matching private key
        // via the inbound SSH transport, so it's safe to forward as
        // a "password" downstream.
        self.credential = Some(UserCredential::PublicKey(public_key.to_string()));
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> AppResult<bool> {
        let app_channel = AppChannel::new(
            self.username.clone(),
            self.credential.clone(),
            self.games.clone(),
        );
        if self.channels.insert(channel.id(), app_channel).is_some() {
            return Err(anyhow!("channel `{}` has been already opened", channel.id()));
        }
        Ok(true)
    }

    async fn channel_close(&mut self, id: ChannelId, _: &mut Session) -> AppResult<()> {
        self.channels.remove(&id);
        self.pty_terms.remove(&id);
        Ok(())
    }

    async fn data(&mut self, id: ChannelId, data: &[u8], _: &mut Session) -> AppResult<()> {
        self.channel_mut(id)?.data(data).await
    }

    async fn pty_request(
        &mut self,
        id: ChannelId,
        term: &str,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> AppResult<()> {
        let term_owned = term.to_string();
        self.pty_terms.insert(id, term_owned.clone());
        let handle = session.handle();
        self.channel_mut(id)?
            .pty_request(id, width, height, term_owned, handle)
            .await
    }

    async fn window_change_request(
        &mut self,
        id: ChannelId,
        width: u32,
        height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> AppResult<()> {
        self.channel_mut(id)?
            .window_change_request(width, height)
            .await
    }
}
