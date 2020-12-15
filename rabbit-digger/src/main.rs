pub mod protocol;
use apir::traits::*;
use apir::{ActiveRT, VirtualHost};
use futures::prelude::*;
use protocol::socks5::{Socks5Client, Socks5Server};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let virtual_host = VirtualHost::new();

    let server = Socks5Server::new(virtual_host.clone(), ActiveRT);
    let client = Socks5Client::new(&virtual_host, "127.0.0.1:1234".parse().unwrap());
    ActiveRT.spawn(server.serve(1234));

    let mut socket = client
        .tcp_connect("127.0.0.1:6666".parse().unwrap())
        .await?;

    socket
        .write_all(
            b"GET / HTTP/1.1\r\nHost: baidu.com\r\nUser-Agent: curl/7.58.0\r\nAccept: */*\r\n\r\n",
        )
        .await?;

    let mut buf = String::new();
    socket.read_to_string(&mut buf).await?;
    println!("Return {}", buf);

    Ok(())
}

#[cfg(test)]
mod test {
    use apir::prelude::*;
    use futures::prelude::*;
    use std::{io, net::SocketAddr};

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
}
