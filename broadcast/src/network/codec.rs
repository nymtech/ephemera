///! A codec for encoding and decoding protobuf messages using  Prost crate.
///!

use std::io;
use std::marker::PhantomData;

use bytes::{Buf, BytesMut};
use prost::Message;
use tokio_util::codec::{Decoder, Encoder};

pub struct ProtoCodec<I, O> {
    _incoming: PhantomData<I>,
    _outgoing: PhantomData<O>,
}

impl<I, O> ProtoCodec<I, O> {
    pub fn new() -> Self {
        Self {
            _incoming: PhantomData,
            _outgoing: PhantomData,
        }
    }
}

pub const MAX_VARINT_LENGTH: usize = 16;

impl<I: Message + Default, O: Message> Decoder for ProtoCodec<I, O> {
    type Item = I;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let total_bytes = src.len();
        let mut decode_view = src.clone().freeze();
        let type_size = match prost::encoding::decode_varint(&mut decode_view) {
            Ok(len) => len,
            Err(_) if total_bytes <= MAX_VARINT_LENGTH => return Ok(None),
            Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidInput, err)),
        };
        let remaining = decode_view.remaining() as u64;
        if remaining < type_size {
            Ok(None)
        } else {
            let delim_len = total_bytes - decode_view.remaining();
            src.advance(delim_len + (type_size as usize));

            let mut result_bytes = BytesMut::from(decode_view.split_to(type_size as usize).as_ref());
            let res =
                I::decode(&mut result_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
            Ok(Some(res))
        }
    }
}

impl<I: Message + Default, O: Message> Encoder<O> for ProtoCodec<I, O> {
    type Error = io::Error;

    fn encode(&mut self, item: O, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.encode_length_delimited(dst)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
    }
}

pub trait Codec<I, O>: Decoder + Encoder<I> {}
