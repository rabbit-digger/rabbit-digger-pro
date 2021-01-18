use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Not implemented")]
    NotImplemented,
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
