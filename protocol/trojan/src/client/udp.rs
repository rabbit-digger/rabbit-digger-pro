use std::{io, net::SocketAddr};

use crate::stream::IOStream;
use bytes::{Buf, BufMut};
use rd_interface::{
    async_trait, impl_stream_sink, Address, Bytes, BytesMut, IUdpSocket, Result, NOT_IMPLEMENTED,
};
use socks5_protocol::{sync::FromIO, Address as S5Addr};
use tokio_util::codec::{Decoder, Encoder, Framed};

// limited by 2-bytes header
const UDP_MAX_SIZE: usize = 65535;

struct UdpCodec {
    head: Vec<u8>,
}

impl Encoder<(Bytes, Address)> for UdpCodec {
    type Error = io::Error;

    fn encode(&mut self, item: (Bytes, Address), dst: &mut BytesMut) -> Result<(), Self::Error> {
        if item.0.len() > UDP_MAX_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", item.0.len()),
            ));
        }

        let addr = match item.1 {
            Address::Domain(domain, port) => S5Addr::Domain(domain, port),
            Address::SocketAddr(s) => S5Addr::SocketAddr(s),
        };
        dst.reserve(
            self.head.len() + addr.serialized_len().map_err(|e| e.to_io_err())? + item.0.len(),
        );
        if self.head.len() > 0 {
            dst.extend_from_slice(&self.head);
            self.head = Vec::new();
        }
        let mut writer = dst.writer();

        addr.write_to(&mut writer).map_err(|e| e.to_io_err())?;
        let dst = writer.into_inner();

        dst.put_u16(item.0.len() as u16);
        dst.extend_from_slice(&[0x0D, 0x0A]);
        dst.extend_from_slice(&item.0);

        Ok(())
    }
}

fn copy_2(b: &[u8]) -> [u8; 2] {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&b);
    buf
}

impl Decoder for UdpCodec {
    type Item = (BytesMut, SocketAddr);
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            return Ok(None);
        }
        let head = copy_2(&src[0..2]);
        let addr_size = match head[0] {
            1 => 7,
            3 => 1 + head[1] as usize + 2,
            4 => 19,
            _ => return Err(io::ErrorKind::InvalidData.into()),
        };
        if src.len() < addr_size + 4 {
            return Ok(None);
        }
        let length = u16::from_be_bytes(copy_2(&src[addr_size..addr_size + 2])) as usize;
        if src.len() < addr_size + 4 + length {
            return Ok(None);
        }

        let mut reader = src.reader();
        let address = S5Addr::read_from(&mut reader).map_err(|e| e.to_io_err())?;
        let src = reader.into_inner();

        // Length and CrLf
        src.get_u16();
        src.get_u16();

        Ok(Some((
            src.split_to(length as usize),
            address.to_socket_addr().map_err(|e| e.to_io_err())?,
        )))
    }
}

pub(super) struct TrojanUdp {
    framed: Framed<Box<dyn IOStream>, UdpCodec>,
}

impl TrojanUdp {
    pub fn new(stream: Box<dyn IOStream>, head: Vec<u8>) -> Self {
        let framed = Framed::new(stream, UdpCodec { head });
        Self { framed }
    }
}

impl_stream_sink!(TrojanUdp, framed);

#[async_trait]
impl IUdpSocket for TrojanUdp {
    async fn local_addr(&self) -> rd_interface::Result<SocketAddr> {
        Err(NOT_IMPLEMENTED)
    }
}
