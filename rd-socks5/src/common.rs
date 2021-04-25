use futures::prelude::*;
use rd_interface::{AsyncRead, AsyncWrite};
use std::{
    io::{Cursor, Error, ErrorKind, Result},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};

#[derive(Debug)]
pub enum Address {
    IPv4(SocketAddrV4),
    IPv6(SocketAddrV6),
    Domain(String, u16),
}

impl From<rd_interface::Address> for Address {
    fn from(addr: rd_interface::Address) -> Self {
        match addr {
            rd_interface::Address::IPv4(v4) => Address::IPv4(v4),
            rd_interface::Address::IPv6(v6) => Address::IPv6(v6),
            rd_interface::Address::Domain(domain, port) => Address::Domain(domain, port),
        }
    }
}

impl Into<rd_interface::Address> for Address {
    fn into(self) -> rd_interface::Address {
        match self {
            Address::IPv4(v4) => rd_interface::Address::IPv4(v4),
            Address::IPv6(v4) => rd_interface::Address::IPv6(v4),
            Address::Domain(domain, port) => rd_interface::Address::Domain(domain, port),
        }
    }
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
            Address::Domain(domain, port) => {
                if domain.len() >= 256 {
                    return Err(ErrorKind::InvalidInput.into());
                }
                let header = [0x03, domain.len() as u8];
                writer.write_all(&header).await?;
                writer.write_all(domain.as_bytes()).await?;
                Self::write_port(writer, *port).await?;
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

                Address::Domain(domain, Self::read_port(&mut reader).await?)
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

pub async fn parse_udp(buf: &[u8]) -> Result<(Address, &[u8])> {
    let mut cursor = futures::io::Cursor::new(buf);
    let mut header = [0u8; 3];
    cursor.read_exact(&mut header).await?;
    let addr = match header[0..3] {
        // TODO: support fragment sequence or at least give another error
        [0x00, 0x00, 0x00] => Address::read(&mut cursor).await?,
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "server response wrong RSV {} RSV {} FRAG {}",
                    header[0], header[1], header[2]
                ),
            )
            .into())
        }
    };

    let pos = cursor.position() as usize;

    Ok((addr, &cursor.into_inner()[pos..]))
}

pub async fn pack_udp(addr: Address, buf: &[u8]) -> Result<Vec<u8>> {
    let addr: Address = addr.into();
    let mut cursor = futures::io::Cursor::new(Vec::new());
    cursor.write_all(&[0x00, 0x00, 0x00]).await?;
    addr.write(&mut cursor).await?;
    cursor.write_all(buf).await?;

    let bytes = cursor.into_inner();

    Ok(bytes)
}
