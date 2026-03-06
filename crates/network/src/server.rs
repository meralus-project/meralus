use std::io;

use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;

use super::{InStream, IncomingPacket, OutSink, OutgoingPacket, wrap_stream};

pub struct ServerConnection(InStream<IncomingPacket>, OutSink<OutgoingPacket>);

impl ServerConnection {
    pub fn new(stream: TcpStream) -> Self {
        let (in_stream, out_stream) = wrap_stream(stream);

        Self(in_stream, out_stream)
    }

    pub async fn receive(&mut self) -> Option<Result<IncomingPacket, io::Error>> {
        self.0.next().await
    }

    pub async fn send(&mut self, packet: OutgoingPacket) -> Result<(), io::Error> {
        self.1.send(packet).await
    }
}
