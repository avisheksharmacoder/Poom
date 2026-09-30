use bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder, Framed};

use poom_protocol::{ClientMessage, FrameEncoder, ServerMessage, StreamFrameDecoder};
use crate::error::TransportError;
use crate::stream::IpcStream;

/// Codec for the Server side: decodes incoming `ClientMessage`s, encodes outgoing `ServerMessage`s.
#[derive(Debug, Default, Clone)]
pub struct ServerCodec;

impl Decoder for ServerCodec {
    type Item = ClientMessage;
    type Error = TransportError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        StreamFrameDecoder::decode_client_message(src).map_err(TransportError::from)
    }
}

impl Encoder<ServerMessage> for ServerCodec {
    type Error = TransportError;

    fn encode(&mut self, item: ServerMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        FrameEncoder::encode_server_message(&item, dst).map_err(TransportError::from)
    }
}

/// Codec for the Client side: decodes incoming `ServerMessage`s, encodes outgoing `ClientMessage`s.
#[derive(Debug, Default, Clone)]
pub struct ClientCodec;

impl Decoder for ClientCodec {
    type Item = ServerMessage;
    type Error = TransportError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        StreamFrameDecoder::decode_server_message(src).map_err(TransportError::from)
    }
}

impl Encoder<ClientMessage> for ClientCodec {
    type Error = TransportError;

    fn encode(&mut self, item: ClientMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        FrameEncoder::encode_client_message(&item, dst).map_err(TransportError::from)
    }
}

/// A framed server connection stream.
pub type ServerFramed = Framed<IpcStream, ServerCodec>;

/// A framed client connection stream.
pub type ClientFramed = Framed<IpcStream, ClientCodec>;
