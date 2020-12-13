use std::{io, net::SocketAddr};

use crate::prelude::*;
use futures::prelude::*;

pub async fn echo_server<PN: ProxyTcpListener>(pn: PN, bind: SocketAddr) -> io::Result<()>
where
    PN::TcpListener: 'static,
    PN::TcpStream: 'static,
{
    let listener = pn.tcp_bind(bind).await?;

    let (mut socket, addr) = listener.accept().await?;
    println!("accept from {}", addr);
    let mut buf = Vec::new();
    socket.read_to_end(&mut buf).await.unwrap();
    println!("server recv {:?}", buf);
    socket.write_all(&buf).await.unwrap();

    Ok(())
}
