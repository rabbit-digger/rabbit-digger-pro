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
    #[error("Context error: {0:?}")]
    Context(#[from] crate::context::Error),
    #[error("Not found")]
    NotFound(String),
    #[error("{0:?}")]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub const NOT_IMPLEMENTED: Error = Error::NotImplemented;

pub fn map_other(e: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::Other(e.into())
}

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
    pub fn is_addr_in_use(&self) -> bool {
        match self {
            Error::IO(e) => e.kind() == io::ErrorKind::AddrInUse,
            _ => false,
        }
    }
}
