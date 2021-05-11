use rd_interface::{Registry, Result};

mod builtin;
// mod http;
mod redir;
mod socks5;

pub fn init(registry: &mut Registry) -> Result<()> {
    builtin::init(registry)?;
    // http::init(registry)?;
    redir::init(registry)?;
    socks5::init(registry)?;
    Ok(())
}
