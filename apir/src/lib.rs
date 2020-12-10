//! APiR(Async Proxy in Rust)
//!
//! Aimed to be the standard between proxy softwares written in Rust.

mod channel;
mod integrations;
pub mod traits;

#[cfg(feature = "tokio")]
pub use integrations::tokio::Tokio;

#[cfg(feature = "async-std")]
pub use integrations::async_std::AsyncStd;

pub use channel::{channel, Endpoint};
