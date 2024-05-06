use std::pin::Pin;
use std::task::{Context, Poll};
use futures::{Sink, Stream};
use pin_project::pin_project;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use crate::codec::PacketCodec;
use crate::packet::Packet;

#[pin_project]
pub struct Connection {
    #[pin]
    inner: Framed<TcpStream, PacketCodec>
}

impl Connection {
    pub fn new(inner: Framed<TcpStream, PacketCodec>) -> Self {
        Self { inner }
    }

    pub fn get_ref(&self) -> &TcpStream {
        self.inner.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut TcpStream {
        self.inner.get_mut()
    }

    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut TcpStream> {
        self.project().inner.get_pin_mut()
    }
}

impl Sink<Packet> for Connection {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        self.project().inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().inner.poll_close(cx)
    }
}

impl Stream for Connection {
    type Item = Result<Packet, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().inner.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
