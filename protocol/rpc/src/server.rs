use std::{pin::Pin, task::Poll};

use futures::{future::poll_fn, ready, SinkExt};
use rd_interface::{
    async_trait, Address, AsyncRead, AsyncWrite, Bytes, Context, IServer, Net, ReadBuf, Result,
    Stream, TcpStream,
};
use serde_json::to_value;

use crate::{
    session::{Obj, RequestGetter, ServerSession},
    types::{Command, RpcValue},
};

#[derive(Clone)]
pub struct RpcServer {
    listen: Net,
    net: Net,
    bind: Address,
}
impl RpcServer {
    pub fn new(listen: Net, net: Net, bind: Address) -> RpcServer {
        RpcServer { listen, net, bind }
    }
}

#[async_trait]
impl IServer for RpcServer {
    async fn start(&self) -> Result<()> {
        let listener = self
            .listen
            .tcp_bind(&mut Context::new(), &self.bind)
            .await?;

        loop {
            let (conn, _) = listener.accept().await?;
            let this = self.clone();
            tokio::spawn(async move {
                if let Err(e) = this.handle_conn(conn).await {
                    tracing::error!("handle_conn {:?}", e);
                }
            });
        }
    }
}

impl RpcServer {
    async fn handle_req(&self, req: &RequestGetter) -> Result<(RpcValue, Option<Vec<u8>>)> {
        match req.cmd() {
            Command::TcpConnect(ctx, addr) => {
                let tcp = self
                    .net
                    .tcp_connect(&mut Context::from_value(ctx.clone())?, &addr)
                    .await?;

                Ok((
                    RpcValue::Object(req.insert_object(Obj::TcpStream(tcp))),
                    None,
                ))
            }
            Command::TcpBind(ctx, addr) => {
                let listener = self
                    .net
                    .tcp_bind(&mut Context::from_value(ctx.clone())?, &addr)
                    .await?;

                Ok((
                    RpcValue::Object(req.insert_object(Obj::TcpListener(listener))),
                    None,
                ))
            }
            Command::UdpBind(ctx, addr) => {
                let udp = self
                    .net
                    .udp_bind(&mut Context::from_value(ctx.clone())?, &addr)
                    .await?;

                Ok((
                    RpcValue::Object(req.insert_object(Obj::UdpSocket(udp))),
                    None,
                ))
            }
            Command::Close(obj) => {
                req.remove_object(*obj);

                Ok((RpcValue::Null, None))
            }
            Command::LookupHost(addr) => {
                let addrs = self.net.lookup_host(&addr).await?;

                Ok((RpcValue::Value(to_value(addrs)?), None))
            }
            Command::RecvFrom(obj) => {
                let obj = req.get_object(*obj)?;

                poll_fn(move |cx| {
                    let mut udp = ready!(obj.poll_lock(cx));
                    let udp = Pin::new(udp.udp_socket_mut()?);
                    let (buf, from) = match ready!(udp.poll_next(cx)) {
                        Some(item) => item?,
                        None => return Poll::Ready(Err(rd_interface::Error::other("no data"))),
                    };

                    Poll::Ready(Ok((RpcValue::Value(to_value(from)?), Some(buf.to_vec()))))
                })
                .await
            }
            Command::SendTo(obj, addr) => {
                let obj = req.get_object(*obj)?;
                let mut is_ready = false;
                let mut flushing = false;
                let data = Bytes::copy_from_slice(req.data());

                poll_fn(move |cx| {
                    let mut udp = ready!(obj.poll_lock(cx));
                    let udp = udp.udp_socket_mut()?;

                    loop {
                        if !is_ready {
                            ready!(udp.poll_ready_unpin(cx))?;
                            is_ready = true;
                        }
                        if flushing {
                            ready!(udp.poll_flush_unpin(cx))?;
                            return Poll::Ready(Ok((RpcValue::Null, None)));
                        }
                        udp.start_send_unpin((data.clone(), addr.clone()))?;
                        flushing = true;
                    }
                })
                .await
            }
            Command::Read(obj, buf_size) => {
                let obj = req.get_object(*obj)?;
                let mut buf = vec![0u8; *buf_size as usize];

                poll_fn(move |cx| {
                    let mut tcp = ready!(obj.poll_lock(cx));
                    let tcp = Pin::new(tcp.tcp_stream_mut()?);
                    let mut read_buf = ReadBuf::new(&mut buf);
                    ready!(tcp.poll_read(cx, &mut read_buf))?;
                    let filled = read_buf.filled().len();

                    Poll::Ready(Ok((RpcValue::Null, Some(buf[..filled].to_vec()))))
                })
                .await
            }
            Command::Write(obj) => {
                let obj = req.get_object(*obj)?;

                poll_fn(|cx| {
                    let mut tcp = ready!(obj.poll_lock(cx));
                    let tcp = Pin::new(tcp.tcp_stream_mut()?);
                    let write = ready!(tcp.poll_write(cx, &req.data()))?;

                    Poll::Ready(Ok((RpcValue::Value(to_value(write)?), None)))
                })
                .await
            }
            Command::Flush(obj) => {
                let obj = req.get_object(*obj)?;

                poll_fn(|cx| {
                    let mut tcp = ready!(obj.poll_lock(cx));
                    let tcp = Pin::new(tcp.tcp_stream_mut()?);
                    ready!(tcp.poll_flush(cx))?;

                    Poll::Ready(Ok((RpcValue::Null, None)))
                })
                .await
            }
            Command::Shutdown(obj) => {
                let obj = req.get_object(*obj)?;

                poll_fn(|cx| {
                    let mut tcp = ready!(obj.poll_lock(cx));
                    let tcp = Pin::new(tcp.tcp_stream_mut()?);
                    ready!(tcp.poll_shutdown(cx))?;

                    Poll::Ready(Ok((RpcValue::Null, None)))
                })
                .await
            }
            Command::Accept(obj) => {
                let obj = req.get_object(*obj)?;
                let obj = obj.lock().await;
                let (tcp, addr) = obj.tcp_listener()?.accept().await?;

                Ok((
                    RpcValue::ObjectValue(req.insert_object(Obj::TcpStream(tcp)), to_value(addr)?),
                    None,
                ))
            }
            Command::LocalAddr(obj) => {
                let obj = req.get_object(*obj)?;
                let obj = obj.lock().await;
                let addr = match &*obj {
                    Obj::TcpStream(tcp) => tcp.local_addr().await,
                    Obj::TcpListener(listener) => listener.local_addr().await,
                    Obj::UdpSocket(udp) => udp.local_addr().await,
                }?;

                Ok((RpcValue::Value(to_value(addr)?), None))
            }
            Command::PeerAddr(obj) => {
                let obj = req.get_object(*obj)?;
                let obj = obj.lock().await;
                let addr = match &*obj {
                    Obj::TcpStream(tcp) => tcp.peer_addr().await,
                    _ => return Err(rd_interface::Error::other("invalid object")),
                }?;

                Ok((RpcValue::Value(to_value(addr)?), None))
            }
            _ => Err(rd_interface::Error::other("Invalid command")),
        }
    }
    async fn handle_conn(&self, tcp: TcpStream) -> Result<()> {
        let sess = ServerSession::new(tcp);
        let handshake_req = sess.recv().await?;
        // TODO: handle session_id
        let _session_id = match handshake_req.cmd() {
            Command::Handshake(session_id) => session_id,
            _ => return Err(rd_interface::Error::other("Invalid handshake")),
        };
        handshake_req.response(Ok(RpcValue::Null), None).await?;

        loop {
            let req = sess.recv().await?;
            let this = self.clone();
            tokio::spawn(async move {
                let (result, data) = match this.handle_req(&req).await {
                    Ok((result, data)) => (Ok(result), data),
                    Err(e) => (Err(e.to_string()), None),
                };

                match data {
                    Some(data) => req.response(result, Some(&data[..])).await,
                    None => req.response(result, None).await,
                }
            });
        }
    }
}
