use apir::traits::{NotImplement, ProxyRuntime};
use std::marker::PhantomData;

pub struct Socks5Client<PR> {
    _pr: PhantomData<PR>,
}

impl<PR> ProxyRuntime for Socks5Client<PR>
where
    PR: ProxyRuntime,
{
    type TcpListener = NotImplement<Self::TcpStream>;
    type TcpStream = NotImplement;
    type UdpSocket = NotImplement;
}
