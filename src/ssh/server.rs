use crate::config::GameMetadata;
use crate::ssh::client::AppClient;
use crate::AppResult;
use rand::RngExt;
use russh::keys::ssh_key::private::{Ed25519Keypair, Ed25519PrivateKey, KeypairData};
use russh::server::{Config, Server};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

const KEY_PATH: &str = "./keys";

fn save_keys(signing_key: &russh::keys::PrivateKey) -> AppResult<()> {
    let mut buffer = std::io::BufWriter::new(File::create(KEY_PATH)?);
    buffer.write_all(&signing_key.to_bytes()?)?;
    log::info!("Created new keypair for SSH server.");
    Ok(())
}

fn load_keys() -> AppResult<russh::keys::PrivateKey> {
    let bytes = std::fs::read(KEY_PATH)?;
    let key = russh::keys::PrivateKey::from_bytes(&bytes)?;
    log::info!("Loaded keypair for SSH server.");
    Ok(key)
}

pub struct AppServer {
    port: u16,
    games: Arc<Vec<GameMetadata>>,
}

impl AppServer {
    pub fn new(port: u16, games: Vec<GameMetadata>) -> Self {
        Self {
            port,
            games: Arc::new(games),
        }
    }

    pub async fn run(&mut self) -> AppResult<()> {
        log::info!(
            "Starting SSH hub on port {}. Press Ctrl-C to exit.",
            self.port
        );

        let private_key = load_keys().unwrap_or_else(|_| {
            let seed: [u8; Ed25519PrivateKey::BYTE_SIZE] = rand::rng().random();
            let key_data = KeypairData::from(Ed25519Keypair::from_seed(&seed));
            let key = russh::keys::PrivateKey::new(key_data, "sshhub ssh server key")
                .expect("Failed to generate SSH keys");
            save_keys(&key).expect("Failed to save SSH keys");
            key
        });

        let config = Config {
            inactivity_timeout: Some(Duration::from_secs(3600)),
            auth_rejection_time: Duration::from_secs(3),
            auth_rejection_time_initial: Some(Duration::from_secs(0)),
            keys: vec![private_key],
            ..Default::default()
        };

        self.run_on_address(Arc::new(config), ("0.0.0.0", self.port))
            .await?;
        Ok(())
    }
}

impl Server for AppServer {
    type Handler = AppClient;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> AppClient {
        AppClient::new(self.games.clone())
    }
}
