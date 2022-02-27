use rd_interface::{Registry, Result};

pub mod builtin;
pub mod http;
pub mod mixed;
pub mod rule;
pub mod sniffer;
pub mod socks5;
pub mod tests;
pub mod transparent;
pub mod util;

pub fn init(registry: &mut Registry) -> Result<()> {
    builtin::init(registry)?;
    sniffer::init(registry)?;
    http::init(registry)?;
    mixed::init(registry)?;
    transparent::init(registry)?;
    rule::init(registry)?;
    socks5::init(registry)?;
    Ok(())
}

#[cfg(test)]
mod init_tests {
    use super::*;

    #[test]
    fn test_std_init() {
        let mut registry = Registry::new();
        init(&mut registry).unwrap();
    }
}
