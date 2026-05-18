use crate::config::GameMetadata;
use crate::ssh::bridge;
use crate::ssh::utils::convert_data_to_terminal_event;
use crate::ssh::SSHWriterProxy;
use crate::ssh::UserCredential;
use crate::tui::Tui;
use crate::AppResult;
use crate::TerminalEvent;
use crossterm::event::KeyCode;
use russh::server::Handle;
use russh::ChannelId;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{self, MissedTickBehavior};

/// All the per-session state we hand to the spawned task.
pub struct SessionInbound {
    pub channel_id: ChannelId,
    pub handle: Handle,
    pub username: String,
    pub credential: UserCredential,
    pub games: Arc<Vec<GameMetadata>>,
    pub term: String,
    pub initial_width: u32,
    pub initial_height: u32,
    pub data_rx: mpsc::Receiver<Vec<u8>>,
    pub resize_rx: mpsc::Receiver<(u32, u32)>,
}

const DRAW_TIME_STEP: Duration = Duration::from_millis(1000 / 30);

pub fn spawn_session(inbound: SessionInbound) {
    tokio::spawn(async move {
        if let Err(e) = run_session(inbound).await {
            log::error!("Session task error: {e}");
        }
    });
}

async fn run_session(mut inbound: SessionInbound) -> AppResult<()> {
    log::info!(
        "Session opened: user={} channel={}",
        inbound.username,
        inbound.channel_id
    );

    let SelectionOutcome { selected, mut data_rx, mut resize_rx, current_width, current_height } =
        run_lobby(&mut inbound).await?;

    let Some(game) = selected else {
        log::info!("User {} left the hub", inbound.username);
        let _ = inbound.handle.close(inbound.channel_id).await;
        return Ok(());
    };

    log::info!(
        "User {} selected '{}' -> bridging to {}:{}",
        inbound.username,
        game.key,
        game.host,
        game.port
    );

    let bridge_result = bridge::run(bridge::BridgeArgs {
        channel_id: inbound.channel_id,
        handle: inbound.handle.clone(),
        username: inbound.username.clone(),
        credential: inbound.credential.as_str().to_string(),
        game: game.clone(),
        term: inbound.term,
        width: current_width,
        height: current_height,
        data_rx: &mut data_rx,
        resize_rx: &mut resize_rx,
    })
    .await;

    if let Err(e) = bridge_result {
        log::warn!(
            "Bridge to {} ({}:{}) ended with error: {e}",
            game.key,
            game.host,
            game.port
        );
    }

    let _ = inbound.handle.close(inbound.channel_id).await;
    Ok(())
}

struct SelectionOutcome {
    selected: Option<GameMetadata>,
    data_rx: mpsc::Receiver<Vec<u8>>,
    resize_rx: mpsc::Receiver<(u32, u32)>,
    current_width: u32,
    current_height: u32,
}

/// Drive the lobby TUI until the user picks a game (returns `Some`) or quits
/// (returns `None`). Owns the `Tui` so it gets dropped (alt-screen cleanup)
/// before the caller transitions to the bridge.
async fn run_lobby(inbound: &mut SessionInbound) -> AppResult<SelectionOutcome> {
    let writer = SSHWriterProxy::new(inbound.channel_id, inbound.handle.clone());
    let mut tui = Tui::new(inbound.username.clone(), writer)?;

    let mut data_rx = std::mem::replace(&mut inbound.data_rx, mpsc::channel(1).1);
    let mut resize_rx = std::mem::replace(&mut inbound.resize_rx, mpsc::channel(1).1);

    let mut selected_idx: usize = 0;
    let mut dirty = true;
    let mut last_input_at = Instant::now();
    let kick_after = compute_lobby_kick(&inbound.games);

    let mut current_width = inbound.initial_width;
    let mut current_height = inbound.initial_height;

    let mut draw_ticker = time::interval(DRAW_TIME_STEP);
    draw_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut selected: Option<GameMetadata> = None;
    let mut quit = false;

    loop {
        if quit || selected.is_some() {
            break;
        }
        tokio::select! {
            biased;
            data = data_rx.recv() => {
                let Some(data) = data else { quit = true; continue; };
                let Some(event) = convert_data_to_terminal_event(&data) else { continue; };
                last_input_at = Instant::now();
                match event {
                    TerminalEvent::Key(key) => {
                        let n = inbound.games.len().max(1);
                        match key.code {
                            KeyCode::Esc => { quit = true; }
                            KeyCode::Up | KeyCode::Char('k') => {
                                selected_idx = (selected_idx + n - 1) % n;
                                dirty = true;
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                selected_idx = (selected_idx + 1) % n;
                                dirty = true;
                            }
                            KeyCode::Enter => {
                                if let Some(g) = inbound.games.get(selected_idx) {
                                    selected = Some(g.clone());
                                }
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                let idx = (c as u8 - b'1') as usize;
                                if idx < inbound.games.len() {
                                    selected = Some(inbound.games[idx].clone());
                                }
                            }
                            _ => {}
                        }
                    }
                    TerminalEvent::Resize(_, _) | TerminalEvent::Quit => {}
                }
            }
            change = resize_rx.recv() => {
                let Some((w, h)) = change else { quit = true; continue; };
                current_width = w;
                current_height = h;
                dirty = true;
            }
            _ = draw_ticker.tick() => {
                let warning = kick_warning_secs(last_input_at, Instant::now(), kick_after);
                if warning.is_none()
                    && Instant::now().saturating_duration_since(last_input_at) >= kick_after
                {
                    log::info!("Hub: kicking idle user {}", inbound.username);
                    quit = true;
                    continue;
                }
                if dirty || warning.is_some() {
                    let _ = tui.draw_lobby(&inbound.games, selected_idx, warning);
                    let _ = tui.push_data().await;
                    dirty = false;
                }
            }
        }
    }

    drop(tui);

    Ok(SelectionOutcome {
        selected,
        data_rx,
        resize_rx,
        current_width,
        current_height,
    })
}

/// Idle threshold the hub applies in its own lobby. Pick the min of the games'
/// own thresholds (so the hub is at least as patient as the strictest game),
/// floored at 30s, defaulted to 60s when the catalogue is empty.
fn compute_lobby_kick(games: &[GameMetadata]) -> Duration {
    let min_secs = games.iter().map(|g| g.inactivity_secs).min().unwrap_or(60);
    Duration::from_secs(min_secs.max(30))
}

const KICK_WARNING_REMAINING: Duration = Duration::from_secs(10);

fn kick_warning_secs(last_input_at: Instant, now: Instant, kick_after: Duration) -> Option<u32> {
    let elapsed = now.saturating_duration_since(last_input_at);
    if elapsed >= kick_after {
        return None;
    }
    let remaining = kick_after - elapsed;
    if remaining >= KICK_WARNING_REMAINING {
        return None;
    }
    let secs = remaining.as_secs() as u32 + u32::from(remaining.subsec_nanos() > 0);
    Some(secs)
}

