use std::io;
use thiserror::Error;

/// Errors in this crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Not implemented")]
    NotImplemented,
    #[error("Config error {0}")]
    Config(#[from] serde_json::Error),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub const NOT_IMPLEMENTED: Error = Error::NotImplemented;
