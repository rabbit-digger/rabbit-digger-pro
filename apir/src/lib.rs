//! APiR(Async Proxy in Rust)
//!
//! Aimed to be the standard between proxy softwares written in Rust.

pub mod dynamic;
mod integrations;
pub mod traits;
mod virtual_host;

#[cfg(feature = "tokio")]
pub use integrations::tokio::Tokio;
#[cfg(feature = "tokio")]
pub use integrations::tokio::Tokio as ActiveRT;

#[cfg(feature = "use_async_std")]
pub use integrations::async_std::AsyncStd;
#[cfg(feature = "use_async_std")]
pub use integrations::async_std::AsyncStd as ActiveRT;

pub use virtual_host::VirtualHost;

pub mod prelude {
    pub use crate::traits::*;
}

#[cfg(test)]
pub mod tests;
