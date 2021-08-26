// UDP: https://github.com/shadowsocks/shadowsocks-rust/blob/0433b3ec09bcaa26f7460a50287b56c67b687a34/crates/shadowsocks-service/src/local/redir/udprelay/sys/unix/linux.rs#L56

use std::{
    io, mem,
    net::{SocketAddr, UdpSocket},
    os::unix::prelude::AsRawFd,
    ptr,
    time::Duration,
};

use crate::builtin::local::CompatTcp;
use cfg_if::cfg_if;
use lru_time_cache::LruCache;
use rd_interface::{
    async_trait,
    constant::UDP_BUFFER_SIZE,
    error::map_other,
    registry::ServerFactory,
    schemars::{self, JsonSchema},
    util::connect_tcp,
    Address, Context, Error, IServer, IntoAddress, IntoDyn, Net, Result,
};
use serde::Deserialize;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::{
    io::unix::AsyncFd,
    net::{TcpListener, TcpSocket, TcpStream},
    select,
    sync::{
        mpsc::{unbounded_channel, UnboundedSender as Sender},
        Mutex,
    },
    task::JoinHandle,
    time::timeout,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TProxyServerConfig {
    bind: Address,
}

pub struct TProxyServer {
    cfg: TProxyServerConfig,
    net: Net,
}

#[async_trait]
impl IServer for TProxyServer {
    async fn start(&self) -> Result<()> {
        let tcp_listener = create_tcp_listener(self.cfg.bind.to_socket_addr()?).await?;
        let udp_listener = create_udp_socket(self.cfg.bind.to_socket_addr()?).await?;

        select! {
            r = self.serve_listener(tcp_listener) => r,
            r = self.serve_udp(udp_listener) => r,
        }
    }
}

impl TProxyServer {
    pub fn new(cfg: TProxyServerConfig, net: Net) -> Self {
        TProxyServer { cfg, net }
    }

    async fn serve_udp(&self, listener: UdpListener) -> Result<()> {
        let mut buf = [0u8; 4096];
        let net = self.net.clone();
        let mut nat = LruCache::<SocketAddr, UdpTunnel>::with_expiry_duration_and_capacity(
            Duration::from_secs(30),
            128,
        );

        loop {
            let (size, src, dst) = listener.recv(&mut buf).await?;
            let payload = &buf[..size];

            let udp = nat
                .entry(src)
                .or_insert_with(|| UdpTunnel::new(net.clone(), src));

            if let Err(e) = udp.send_to(payload, dst).await {
                tracing::error!("Udp send_to {:?}", e);
                nat.remove(&src);
            }
        }
    }

    async fn serve_listener(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (socket, addr) = listener.accept().await?;
            let net = self.net.clone();
            let _ = tokio::spawn(async move {
                if let Err(e) = Self::serve_connection(net, socket, addr).await {
                    tracing::error!("Error when serve_connection: {:?}", e);
                }
            });
        }
    }

    async fn serve_connection(net: Net, socket: TcpStream, addr: SocketAddr) -> Result<()> {
        let target = socket.local_addr()?;

        let target_tcp = net
            .tcp_connect(&mut Context::from_socketaddr(addr), &target.into_address()?)
            .await?;
        let socket = CompatTcp(socket).into_dyn();

        connect_tcp(socket, target_tcp).await?;

        Ok(())
    }
}

impl ServerFactory for TProxyServer {
    const NAME: &'static str = "tproxy";
    type Config = TProxyServerConfig;
    type Server = Self;

    fn new(_: Net, net: Net, config: Self::Config) -> Result<Self> {
        Ok(TProxyServer::new(config, net))
    }
}

// https://github.com/shadowsocks/shadowsocks-rust/blob/0433b3ec09bcaa26f7460a50287b56c67b687a34/crates/shadowsocks-service/src/local/redir/tcprelay/sys/unix/linux.rs#L92-L134
async fn create_tcp_listener(addr: SocketAddr) -> io::Result<TcpListener> {
    let socket = match addr {
        SocketAddr::V4(..) => TcpSocket::new_v4()?,
        SocketAddr::V6(..) => TcpSocket::new_v6()?,
    };

    // For Linux 2.4+ TPROXY
    // Sockets have to set IP_TRANSPARENT, IPV6_TRANSPARENT for retrieving original destination by getsockname()
    unsafe {
        let fd = socket.as_raw_fd();

        let enable: libc::c_int = 1;
        let ret = match addr {
            SocketAddr::V4(..) => libc::setsockopt(
                fd,
                libc::IPPROTO_IP,
                libc::IP_TRANSPARENT,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            ),
            SocketAddr::V6(..) => libc::setsockopt(
                fd,
                libc::IPPROTO_IPV6,
                libc::IPV6_TRANSPARENT,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            ),
        };

        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    // tokio requires allow reuse addr
    socket.set_reuseaddr(true)?;

    // bind, listen as original
    socket.bind(addr)?;
    // listen backlogs = 1024 as mio's default
    socket.listen(1024)
}

struct UdpListener(AsyncFd<UdpSocket>);

// https://github.com/shadowsocks/shadowsocks-rust/blob/0433b3ec09bcaa26f7460a50287b56c67b687a34/crates/shadowsocks-service/src/local/redir/udprelay/sys/unix/linux.rs#L56
async fn create_udp_socket(addr: SocketAddr) -> io::Result<UdpListener> {
    let socket = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))?;
    set_socket_before_bind(&addr, &socket)?;

    socket.set_nonblocking(true)?;
    socket.set_reuse_address(true)?;
    socket.set_reuse_port(true)?;

    socket.bind(&SockAddr::from(addr))?;

    let io = AsyncFd::new(socket.into())?;
    Ok(UdpListener(io))
}

fn set_socket_before_bind(addr: &SocketAddr, socket: &Socket) -> io::Result<()> {
    let fd = socket.as_raw_fd();

    let enable: libc::c_int = 1;
    unsafe {
        // 1. Set IP_TRANSPARENT, IPV6_TRANSPARENT to allow binding to non-local addresses
        let ret = match *addr {
            SocketAddr::V4(..) => libc::setsockopt(
                fd,
                libc::SOL_IP,
                libc::IP_TRANSPARENT,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            ),
            SocketAddr::V6(..) => libc::setsockopt(
                fd,
                libc::SOL_IPV6,
                libc::IPV6_TRANSPARENT,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            ),
        };
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }

        // 2. Set IP_RECVORIGDSTADDR, IPV6_RECVORIGDSTADDR
        let ret = match *addr {
            SocketAddr::V4(..) => libc::setsockopt(
                fd,
                libc::SOL_IP,
                libc::IP_RECVORIGDSTADDR,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            ),
            SocketAddr::V6(..) => libc::setsockopt(
                fd,
                libc::SOL_IPV6,
                libc::IPV6_RECVORIGDSTADDR,
                &enable as *const _ as *const _,
                mem::size_of_val(&enable) as libc::socklen_t,
            ),
        };
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

fn get_destination_addr(msg: &libc::msghdr) -> io::Result<SocketAddr> {
    unsafe {
        let (_, addr) = SockAddr::init(|dst_addr, dst_addr_len| {
            let mut cmsg: *mut libc::cmsghdr = libc::CMSG_FIRSTHDR(msg);
            while !cmsg.is_null() {
                let rcmsg = &*cmsg;
                match (rcmsg.cmsg_level, rcmsg.cmsg_type) {
                    (libc::SOL_IP, libc::IP_RECVORIGDSTADDR) => {
                        ptr::copy(
                            libc::CMSG_DATA(cmsg),
                            dst_addr as *mut _,
                            mem::size_of::<libc::sockaddr_in>(),
                        );
                        *dst_addr_len = mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

                        return Ok(());
                    }
                    (libc::SOL_IPV6, libc::IPV6_RECVORIGDSTADDR) => {
                        ptr::copy(
                            libc::CMSG_DATA(cmsg),
                            dst_addr as *mut _,
                            mem::size_of::<libc::sockaddr_in6>(),
                        );
                        *dst_addr_len = mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t;

                        return Ok(());
                    }
                    _ => {}
                }
                cmsg = libc::CMSG_NXTHDR(msg, cmsg);
            }

            let err = io::Error::new(
                io::ErrorKind::InvalidData,
                "missing destination address in msghdr",
            );
            Err(err)
        })?;

        Ok(addr.as_socket().expect("SocketAddr"))
    }
}

fn recv_dest_from(
    socket: &UdpSocket,
    buf: &mut [u8],
) -> io::Result<(usize, SocketAddr, SocketAddr)> {
    unsafe {
        let mut control_buf = [0u8; 64];
        let mut src_addr: libc::sockaddr_storage = mem::zeroed();

        let mut msg: libc::msghdr = mem::zeroed();
        msg.msg_name = &mut src_addr as *mut _ as *mut _;
        msg.msg_namelen = mem::size_of_val(&src_addr) as libc::socklen_t;

        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut _,
            iov_len: buf.len() as libc::size_t,
        };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;

        msg.msg_control = control_buf.as_mut_ptr() as *mut _;
        cfg_if! {
            if #[cfg(any(target_env = "musl", all(target_env = "uclibc", target_arch = "arm")))] {
                msg.msg_controllen = control_buf.len() as libc::socklen_t;
            } else {
                msg.msg_controllen = control_buf.len() as libc::size_t;
            }
        }

        let fd = socket.as_raw_fd();
        let ret = libc::recvmsg(fd, &mut msg, 0);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        let (_, src_saddr) = SockAddr::init(|a, l| {
            ptr::copy_nonoverlapping(msg.msg_name, a as *mut _, msg.msg_namelen as usize);
            *l = msg.msg_namelen;
            Ok(())
        })?;

        Ok((
            ret as usize,
            src_saddr.as_socket().expect("SocketAddr"),
            get_destination_addr(&msg)?,
        ))
    }
}

impl UdpListener {
    async fn recv(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr, SocketAddr)> {
        loop {
            let mut guard = self.0.readable().await?;
            match guard.try_io(|inner| recv_dest_from(inner.get_ref(), buf)) {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        loop {
            let mut write_guard = self.0.writable().await?;
            match write_guard.try_io(|inner| inner.get_ref().send_to(buf, target)) {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }
}

struct UdpTunnel {
    tx: Sender<(SocketAddr, Vec<u8>)>,
    handle: Mutex<Option<JoinHandle<Result<()>>>>,
}

impl UdpTunnel {
    fn new(net: Net, src: SocketAddr) -> UdpTunnel {
        let (tx, mut rx) = unbounded_channel::<(SocketAddr, Vec<u8>)>();
        let handle = tokio::spawn(async move {
            let udp = timeout(
                Duration::from_secs(5),
                net.udp_bind(
                    &mut Context::from_socketaddr(src),
                    &"0.0.0.0:0".into_address()?,
                ),
            )
            .await
            .map_err(map_other)??;

            let send = async {
                while let Some((addr, packet)) = rx.recv().await {
                    udp.send_to(&packet, addr.into()).await?;
                }
                Ok(()) as Result<()>
            };
            let recv = async {
                let mut buf = [0u8; UDP_BUFFER_SIZE];
                loop {
                    let (size, addr) = udp.recv_from(&mut buf).await?;

                    // TODO: cache sockets here.
                    let back_udp = create_udp_socket(addr).await?;
                    if back_udp.send_to(&buf[..size], src).await.is_err() {
                        break;
                    }
                }
                tracing::trace!("send_raw return error");
                Ok(()) as Result<()>
            };

            let r = select! {
                r = send => r,
                r = recv => r,
            };

            if let Err(e) = &r {
                tracing::error!("Error {:?}", e);
            }

            Ok(()) as Result<()>
        });
        UdpTunnel {
            tx,
            handle: Mutex::new(handle.into()),
        }
    }
    /// return false if the send queue is full
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<()> {
        match self.tx.send((addr, buf.to_vec())) {
            Ok(_) => Ok(()),
            Err(_) => {
                let mut handle = self.handle.lock().await;
                if let Some(handle) = handle.take() {
                    let r = handle.await;
                    tracing::error!("Other side closed: {:?}", r);
                }
                Err(Error::Other("Other side closed".into()))
            }
        }
    }
}
