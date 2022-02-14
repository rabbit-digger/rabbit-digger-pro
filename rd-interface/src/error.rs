use std::{
    error::Error as StdError,
    fmt::{self, Display},
    io,
};
use thiserror::Error;

pub struct ErrorWithContext {
    context: Box<dyn Display + Send + Sync + 'static>,
    error: Box<dyn StdError + Send + Sync + 'static>,
}

impl fmt::Debug for ErrorWithContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.context, self.error)
    }
}
impl ErrorWithContext {
    fn new<C, E>(context: C, error: E) -> ErrorWithContext
    where
        E: StdError + Send + Sync + 'static,
        C: Display + Send + Sync + 'static,
    {
        ErrorWithContext {
            context: Box::new(context),
            error: Box::new(error),
        }
    }
}

/// Errors in this crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error")]
    IO(#[from] io::Error),
    #[error("Not matched")]
    NotMatched,
    #[error("Not enabled in config")]
    NotEnabled,
    #[error("Not implemented")]
    NotImplemented,
    #[error("Config error: {0}")]
    Config(#[from] serde_json::Error),
    #[error("Aborted by user")]
    AbortedByUser,
    #[error("Context error: {0:?}")]
    Context(#[from] crate::context::Error),
    #[error("Not found")]
    NotFound(String),
    #[error("{0:?}")]
    Other(Box<dyn StdError + Send + Sync + 'static>),
    #[error("{0:?}")]
    WithContext(ErrorWithContext),
    #[error("Operation timeout: {0:?}")]
    Timeout(#[from] tokio::time::error::Elapsed),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;
pub const NOT_IMPLEMENTED: Error = Error::NotImplemented;
pub const NOT_ENABLED: Error = Error::NotEnabled;

pub fn map_other(e: impl StdError + Send + Sync + 'static) -> Error {
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
        matches!(self, Error::AbortedByUser)
    }
    pub fn is_addr_in_use(&self) -> bool {
        match self {
            Error::IO(e) => e.kind() == io::ErrorKind::AddrInUse,
            _ => false,
        }
    }
    pub fn other(string: impl Into<String>) -> Error {
        Error::Other(string.into().into())
    }
}

pub trait ErrorContext<T, E> {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static;
    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E> ErrorContext<T, E> for Result<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        self.map_err(|error| Error::WithContext(ErrorWithContext::new(context, error)))
    }
    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|error| Error::WithContext(ErrorWithContext::new(f(), error)))
    }
}
