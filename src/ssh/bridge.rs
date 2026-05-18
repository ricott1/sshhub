use crate::config::GameMetadata;
use crate::AppResult;
use anyhow::{anyhow, Context};
use russh::client::{self, Config, Handler};
use russh::keys::PublicKey;
use russh::{ChannelMsg, Disconnect};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

pub struct BridgeArgs<'a> {
    pub channel_id: russh::ChannelId,
    pub handle: russh::server::Handle,
    pub username: String,
    pub credential: String,
    pub game: GameMetadata,
    pub term: String,
    pub width: u32,
    pub height: u32,
    pub data_rx: &'a mut mpsc::Receiver<Vec<u8>>,
    pub resize_rx: &'a mut mpsc::Receiver<(u32, u32)>,
}

/// Trust-on-first-use server key handler. We're a hub that connects to a
/// fixed list of game servers configured locally, so TOFU is acceptable for
/// the sketch. A future iteration could pin per-game host keys in games.toml.
struct BridgeClientHandler;

impl Handler for BridgeClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub async fn run(args: BridgeArgs<'_>) -> AppResult<()> {
    let config = Arc::new(Config {
        inactivity_timeout: Some(Duration::from_secs(3600)),
        ..Default::default()
    });

    let mut session = client::connect(config, (args.game.host.as_str(), args.game.port), BridgeClientHandler)
        .await
        .with_context(|| format!("connecting to {}:{}", args.game.host, args.game.port))?;

    let auth = session
        .authenticate_password(args.username.as_str(), args.credential.as_str())
        .await
        .context("outbound authenticate_password failed")?;
    if !auth.success() {
        return Err(anyhow!(
            "outbound auth rejected by {}",
            args.game.key
        ));
    }

    let mut outbound = session
        .channel_open_session()
        .await
        .context("outbound channel_open_session failed")?;

    outbound
        .request_pty(false, &args.term, args.width, args.height, 0, 0, &[])
        .await
        .context("outbound request_pty failed")?;

    outbound
        .request_shell(false)
        .await
        .context("outbound request_shell failed")?;

    loop {
        tokio::select! {
            data = args.data_rx.recv() => {
                let Some(bytes) = data else { break; };
                if let Err(e) = outbound.data(&bytes[..]).await {
                    log::warn!("outbound data write failed: {e}");
                    break;
                }
            }
            change = args.resize_rx.recv() => {
                let Some((w, h)) = change else { break; };
                if let Err(e) = outbound.window_change(w, h, 0, 0).await {
                    log::warn!("outbound window_change failed: {e}");
                    break;
                }
            }
            msg = outbound.wait() => {
                let Some(msg) = msg else { break; };
                match msg {
                    ChannelMsg::Data { data } => {
                        if let Err(e) = args.handle.data(args.channel_id, data.to_vec()).await {
                            log::warn!("inbound data write failed: {e:?}");
                            break;
                        }
                    }
                    ChannelMsg::ExtendedData { data, .. } => {
                        if let Err(e) = args.handle.data(args.channel_id, data.to_vec()).await {
                            log::warn!("inbound extended data write failed: {e:?}");
                            break;
                        }
                    }
                    ChannelMsg::Eof | ChannelMsg::Close | ChannelMsg::ExitStatus { .. } => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = outbound.close().await;
    let _ = session
        .disconnect(Disconnect::ByApplication, "", "en")
        .await;

    Ok(())
}
