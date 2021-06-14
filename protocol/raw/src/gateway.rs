use std::{
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use dashmap::DashMap;
use futures::{Sink, SinkExt, Stream, StreamExt};
use rd_interface::Result;
use tokio_smoltcp::device::{Interface, Packet};

pub struct GatewayInterface<I> {
    inner: I,
    map: Arc<DashMap<SocketAddr, SocketAddr>>,
}

impl<I> GatewayInterface<I> {
    pub fn new(inner: I) -> GatewayInterface<I> {
        GatewayInterface {
            inner,
            map: Arc::new(DashMap::new()),
        }
    }
    pub fn get_map(&self) -> Arc<DashMap<SocketAddr, SocketAddr>> {
        self.map.clone()
    }
}

impl<I> Stream for GatewayInterface<I>
where
    I: Interface,
{
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_next_unpin(cx)
    }
}

impl<I> Sink<Packet> for GatewayInterface<I>
where
    I: Interface,
{
    type Error = I::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        self.inner.start_send_unpin(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx)
    }
}
