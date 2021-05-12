use rd_interface::{Registry, Result};

pub mod builtin;
// pub mod http;
pub mod redir;
pub mod socks5;

pub fn init(registry: &mut Registry) -> Result<()> {
    builtin::init(registry)?;
    // http::init(registry)?;
    redir::init(registry)?;
    socks5::init(registry)?;
    Ok(())
}
