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
    #[error("Aborted by user")]
    AbortedByUser,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub const NOT_IMPLEMENTED: Error = Error::NotImplemented;

impl From<Error> for io::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::IO(e) => e,
            e => io::Error::new(io::ErrorKind::Other, e),
        }
    }
}

impl Error {
    pub fn is_aborted(&self) -> bool {
        match self {
            Error::AbortedByUser => true,
            _ => false,
        }
    }
}
