use crate::prelude::*;

use asynchronous_codec::{Framed, BytesMut};
use std::io;
pub(crate) type KadStreamSink<S, A, B> = stream::AndThen<sink::With<stream::ErrInto<Framed<S, UviBytes<io::Cursor<Vec<u8>>>>, io::Error>, io::Cursor<Vec<u8>>, A, future::Ready<Result<io::Cursor<Vec<u8>>, io::Error>>, fn(A) -> future::Ready<Result<io::Cursor<Vec<u8>>, io::Error>>>, future::Ready<Result<B, io::Error>>, fn(BytesMut) -> future::Ready<Result<B, io::Error>>>;
use unsigned_varint::codec::UviBytes;

pub struct ArcConfig {
    pub inner: Arc<KamilataConfig>,
}

impl From<&Arc<KamilataConfig>> for ArcConfig {
    fn from(inner: &Arc<KamilataConfig>) -> Self {
        Self { inner: Arc::clone(inner) }
    }
}

impl UpgradeInfo for ArcConfig {
    type Info = String;
    type InfoIter = std::vec::IntoIter<std::string::String>;

    fn protocol_info(&self) -> Self::InfoIter {
        self.inner.protocol_names.clone().into_iter()
    }
}

pub(crate) type KamInStreamSink<S> = KadStreamSink<S, ResponsePacket, RequestPacket>;
pub(crate) type KamOutStreamSink<S> = KadStreamSink<S, RequestPacket, ResponsePacket>;

impl<S> InboundUpgrade<S> for ArcConfig
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    type Output = KamInStreamSink<S>;
    type Error = ioError;
    type Future = future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, socket: S, _: Self::Info) -> Self::Future {
        use protocol::{Parcel, Settings as ProtocolSettings};

        let mut codec = UviBytes::default();
        codec.set_max_len(5_000_000); // TODO: Change this value

        future::ok(
            Framed::new(socket, codec)
                .err_into()
                .with::<_, _, fn(_) -> _, _>(|response: ResponsePacket| {
                    let stream = response.into_stream(&ProtocolSettings::default()).map_err(|e| {
                        ioError::new(std::io::ErrorKind::Other, e.to_string()) // TODO: error handling
                    });
                    future::ready(stream)
                })
                .and_then::<_, fn(_) -> _>(|bytes: BytesMut| {
                    let request = RequestPacket::from_raw_bytes(&bytes, &ProtocolSettings::default()).map_err(|e| {
                        ioError::new(std::io::ErrorKind::Other, e.to_string()) // TODO: error handling
                    });
                    future::ready(request)
                }),
        )
    }
}

impl<S> OutboundUpgrade<S> for ArcConfig
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    type Output = KamOutStreamSink<S>;
    type Error = ioError;
    type Future = future::Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, socket: S, _: Self::Info) -> Self::Future {
        use protocol::{Parcel, Settings as ProtocolSettings};

        let mut codec = UviBytes::default();
        codec.set_max_len(5_000_000); // TODO: Change this value

        future::ok(
            Framed::new(socket, codec)
                .err_into()
                .with::<_, _, fn(_) -> _, _>(|request: RequestPacket| {
                    let stream = request.into_stream(&ProtocolSettings::default()).map_err(|e| {
                        ioError::new(std::io::ErrorKind::Other, e.to_string()) // TODO error handling
                    });
                    future::ready(stream)
                })
                .and_then::<_, fn(_) -> _>(|bytes: BytesMut| {
                    let response = ResponsePacket::from_raw_bytes(&bytes, &ProtocolSettings::default()).map_err(|e| {
                        ioError::new(std::io::ErrorKind::Other, e.to_string()) // TODO error handling
                    });
                    future::ready(response)
                }),
        )
    }
}
