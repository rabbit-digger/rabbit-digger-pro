use std::{io, net::SocketAddr};

use crate::prelude::*;
use futures::prelude::*;

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

pub async fn echo_server_udp<PN: ProxyUdpSocket + Runtime>(
    pn: PN,
    bind: SocketAddr,
) -> io::Result<RemoteHandle<io::Result<()>>>
where
    PN::UdpSocket: 'static,
{
    let server = pn.udp_bind(bind).await?;

    let handle = pn.spawn_handle(async move {
        let mut buf = [0u8; 1024];
        let (size, addr) = server.recv_from(&mut buf).await?;
        server.send_to(&buf[..size], addr).await?;

        io::Result::Ok(())
    });

    Ok(handle)
}
