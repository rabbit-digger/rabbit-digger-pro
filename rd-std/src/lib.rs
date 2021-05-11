use rd_interface::{Registry, Result};

mod http;
mod redir;
mod socks5;

pub fn init(registry: &mut Registry) -> Result<()> {
    http::init(registry)?;
    redir::init(registry)?;
    socks5::init(registry)?;
    Ok(())
}
