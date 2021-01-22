mod config;
mod plugins;

use anyhow::Result;
use plugins::load_plugins;
use rd_interface::{config::Value, Arc, NoopNet};
use serde_json::json;

#[async_std::main]
async fn main() -> Result<()> {
    let registry = load_plugins()?;
    println!("registry: {:?}", registry);

    let local = registry.net.get("local");
    println!("local: {:?}", local.is_some());
    let local = local.unwrap()(Arc::new(NoopNet), Value::Null)?;

    let server = registry.server.get("socks5").unwrap()(
        local,
        json!({
            "address": "",
            "port": 12345,
        }),
    )
    .unwrap();
    server.start().await?;

    // let listener = local
    //     .tcp_bind("0.0.0.0:12345".parse::<SocketAddr>()?.into_address()?)
    //     .await?;
    // let (mut s, a) = listener.accept().await?;
    // println!("from {:?}", a);
    // s.write_all(b"hello world").await?;

    Ok(())
}
