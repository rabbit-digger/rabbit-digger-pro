use core::fmt;
use std::{
    io::{self, Cursor, Write},
    pin::Pin,
    task::{Context, Poll},
};

use crate::Obfs;
use futures::ready;
use pin_project_lite::pin_project;
use rand::prelude::*;
use rd_interface::{
    async_trait, prelude::*, rd_config, Address, AsyncWrite, ITcpStream, IntoDyn, ReadBuf, Result,
    TcpStream, NOT_IMPLEMENTED,
};
use tokio::io::AsyncRead;

#[rd_config]
#[derive(Debug)]
pub struct HttpSimple {
    obfs_param: String,
}

impl Obfs for HttpSimple {
    fn tcp_connect(
        &self,
        tcp: TcpStream,
        _ctx: &mut rd_interface::Context,
        _addr: Address,
    ) -> Result<TcpStream> {
        Ok(Connect::new(tcp, &self.obfs_param).into_dyn())
    }

    fn tcp_accept(&self, _tcp: TcpStream, _addr: std::net::SocketAddr) -> Result<TcpStream> {
        Err(NOT_IMPLEMENTED)
    }
}

enum WriteState {
    Wait,
    Write(Vec<u8>, usize),
    Done,
}

enum ReadState {
    Read(Vec<u8>, usize),
    Write(Vec<u8>, usize),
    Done,
}

pin_project! {
    struct Connect {
        inner: TcpStream,
        write: WriteState,
        read: ReadState,
        obfs_param: String,
    }
}

impl Connect {
    fn new(tcp: TcpStream, param: &str) -> Connect {
        Connect {
            inner: tcp,
            write: WriteState::Wait,
            read: ReadState::Read(vec![0u8; 8192], 0),
            obfs_param: param.to_string(),
        }
    }
}

impl AsyncRead for Connect {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut this = self.project();
        loop {
            match this.read {
                ReadState::Read(ref mut read_buf, pos) => {
                    let mut tmp_buf = ReadBuf::new(&mut read_buf[*pos..]);
                    ready!(Pin::new(&mut this.inner).poll_read(cx, &mut tmp_buf))?;

                    *pos += tmp_buf.filled().len();

                    if let Some(at) = find_subsequence_end(&read_buf, b"\r\n\r\n") {
                        read_buf.truncate(*pos);
                        *this.read = ReadState::Write(read_buf.split_off(at), 0);
                    }
                }
                ReadState::Write(ref write_buf, pos) => {
                    let remaining = &write_buf[*pos..];

                    let to_read = remaining.len().min(buf.remaining());
                    buf.initialize_unfilled_to(to_read)
                        .copy_from_slice(&remaining[..to_read]);

                    buf.advance(to_read);
                    *pos += to_read;

                    if write_buf.len() == *pos {
                        *this.read = ReadState::Done;
                    }
                    return Poll::Ready(Ok(()));
                }
                ReadState::Done => {
                    return Pin::new(&mut this.inner).poll_read(cx, buf);
                }
            }
        }
    }
}

struct UrlEncode<'a>(&'a [u8]);

impl<'a> fmt::Display for UrlEncode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in self.0 {
            write!(f, "%{:02x}", i)?;
        }
        Ok(())
    }
}

impl AsyncWrite for Connect {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let mut this = self.project();
        loop {
            match this.write {
                WriteState::Wait => {
                    let head_len = thread_rng().gen_range(0..64usize).min(buf.len());
                    let head = &buf[..head_len];
                    let body = &buf[head_len..];

                    let mut cursor = Cursor::new(Vec::<u8>::with_capacity(1024));
                    cursor.write_fmt(format_args!(
                        "GET /{path} HTTP/1.1\r\nHost: {host}\r\n\r\n",
                        path = UrlEncode(head),
                        host = this.obfs_param
                    ))?;
                    cursor.write_all(body)?;

                    let buf = cursor.into_inner();

                    *this.write = WriteState::Write(buf, 0);
                }
                WriteState::Write(ref buf, pos) => {
                    let wrote = ready!(Pin::new(&mut this.inner).poll_write(cx, &buf[*pos..]))?;
                    *pos += wrote;

                    if buf.len() == *pos {
                        *this.write = WriteState::Done;
                    }
                }
                WriteState::Done => {
                    return Pin::new(&mut this.inner).poll_write(cx, buf);
                }
            };
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[async_trait]
impl ITcpStream for Connect {
    async fn peer_addr(&self) -> Result<std::net::SocketAddr> {
        self.inner.peer_addr().await
    }

    async fn local_addr(&self) -> Result<std::net::SocketAddr> {
        self.inner.local_addr().await
    }
}

fn find_subsequence_end(array: &[u8], pattern: &[u8]) -> Option<usize> {
    array
        .windows(pattern.len())
        .position(|window| window == pattern)
        .map(|at| at + pattern.len())
}
