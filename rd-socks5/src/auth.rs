use super::common::pack_send;
use futures::prelude::*;
use rd_interface::{async_trait, AsyncRead, AsyncWrite};
use std::io::{Error, ErrorKind, Result, Write};

#[async_trait]
pub trait Method {
    fn method_id(&self) -> u8;
    async fn auth_client(&self) -> Result<()>;
    async fn auth_server(&self) -> Result<()>;
}

fn err(s: &'static str) -> Error {
    Error::new(ErrorKind::ConnectionRefused, s.to_string())
}

pub async fn auth_client<S: AsyncRead + AsyncWrite + Unpin>(
    mut s: S,
    methods: &[&(dyn Method + Send + Sync)],
) -> Result<()> {
    let len = methods.len();
    if len > 100 {
        return Err(ErrorKind::InvalidInput.into());
    }
    pack_send(&mut s, |buf| {
        buf.write_all(&[5u8, len as u8])?;
        buf.write_all(&methods.iter().map(|m| m.method_id()).collect::<Vec<_>>())?;
        Ok(())
    })
    .await?;

    let mut buf = [0u8; 2];
    s.read_exact(&mut buf).await?;
    if buf[0] != 0x05 {
        return Err(err("handshake failed: wrong socks version"));
    }
    let selected = buf[1] as usize;
    if selected == 0xFF {
        return Err(err("server doesn't support any auth method"));
    }

    let method = methods
        .get(selected)
        .ok_or(err("server respond wrong method"))?;

    method.auth_client().await?;

    Ok(())
}

pub async fn auth_server<S: AsyncRead + AsyncWrite + Unpin>(
    mut s: S,
    methods: &[&(dyn Method + Send + Sync)],
) -> Result<()> {
    let mut buf = [0u8; 2];
    s.read_exact(&mut buf).await?;
    if buf[0] != 0x05 {
        return Err(err("handshake failed: wrong socks version"));
    }
    if buf[1] == 0 {
        return Err(err("handshake failed: client support 0 methods"));
    }
    let mut buf = vec![0u8; buf[1] as usize];
    s.read_exact(&mut buf).await?;

    // select method
    let method = match methods.iter().find(|m| buf.contains(&m.method_id())) {
        Some(m) => m,
        None => {
            s.write_all(&[0x05, 0xFF]).await?;
            s.flush().await?;
            return Err(err("method not supported"));
        }
    };

    let method_id = method.method_id();
    let method_offset = buf.iter().position(|m| *m == method_id).unwrap() as u8;
    s.write_all(&[0x05, method_offset]).await?;
    s.flush().await?;

    method.auth_server().await?;

    Ok(())
}

pub struct NoAuth;
#[async_trait]
impl Method for NoAuth {
    fn method_id(&self) -> u8 {
        // rfc 1928
        0
    }
    async fn auth_client(&self) -> Result<()> {
        Ok(())
    }
    async fn auth_server(&self) -> Result<()> {
        Ok(())
    }
}
