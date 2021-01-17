mod config;
mod core;
mod plugins;
pub mod protocol;

use crate::core::RuleSet;
use anyhow::Result;
use apir::traits::*;
use apir::ActiveRT;
use futures::prelude::*;
use plugins::load_plugins;
use protocol::socks5::{Socks5Client, Socks5Server};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let plugins = load_plugins()?;
    println!("plugins: {:?}", plugins);

    let rt = ActiveRT;

    let core = RuleSet::new();

    let server = Socks5Server::new(rt.clone(), ActiveRT);
    let client = Socks5Client::new(&rt, "127.0.0.1:10801".parse().unwrap());
    ActiveRT.spawn(server.serve(1234));

    let mut socket = client.tcp_connect("93.184.216.34:80").await?;

    socket
        .write_all(
            b"GET / HTTP/1.1\r\nHost: example.com\r\nUser-Agent: curl/7.58.0\r\nAccept: */*\r\n\r\n",
        )
        .await?;
    rt.sleep(Duration::from_secs(1)).await;

    let mut buf = [0u8; 4096];
    let size = socket.read(&mut buf).await?;
    let s = std::str::from_utf8(&buf[..size]).unwrap();
    println!("Return {}", s);

    Ok(())
}

#[cfg(test)]
mod test {
    use apir::prelude::*;
    use futures::prelude::*;
    use std::{io, net::SocketAddr};

    pub async fn echo_server<PN: ProxyTcpListener + Runtime>(
        pn: PN,
        bind: SocketAddr,
    ) -> io::Result<RemoteHandle<io::Result<()>>>
    where
        PN::TcpListener: 'static,
        PN::TcpStream: 'static,
    {
        let listener = pn.tcp_bind(bind).await?;

        let handle = pn.spawn_handle(async move {
            let (socket, addr) = listener.accept().await?;
            println!("echo_server: accept from {}", addr);

            let (tx, mut rx) = socket.split();
            futures::io::copy(tx, &mut rx).await?;
            io::Result::Ok(())
        });

        Ok(handle)
    }
}
