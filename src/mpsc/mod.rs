use crate::{
    channel, multi_channel, spsc,
    util::{
        codec::{DecodeMethod, EncodeMethod},
        split::{RWSplit, TcpSplit},
    },
};
use bytes::BytesMut;
use errors::*;
use futures::{ready, Future, Sink, SinkExt, Stream, StreamExt};
use snafu::{Backtrace, ResultExt};
use std::{
    fmt,
    net::{SocketAddr, ToSocketAddrs},
    task::Poll,
};
use tokio::io::{AsyncRead, AsyncWrite};

pub mod builder;

#[cfg(feature = "json")]
pub type JsonSender<T> = Sender<T, crate::util::codec::JsonCodec>;

#[cfg(feature = "json")]
pub type JsonReceiver<T, const N: usize = 0> = Receiver<T, crate::util::codec::JsonCodec, N>;

#[cfg(feature = "protobuf")]
pub type ProtobufSender<T> = Sender<T, crate::util::codec::ProtobufCodec>;

#[cfg(feature = "protobuf")]
pub type ProtobufReceiver<T, const N: usize = 0> =
    Receiver<T, crate::util::codec::ProtobufCodec, N>;

#[cfg(feature = "rkyv")]
pub type RkyvSender<T, const N: usize = 0> = Sender<T, crate::util::codec::RkyvCodec>;

#[cfg(feature = "rkyv")]
pub type RkyvReceiver<T, const N: usize = 0> = Receiver<T, crate::util::codec::RkyvCodec, N>;

pub fn send_to<A: 'static + Clone + Send + ToSocketAddrs, T, E>(
    dest: A,
) -> builder::SenderBuilderFuture<
    A,
    T,
    E,
    TcpSplit,
    impl Future<Output = channel::builder::BuildResult<TcpSplit>>,
    impl Clone + Fn(SocketAddr) -> bool,
> {
    builder::new_sender(dest)
}

pub fn recv_on<A: 'static + Clone + Send + ToSocketAddrs, T, E>(
    local_addr: A,
) -> builder::ReceiverBuilderFuture<
    A,
    T,
    E,
    TcpSplit,
    impl Future<Output = multi_channel::builder::AcceptResult>,
    impl Clone + Fn(SocketAddr) -> bool,
> {
    builder::new_receiver(local_addr)
}

#[pin_project::pin_project]
pub struct Sender<T: fmt::Debug, E, RW = TcpSplit>(#[pin] pub(crate) channel::Channel<T, E, RW>);

impl<T, E, RW> Sender<T, E, RW>
where
    T: fmt::Debug,
{
    pub fn local_addr(&self) -> &SocketAddr {
        &self.0.local_addr()
    }

    pub fn peer_addr(&self) -> &SocketAddr {
        &self.0.peer_addr()
    }
}

impl<T: fmt::Debug, E, RW> From<spsc::Sender<T, E, RW>> for Sender<T, E, RW> {
    fn from(sender: spsc::Sender<T, E, RW>) -> Self {
        Sender(sender.0)
    }
}

impl<T, E, R, W> Sender<T, E, RWSplit<R, W>>
where
    T: fmt::Debug,
{
    pub fn split(
        self,
    ) -> Result<(Self, spsc::Receiver<T, E, RWSplit<R, W>>), channel::errors::SplitError> {
        let (r, w) = self.0.split()?;

        Ok((w.into(), r))
    }
}

impl<T, E, RW> Sender<T, E, RW>
where
    T: 'static + fmt::Debug,
    E: 'static + EncodeMethod<T>,
    RW: AsyncWrite + Unpin,
{
    pub async fn send(&mut self, item: T) -> Result<(), SenderError<T, E>> {
        SinkExt::send(self, item).await
    }
}

impl<T, E, RW> Sink<T> for Sender<T, E, RW>
where
    T: 'static + fmt::Debug,
    E: 'static + EncodeMethod<T>,
    RW: AsyncWrite,
{
    type Error = SenderError<T, E>;

    fn start_send(self: std::pin::Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.project().0.start_send(item).context(SenderSnafu)
    }

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if let Err(e) = ready!(self.project().0.poll_ready(cx)) {
            return Poll::Ready(Err(e).context(SenderSnafu));
        }

        Poll::Ready(Ok(()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if let Err(e) = ready!(self.project().0.poll_flush(cx)) {
            return Poll::Ready(Err(e).context(SenderSnafu));
        }

        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        if let Err(e) = ready!(self.project().0.poll_close(cx)) {
            return Poll::Ready(Err(e).context(SenderSnafu));
        }

        Poll::Ready(Ok(()))
    }
}

#[derive(Debug)]
#[pin_project::pin_project]
pub struct Receiver<T, E, const N: usize = 0, RW = TcpSplit>(
    #[pin] multi_channel::Channel<T, E, N, RW>,
);

impl<T, E, const N: usize, RW> Receiver<T, E, N, RW> {
    pub(crate) fn from_channel(channel: multi_channel::Channel<T, E, N, RW>) -> Self {
        Self(channel)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn limit(&self) -> Option<usize> {
        self.0.limit()
    }

    pub fn local_addr(&self) -> &SocketAddr {
        self.0.local_addr()
    }

    pub fn peer_addrs(&self) -> Vec<SocketAddr> {
        self.0.peer_addrs()
    }
}

impl<T, E, const N: usize, R, W> Receiver<T, E, N, RWSplit<R, W>> {
    pub fn split(
        self,
    ) -> Result<
        (Self, crate::broadcast::Sender<T, E, N, RWSplit<R, W>>),
        multi_channel::errors::SplitError,
    > {
        let readhalf_is_listener = true;
        self.0.split(readhalf_is_listener)
    }
}

impl<T, E, const N: usize> Receiver<T, E, N> {
    pub async fn accept(&mut self) -> Result<SocketAddr, ReceiverAcceptingError<TcpSplit>> {
        self.0.accept().await.context(ReceiverAcceptingSnafu)
    }
}

impl<
        T: 'static + fmt::Debug,
        E: 'static + DecodeMethod<T>,
        const N: usize,
        RW: 'static + fmt::Debug + AsyncRead + Unpin,
    > Receiver<T, E, N, RW>
{
    pub async fn recv(&mut self) -> Option<Result<T, ReceiverError<T, E>>> {
        self.0.next().await.map(|res| res.context(ReceiverSnafu))
    }

    pub async fn recv_with_addr(&mut self) -> Option<(Result<T, ReceiverError<T, E>>, SocketAddr)> {
        self.0
            .recv_with_addr()
            .await
            .map(|(res, addr)| (res.context(ReceiverSnafu), addr))
    }

    pub async fn recv_frame(&mut self) -> Option<Result<BytesMut, ReceiverError<T, E>>> {
        self.0
            .recv_frame()
            .await
            .map(|res| res.context(ReceiverSnafu))
    }

    pub async fn recv_frame_with_addr(
        &mut self,
    ) -> Option<(Result<BytesMut, ReceiverError<T, E>>, SocketAddr)> {
        self.0
            .recv_frame_with_addr()
            .await
            .map(|(res, addr)| (res.context(ReceiverSnafu), addr))
    }
}

impl<
        T: 'static + fmt::Debug,
        E: 'static + DecodeMethod<T>,
        const N: usize,
        RW: 'static + fmt::Debug + AsyncRead + Unpin,
    > Stream for Receiver<T, E, N, RW>
{
    type Item = Result<T, ReceiverError<T, E>>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match ready!(self.project().0.poll_next(cx)) {
            Some(Ok(item)) => Poll::Ready(Some(Ok(item))),
            Some(Err(e)) => Poll::Ready(Some(Err(e).context(ReceiverSnafu))),
            None => Poll::Ready(None),
        }
    }
}

pub mod errors {
    use super::*;
    use snafu::Snafu;

    #[derive(Debug, Snafu)]
    #[snafu(display("[ReceiverAcceptingError] Failed to accept stream"))]
    #[snafu(visibility(pub(super)))]
    pub struct ReceiverAcceptingError<T: 'static + fmt::Debug> {
        source: multi_channel::errors::AcceptingError<T>,
        backtrace: Backtrace,
    }

    #[derive(Debug, Snafu)]
    #[snafu(display("[SenderError] Failed to send item on mpsc::Sender"))]
    #[snafu(visibility(pub(super)))]
    pub struct SenderError<T, E>
    where
        T: 'static + fmt::Debug,
        E: 'static + EncodeMethod<T>,
        E::Error: 'static + fmt::Debug + std::error::Error,
    {
        source: channel::errors::ChannelSinkError<T, E>,
        backtrace: Backtrace,
    }

    #[derive(Debug, Snafu)]
    #[snafu(display("[ReceiverError] Failed to receiver item on mpsc::Receiver"))]
    #[snafu(visibility(pub(super)))]
    pub struct ReceiverError<T, E>
    where
        T: 'static + fmt::Debug,
        E: 'static + DecodeMethod<T>,
        E::Error: 'static + fmt::Debug + std::error::Error,
    {
        source: multi_channel::errors::ChannelStreamError<T, E>,
        backtrace: Backtrace,
    }
}
