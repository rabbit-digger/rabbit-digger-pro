mod protocol;
use apir::traits::*;
use apir::{Tokio, VirtualHost};
use futures::prelude::*;
use protocol::socks5::{AuthMethod, Socks5Client, Socks5Server};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let virtual_host = VirtualHost::new();

    let server = Socks5Server::new(virtual_host.clone(), Tokio, AuthMethod::NoAuth);
    let client = Socks5Client::new(&virtual_host, "127.0.0.1:10801".parse().unwrap());
    Tokio.spawn(server.serve(1234));

    let mut socket = client
        .tcp_connect("39.156.69.79:80".parse().unwrap())
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
