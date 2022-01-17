use rd_interface::{Address as RDAddr, Bytes};
use socks5_protocol::{sync::FromIO, Address, Error};
use std::io::{self, ErrorKind, Read, Result, Write};

pub fn map_err(e: Error) -> rd_interface::Error {
    match e {
        Error::Io(io) => rd_interface::Error::IO(io),
        e => rd_interface::Error::Other(e.into()),
    }
}

pub fn parse_udp(buf: Bytes) -> Result<(RDAddr, Bytes)> {
    let mut cursor = std::io::Cursor::new(buf);
    let mut header = [0u8; 3];
    cursor.read_exact(&mut header)?;
    let addr = match header[0..3] {
        // TODO: support fragment sequence or at least give another error
        [0x00, 0x00, 0x00] => Address::read_from(&mut cursor).map_err(map_err)?,
        _ => {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "server response wrong RSV {} RSV {} FRAG {}",
                    header[0], header[1], header[2]
                ),
            ))
        }
    };

    let pos = cursor.position() as usize;

    Ok((sa2ra(addr), cursor.into_inner().slice(pos..)))
}

pub fn pack_udp(addr: RDAddr, buf: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    cursor.write_all(&[0x00, 0x00, 0x00])?;
    ra2sa(addr).write_to(&mut cursor).map_err(map_err)?;
    cursor.write_all(buf)?;

    let bytes = cursor.into_inner();

    Ok(bytes)
}

pub fn sa2ra(addr: socks5_protocol::Address) -> rd_interface::Address {
    match addr {
        socks5_protocol::Address::Domain(d, p) => rd_interface::Address::Domain(d, p),
        socks5_protocol::Address::SocketAddr(s) => rd_interface::Address::SocketAddr(s),
    }
}
pub fn ra2sa(addr: rd_interface::Address) -> socks5_protocol::Address {
    match addr {
        rd_interface::Address::Domain(d, p) => socks5_protocol::Address::Domain(d, p),
        rd_interface::Address::SocketAddr(s) => socks5_protocol::Address::SocketAddr(s),
    }
}
