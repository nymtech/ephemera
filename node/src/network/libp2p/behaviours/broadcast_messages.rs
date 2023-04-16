use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use futures_util::{AsyncReadExt, AsyncWriteExt};
use libp2p::request_response;
use log::{error, trace};
use serde::{Deserialize, Serialize};

use crate::broadcast::RbMsg;
use crate::utilities::id::EphemeraId;

#[derive(Clone)]
pub(crate) struct RbMsgMessagesCodec;

impl RbMsgMessagesCodec {
    async fn write_length_prefixed<D: AsRef<[u8]>, I: AsyncWrite + Unpin>(
        io: &mut I,
        data: D,
    ) -> Result<(), std::io::Error> {
        Self::write_varint(io, data.as_ref().len() as u32).await?;
        io.write_all(data.as_ref()).await?;
        io.flush().await?;

        Ok(())
    }

    async fn write_varint<I: AsyncWrite + Unpin>(
        io: &mut I,
        len: u32,
    ) -> Result<(), std::io::Error> {
        let mut len_data = unsigned_varint::encode::u32_buffer();
        let encoded_len = unsigned_varint::encode::u32(len, &mut len_data).len();
        io.write_all(&len_data[..encoded_len]).await?;

        Ok(())
    }

    async fn read_varint<T: AsyncRead + Unpin>(io: &mut T) -> Result<u32, std::io::Error> {
        let mut buffer = unsigned_varint::encode::u32_buffer();
        let mut buffer_len = 0;

        loop {
            //read 1 byte at time because we don't know how it compacted 32 bit integer
            io.read_exact(&mut buffer[buffer_len..buffer_len + 1])
                .await?;
            buffer_len += 1;
            match unsigned_varint::decode::u32(&buffer[..buffer_len]) {
                Ok((len, _)) => {
                    trace!("Read varint: {}", len);
                    return Ok(len);
                }
                Err(unsigned_varint::decode::Error::Overflow) => {
                    error!("Invalid varint received");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid varint",
                    ));
                }
                Err(unsigned_varint::decode::Error::Insufficient) => {
                    continue;
                }
                Err(_) => {
                    error!("Varint decoding error: #[non_exhaustive]");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid varint",
                    ));
                }
            }
        }
    }

    async fn read_length_prefixed<T: AsyncRead + Unpin>(
        io: &mut T,
        max_size: u32,
    ) -> Result<Vec<u8>, std::io::Error> {
        let len = Self::read_varint(io).await?;
        if len > max_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Message too large",
            ));
        }

        let mut buf = vec![0; len as usize];
        io.read_exact(&mut buf).await?;
        Ok(buf)
    }
}

#[derive(Clone)]
pub(crate) struct RbMsgProtocol;

impl request_response::ProtocolName for RbMsgProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/ephemera/rb/1".as_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RbMsgResponse {
    pub(crate) id: EphemeraId,
}

impl RbMsgResponse {
    pub(crate) fn new(id: EphemeraId) -> Self {
        Self { id }
    }
}

#[async_trait]
impl request_response::Codec for RbMsgMessagesCodec {
    type Protocol = RbMsgProtocol;
    type Request = RbMsg;
    type Response = RbMsgResponse;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> Result<Self::Request, std::io::Error>
    where
        T: AsyncRead + Unpin + Send,
    {
        let data = Self::read_length_prefixed(io, 1024 * 1024).await?;
        let msg = serde_json::from_slice(&data)?;
        trace!("Received request {:?}", msg);
        Ok(msg)
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let response = Self::read_length_prefixed(io, 1024 * 1024).await?;
        let response = serde_json::from_slice(&response)?;
        trace!("Received response {:?}", response);
        Ok(response)
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> Result<(), std::io::Error>
    where
        T: AsyncWrite + Unpin + Send,
    {
        trace!("Writing request {:?}", req);
        let data = serde_json::to_vec(&req).unwrap();
        Self::write_length_prefixed(io, data).await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        response: Self::Response,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        trace!("Writing response {:?}", response);
        let response = serde_json::to_vec(&response).unwrap();
        Self::write_length_prefixed(io, response).await?;
        Ok(())
    }
}
