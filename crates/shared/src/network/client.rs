use std::io;

use futures::{SinkExt, StreamExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use super::{InStream, IncomingPacket, OutSink, OutgoingPacket, wrap_stream};

pub struct Client(InStream<OutgoingPacket>, OutSink<IncomingPacket>);

impl Client {
    pub fn new(stream: TcpStream) -> Self {
        let (in_stream, out_stream) = wrap_stream(stream);

        Self(in_stream, out_stream)
    }

    pub async fn connect<T: ToSocketAddrs>(addr: T) -> Result<Self, io::Error> {
        TcpStream::connect(addr).await.map(Self::new)
    }

    pub async fn receive(&mut self) -> Option<Result<OutgoingPacket, io::Error>> {
        self.0.next().await
    }

    pub async fn send(&mut self, packet: IncomingPacket) -> Result<(), io::Error> {
        self.1.send(packet).await
    }
}
