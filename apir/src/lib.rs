//! APiR(Async Proxy in Rust)
//!
//! Aimed to be the standard between proxy softwares written in Rust.
use async_trait::async_trait;
use futures::io::{AsyncRead, AsyncWrite};
use std::{io, net::Shutdown, net::SocketAddr, net::ToSocketAddrs};

mod dynamic;

pub enum TunnelType {
    Stream,
    Dgram,
}

pub enum Proxy {
    TCP,
    UDP,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
