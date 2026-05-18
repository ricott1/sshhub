mod bridge;
mod channel;
mod client;
mod server;
mod session;
mod utils;

pub use crate::ssh::channel::SSHWriterProxy;
pub use crate::ssh::server::AppServer;

/// What the user supplied at the front door, kept so we can replay it to the
/// upstream game. rebels/stonks hash the string to key save data — so even
/// when the user authenticated by public key, the string we forward is
/// `public_key.to_string()`, not the raw key.
#[derive(Debug, Clone)]
pub enum UserCredential {
    Password(String),
    PublicKey(String),
}

impl UserCredential {
    pub fn as_str(&self) -> &str {
        match self {
            UserCredential::Password(s) | UserCredential::PublicKey(s) => s,
        }
    }
}
