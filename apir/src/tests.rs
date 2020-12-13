use std::{io, net::SocketAddr};

use crate::prelude::*;
use futures::prelude::*;

pub async fn echo_server<PN: ProxyTcpListener + Spawn>(
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
