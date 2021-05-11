use futures::prelude::*;
use rd_interface::{AsyncRead, AsyncWrite};
use std::{
    io::{Error, ErrorKind, Result},
    net::{Ipv4Addr, SocketAddr},
};

#[derive(Debug)]
pub enum Address {
    SocketAddr(SocketAddr),
    Domain(String, u16),
}

impl Default for Address {
    fn default() -> Self {
        Address::SocketAddr(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))
    }
}

impl From<rd_interface::Address> for Address {
    fn from(addr: rd_interface::Address) -> Self {
        match addr {
            rd_interface::Address::SocketAddr(s) => Address::SocketAddr(s),
            rd_interface::Address::Domain(domain, port) => Address::Domain(domain, port),
        }
    }
}

impl Into<rd_interface::Address> for Address {
    fn into(self) -> rd_interface::Address {
        match self {
            Address::SocketAddr(s) => rd_interface::Address::SocketAddr(s),
            Address::Domain(domain, port) => rd_interface::Address::Domain(domain, port),
        }
    }
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        Address::SocketAddr(addr)
    }
}

impl Address {
    pub fn to_socket_addr(self) -> Result<SocketAddr> {
        match self {
            Address::SocketAddr(s) => Ok(s),
            _ => Err(ErrorKind::AddrNotAvailable.into()),
        }
    }
    async fn read_port<R>(mut reader: R) -> Result<u16>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf).await?;
        let port = u16::from_be_bytes(buf);
        Ok(port)
    }
    async fn write_port<W>(mut writer: W, port: u16) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        writer.write_all(&port.to_be_bytes()).await
    }
    pub async fn write<W>(&self, mut writer: W) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        match self {
            Address::SocketAddr(SocketAddr::V4(addr)) => {
                writer.write_all(&[0x01]).await?;
                writer.write_all(&addr.ip().octets()).await?;
                Self::write_port(writer, addr.port()).await?;
            }
            Address::SocketAddr(SocketAddr::V6(addr)) => {
                writer.write_all(&[0x04]).await?;
                writer.write_all(&addr.ip().octets()).await?;
                Self::write_port(writer, addr.port()).await?;
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
                Address::SocketAddr(SocketAddr::new(
                    ip.into(),
                    Self::read_port(&mut reader).await?,
                ))
            }
            3 => {
                let mut len = [0u8; 1];
                reader.read_exact(&mut len).await?;
                let len = len[0] as usize;
                let mut domain = vec![0u8; len];
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
                Address::SocketAddr(SocketAddr::new(
                    ip.into(),
                    Self::read_port(&mut reader).await?,
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
