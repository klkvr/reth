//! Connection types for a session

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{Sink, Stream};
use reth_ecies::stream::ECIESStream;
use reth_eth_wire::{
    errors::EthStreamError,
    message::EthBroadcastMessage,
    multiplex::{ProtocolProxy, RlpxSatelliteStream},
    EthMessage, EthStream, EthVersion, NetworkTypes, P2PStream,
};
use tokio::net::TcpStream;

/// The type of the underlying peer network connection.
pub type EthPeerConnection<T> = EthStream<P2PStream<ECIESStream<TcpStream>>, T>;

/// Various connection types that at least support the ETH protocol.
pub type EthSatelliteConnection<T> =
    RlpxSatelliteStream<ECIESStream<TcpStream>, EthStream<ProtocolProxy, T>>;

/// Connection types that support the ETH protocol.
///
/// This can be either:
/// - A connection that only supports the ETH protocol
/// - A connection that supports the ETH protocol and at least one other `RLPx` protocol
// This type is boxed because the underlying stream is ~6KB,
// mostly coming from `P2PStream`'s `snap::Encoder` (2072), and `ECIESStream` (3600).
#[derive(Debug)]
pub enum EthRlpxConnection<T: NetworkTypes> {
    /// A connection that only supports the ETH protocol.
    EthOnly(Box<EthPeerConnection<T>>),
    /// A connection that supports the ETH protocol and __at least one other__ `RLPx` protocol.
    Satellite(Box<EthSatelliteConnection<T>>),
}

impl<T: NetworkTypes> EthRlpxConnection<T> {
    /// Returns the negotiated ETH version.
    #[inline]
    pub(crate) const fn version(&self) -> EthVersion {
        match self {
            Self::EthOnly(conn) => conn.version(),
            Self::Satellite(conn) => conn.primary().version(),
        }
    }

    /// Consumes this type and returns the wrapped [`P2PStream`].
    #[inline]
    pub(crate) fn into_inner(self) -> P2PStream<ECIESStream<TcpStream>> {
        match self {
            Self::EthOnly(conn) => conn.into_inner(),
            Self::Satellite(conn) => conn.into_inner(),
        }
    }

    /// Returns mutable access to the underlying stream.
    #[inline]
    pub(crate) fn inner_mut(&mut self) -> &mut P2PStream<ECIESStream<TcpStream>> {
        match self {
            Self::EthOnly(conn) => conn.inner_mut(),
            Self::Satellite(conn) => conn.inner_mut(),
        }
    }

    /// Returns  access to the underlying stream.
    #[inline]
    pub(crate) const fn inner(&self) -> &P2PStream<ECIESStream<TcpStream>> {
        match self {
            Self::EthOnly(conn) => conn.inner(),
            Self::Satellite(conn) => conn.inner(),
        }
    }

    /// Same as [`Sink::start_send`] but accepts a [`EthBroadcastMessage`] instead.
    #[inline]
    pub fn start_send_broadcast(
        &mut self,
        item: EthBroadcastMessage<T>,
    ) -> Result<(), EthStreamError> {
        match self {
            Self::EthOnly(conn) => conn.start_send_broadcast(item),
            Self::Satellite(conn) => conn.primary_mut().start_send_broadcast(item),
        }
    }
}

impl<T: NetworkTypes> From<EthPeerConnection<T>> for EthRlpxConnection<T> {
    #[inline]
    fn from(conn: EthPeerConnection<T>) -> Self {
        Self::EthOnly(Box::new(conn))
    }
}

impl<T: NetworkTypes> From<EthSatelliteConnection<T>> for EthRlpxConnection<T> {
    #[inline]
    fn from(conn: EthSatelliteConnection<T>) -> Self {
        Self::Satellite(Box::new(conn))
    }
}

macro_rules! delegate_call {
    ($self:ident.$method:ident($($args:ident),+)) => {
        unsafe {
            match $self.get_unchecked_mut() {
                Self::EthOnly(l) => Pin::new_unchecked(l).$method($($args),+),
                Self::Satellite(r) => Pin::new_unchecked(r).$method($($args),+),
            }
        }
    }
}

impl<T: NetworkTypes> Stream for EthRlpxConnection<T> {
    type Item = Result<EthMessage<T>, EthStreamError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        delegate_call!(self.poll_next(cx))
    }
}

impl<T: NetworkTypes> Sink<EthMessage<T>> for EthRlpxConnection<T> {
    type Error = EthStreamError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        delegate_call!(self.poll_ready(cx))
    }

    fn start_send(self: Pin<&mut Self>, item: EthMessage<T>) -> Result<(), Self::Error> {
        delegate_call!(self.start_send(item))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        delegate_call!(self.poll_flush(cx))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        delegate_call!(self.poll_close(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn assert_eth_stream<St>()
    where
        St: Stream<Item = Result<EthMessage, EthStreamError>> + Sink<EthMessage>,
    {
    }

    #[test]
    const fn test_eth_stream_variants() {
        assert_eth_stream::<EthSatelliteConnection>();
        assert_eth_stream::<EthRlpxConnection>();
    }
}
