pub use self::net::TestNet;
use crate::builtin;
use futures::{SinkExt, StreamExt};
use rd_interface::{Address, Bytes, Context, IntoAddress, Net, Registry};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};

mod channel;
mod net;

pub fn get_registry() -> Registry {
    let mut registry = Registry::new();
    builtin::init(&mut registry).unwrap();
    registry
}

pub async fn spawn_echo_server_udp(net: &Net, addr: impl IntoAddress) {
    let mut udp = net
        .udp_bind(&mut Context::new(), &addr.into_address().unwrap())
        .await
        .unwrap();
    tokio::spawn(async move {
        loop {
            let (buf, addr) = udp.next().await.unwrap().unwrap();
            udp.send((buf.freeze(), addr.into())).await.unwrap();
        }
    });
}

pub async fn assert_echo_udp(net: &Net, addr: impl IntoAddress) {
    let target_addr = addr.into_address().unwrap();
    let mut socket = net
        .udp_bind(&mut Context::new(), &"0.0.0.0:0".parse().unwrap())
        .await
        .unwrap();

    socket
        .send((Bytes::from_static(b"hello"), target_addr.clone()))
        .await
        .unwrap();
    let (buf, addr) = socket.next().await.unwrap().unwrap();

    assert_eq!(&buf[..], b"hello");
    assert_eq!(Address::SocketAddr(addr), target_addr);
}

pub async fn spawn_echo_server(net: &Net, addr: impl IntoAddress) {
    let listener = net
        .tcp_bind(&mut Context::new(), &addr.into_address().unwrap())
        .await
        .unwrap();
    tokio::spawn(async move {
        loop {
            let (tcp, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let (mut rx, mut tx) = io::split(tcp);
                io::copy(&mut rx, &mut tx).await.unwrap();
            });
        }
    });
}

pub async fn assert_echo(net: &Net, addr: impl IntoAddress) {
    const BUF: &'static [u8] = b"asdfasdfasdfasj12312313123";
    let mut tcp = net
        .tcp_connect(&mut Context::new(), &addr.into_address().unwrap())
        .await
        .unwrap();
    tcp.write_all(&BUF).await.unwrap();

    let mut buf = [0u8; BUF.len()];
    tcp.read_exact(&mut buf).await.unwrap();

    assert_eq!(buf, BUF);
}
