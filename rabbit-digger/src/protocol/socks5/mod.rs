mod auth;
mod client;
mod common;
mod server;

pub use auth::NoAuth;
pub use client::Socks5Client;
pub use server::Socks5Server;

#[cfg(test)]
mod tests {
    use crate::{
        protocol::socks5::{Socks5Client, Socks5Server},
        test::echo_server,
    };
    use apir::{prelude::*, ActiveRT, VirtualHost};
    use futures::prelude::*;

    #[tokio::test]
    async fn test_socks5() -> std::io::Result<()> {
        let virtual_host = VirtualHost::with_pr(ActiveRT);

        let echo = echo_server(virtual_host.clone(), "127.0.0.1:6666".parse().unwrap()).await?;
        let server = Socks5Server::new(virtual_host.clone(), virtual_host.clone());
        let client = Socks5Client::new(&virtual_host, "127.0.0.1:1234".parse().unwrap());

        virtual_host.spawn(
            server.serve_listener(
                virtual_host
                    .tcp_bind("0.0.0.0:1234".parse().unwrap())
                    .await?,
            ),
        );

        let mut socket = client
            .tcp_connect("127.0.0.1:6666".parse().unwrap())
            .await?;

        socket.write_all(b"hello world").await?;
        socket.close().await?;

        let mut buf = String::new();
        socket.read_to_string(&mut buf).await?;
        assert_eq!(buf, "hello world");

        echo.await?;

        Ok(())
    }
}
