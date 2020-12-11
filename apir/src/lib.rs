//! APiR(Async Proxy in Rust)
//!
//! Aimed to be the standard between proxy softwares written in Rust.

mod integrations;
pub mod traits;
mod virtual_host;

#[cfg(feature = "tokio")]
pub use integrations::tokio::Tokio;

#[cfg(feature = "async-std")]
pub use integrations::async_std::AsyncStd;

pub use virtual_host::VirtualHost;
