use std::{
    io::{self, Cursor},
    marker::PhantomData,
    pin::Pin,
};

use serde::{Deserialize, Serialize};
use tokio::net::{
    TcpStream,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
};
use tokio_serde::{Deserializer, Framed, Serializer};
use tokio_util::{
    bytes::{Buf, Bytes, BytesMut},
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
};

pub type WrappedStream = FramedRead<OwnedReadHalf, LengthDelimitedCodec>;
pub type WrappedSink = FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>;

#[derive(Debug)]
pub struct Bson<Item, SinkItem> {
    phantom: PhantomData<(Item, SinkItem)>,
}

impl<Item, SinkItem> Default for Bson<Item, SinkItem> {
    fn default() -> Self {
        Self { phantom: PhantomData }
    }
}

impl<Item, SinkItem> Deserializer<Item> for Bson<Item, SinkItem>
where
    for<'a> Item: Deserialize<'a>,
{
    type Error = io::Error;

    fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> Result<Item, Self::Error> {
        bson::from_reader(Cursor::new(src).reader()).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("Failed to deserialize BSON: {err}")))
    }
}

impl<Item, SinkItem: Serialize> Serializer<SinkItem> for Bson<Item, SinkItem> {
    type Error = io::Error;

    fn serialize(self: Pin<&mut Self>, item: &SinkItem) -> Result<Bytes, Self::Error> {
        bson::to_vec(item)
            .map(Bytes::from)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("Failed to serialize BSON: {err}")))
    }
}

pub type InStream<T = ()> = Framed<WrappedStream, T, (), Bson<T, ()>>;
pub type OutSink<T = ()> = Framed<WrappedSink, (), T, Bson<(), T>>;

pub fn wrap_stream<I, O>(stream: TcpStream) -> (InStream<I>, OutSink<O>) {
    let (read, write) = stream.into_split();
    let stream = WrappedStream::new(read, LengthDelimitedCodec::new());
    let sink = WrappedSink::new(write, LengthDelimitedCodec::new());

    (InStream::new(stream, Bson::default()), OutSink::new(sink, Bson::default()))
}
