//! APiR(Async Proxy in Rust)
//!
//! Aimed to be the standard between proxy softwares written in Rust.

mod dynamic;
mod traits;

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
