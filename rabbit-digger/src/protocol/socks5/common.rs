use apir::traits::{AsyncRead, AsyncWrite};
use futures::prelude::*;
use std::{
    io::{Cursor, Error, ErrorKind, Result},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};
pub enum Address {
    IPv4(SocketAddrV4),
    IPv6(SocketAddrV6),
    Domain(String),
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(v4) => Address::IPv4(v4),
            SocketAddr::V6(v6) => Address::IPv6(v6),
        }
    }
}

impl Address {
    pub fn to_socket_addr(self) -> Result<SocketAddr> {
        match self {
            Address::IPv4(v4) => Ok(SocketAddr::V4(v4)),
            Address::IPv6(v6) => Ok(SocketAddr::V6(v6)),
            _ => Err(ErrorKind::AddrNotAvailable.into()),
        }
    }
    async fn read_port<R>(mut reader: R) -> Result<u16>
    where
        R: AsyncRead + Unpin,
    {
        let mut port = [0u8; 2];
        reader.read_exact(&mut port).await?;
        Ok((port[0] as u16) << 8 | port[1] as u16)
    }
    async fn write_port<W>(mut writer: W, port: u16) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all(&[(port >> 8) as u8, port as u8]).await
    }
    pub async fn write<W>(&self, mut writer: W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            Address::IPv4(ip) => {
                writer.write_all(&[0x01]).await?;
                writer.write_all(&ip.ip().octets()).await?;
                Self::write_port(writer, ip.port()).await?;
            }
            Address::IPv6(ip) => {
                writer.write_all(&[0x04]).await?;
                writer.write_all(&ip.ip().octets()).await?;
                Self::write_port(writer, ip.port()).await?;
            }
            Address::Domain(domain) => {
                if domain.len() >= 256 {
                    return Err(ErrorKind::InvalidInput.into());
                }
                let header = [0x03, domain.len() as u8];
                writer.write_all(&header).await?;
                writer.write_all(domain.as_bytes()).await?;
            }
        };
        Ok(())
    }
    pub async fn read<R>(mut reader: R) -> Result<Self>
    where
        R: AsyncRead + Unpin,
    {
        let mut atyp = [0u8; 1];
        reader.read_exact(&mut atyp).await?;

        Ok(match atyp[0] {
            1 => {
                let mut ip = [0u8; 4];
                reader.read_exact(&mut ip).await?;
                Address::IPv4(SocketAddrV4::new(
                    ip.into(),
                    Self::read_port(&mut reader).await?,
                ))
            }
            3 => {
                let mut len = [0u8; 1];
                reader.read_exact(&mut len).await?;
                let len = len[0] as usize;
                let mut domain = Vec::new();
                domain.resize(len, 0);
                reader.read_exact(&mut domain).await?;

                let domain = String::from_utf8(domain).map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidData,
                        format!("bad domain {:?}", e.as_bytes()),
                    )
                })?;

                Address::Domain(domain)
            }
            4 => {
                let mut ip = [0u8; 16];
                reader.read_exact(&mut ip).await?;
                Address::IPv6(SocketAddrV6::new(
                    ip.into(),
                    Self::read_port(&mut reader).await?,
                    0,
                    0,
                ))
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("bad atyp {}", atyp[0]),
                ))
            }
        })
    }
}
pub async fn pack_send(
    mut w: impl AsyncWrite + Unpin,
    f: impl Fn(&mut Cursor<Vec<u8>>) -> Result<()>,
) -> Result<()> {
    let mut buf = Cursor::new(Vec::with_capacity(1024));
    f(&mut buf)?;
    w.write_all(&buf.into_inner()).await?;
    w.flush().await
}
