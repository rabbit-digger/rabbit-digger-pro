pub mod protocol;
use apir::traits::*;
use apir::{Tokio, VirtualHost};
use futures::prelude::*;
use protocol::socks5::{AuthMethod, Socks5Client, Socks5Server};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let virtual_host = VirtualHost::new();

    let server = Socks5Server::new(virtual_host.clone(), Tokio, AuthMethod::NoAuth);
    let client = Socks5Client::new(&virtual_host, "127.0.0.1:1234".parse().unwrap());
    Tokio.spawn(server.serve(1234));

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
    use futures::{io::copy, prelude::*};
    use std::{io, net::SocketAddr};

    pub struct Yield(bool);
    impl Future for Yield {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            if !self.0 {
                cx.waker().wake_by_ref();
                self.0 = true;
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        }
    }
    pub fn yield_now() -> Yield {
        Yield(false)
    }

    pub async fn echo_server<PN: ProxyTcpListener>(pn: PN, bind: SocketAddr) -> io::Result<()>
    where
        PN::TcpListener: 'static,
        PN::TcpStream: 'static,
    {
        let listener = pn.tcp_bind(bind).await?;

        let (socket, addr) = listener.accept().await?;
        println!("echo_server: accept from {}", addr);

        let (tx, mut rx) = socket.split();
        copy(tx, &mut rx).await?;

        Ok(())
    }
}
