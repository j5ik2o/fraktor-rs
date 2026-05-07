//! Single outbound TCP connection with its reader / writer tasks.

use alloc::string::String;
use core::fmt::{Debug, Formatter, Result as FmtResult};
use std::{net::TcpStream as StdTcpStream, time::Instant};

use fraktor_remote_core_rs::core::{
  extension::RemoteEvent,
  transport::{TransportEndpoint, TransportError},
};
use futures::{SinkExt as _, StreamExt as _};
use tokio::{
  net::TcpStream,
  sync::mpsc::{self, Sender, UnboundedReceiver, UnboundedSender},
  task::JoinHandle,
};
use tokio_util::codec::Framed;

use super::{
  WireFrame, connection_loss_reporter::ConnectionLossReporter, frame_codec::WireFrameCodec,
  inbound_frame_event::InboundFrameEvent,
};
use crate::std::association::authority_for_frame;

/// Single outbound TCP connection towards a remote authority.
///
/// Owns a writer channel used by the synchronous
/// `fraktor_remote_core_rs::core::transport::RemoteTransport::send` entry point,
/// and a background tokio task that drains the channel and writes frames to
/// the socket. The same task also reads inbound frames and forwards them to
/// the shared inbound channel owned by the transport.
pub struct TcpClient {
  peer_addr: String,
  writer_tx: UnboundedSender<WireFrame>,
  task:      Option<JoinHandle<()>>,
}

impl Debug for TcpClient {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("TcpClient")
      .field("peer_addr", &self.peer_addr)
      .field("alive", &self.task.as_ref().is_some_and(|t| !t.is_finished()))
      .finish()
  }
}

impl TcpClient {
  /// Connects to `peer_addr` and spawns the reader / writer task.
  ///
  /// Received frames are forwarded to `inbound_tx`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::SendFailed`] if the TCP connection cannot be
  /// established.
  pub async fn connect(
    peer_addr: String,
    inbound_tx: UnboundedSender<InboundFrameEvent>,
  ) -> Result<Self, TransportError> {
    let stream = TcpStream::connect(&peer_addr).await.map_err(|_| TransportError::SendFailed)?;
    Ok(Self::from_connected_stream(stream, peer_addr, inbound_tx, WireFrameCodec::new(), None))
  }

  /// Connects to `peer_addr` using the given frame codec.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::SendFailed`] if the TCP connection cannot be
  /// established.
  pub(crate) async fn connect_with_frame_codec(
    peer_addr: String,
    inbound_tx: UnboundedSender<InboundFrameEvent>,
    frame_codec: WireFrameCodec,
  ) -> Result<Self, TransportError> {
    let stream = TcpStream::connect(&peer_addr).await.map_err(|_| TransportError::SendFailed)?;
    Ok(Self::from_connected_stream(stream, peer_addr, inbound_tx, frame_codec, None))
  }

  pub(crate) fn connect_blocking_with_frame_codec(
    peer_addr: String,
    inbound_tx: UnboundedSender<InboundFrameEvent>,
    frame_codec: WireFrameCodec,
  ) -> Result<Self, TransportError> {
    let stream = connected_tokio_stream(&peer_addr)?;
    Ok(Self::from_connected_stream(stream, peer_addr, inbound_tx, frame_codec, None))
  }

  pub(crate) async fn connect_with_connection_loss_reporter(
    peer_addr: String,
    inbound_tx: UnboundedSender<InboundFrameEvent>,
    frame_codec: WireFrameCodec,
    event_sender: Sender<RemoteEvent>,
    authority: TransportEndpoint,
    monotonic_epoch: Instant,
  ) -> Result<Self, TransportError> {
    let stream = TcpStream::connect(&peer_addr).await.map_err(|_| TransportError::SendFailed)?;
    let reporter = ConnectionLossReporter::new(event_sender, authority, monotonic_epoch);
    Ok(Self::from_connected_stream(stream, peer_addr, inbound_tx, frame_codec, Some(reporter)))
  }

  pub(crate) fn connect_blocking_with_connection_loss_reporter(
    peer_addr: String,
    inbound_tx: UnboundedSender<InboundFrameEvent>,
    frame_codec: WireFrameCodec,
    event_sender: Sender<RemoteEvent>,
    authority: TransportEndpoint,
    monotonic_epoch: Instant,
  ) -> Result<Self, TransportError> {
    let stream = connected_tokio_stream(&peer_addr)?;
    let reporter = ConnectionLossReporter::new(event_sender, authority, monotonic_epoch);
    Ok(Self::from_connected_stream(stream, peer_addr, inbound_tx, frame_codec, Some(reporter)))
  }

  fn from_connected_stream(
    stream: TcpStream,
    peer_addr: String,
    inbound_tx: UnboundedSender<InboundFrameEvent>,
    frame_codec: WireFrameCodec,
    connection_loss_reporter: Option<ConnectionLossReporter>,
  ) -> Self {
    let (writer_tx, writer_rx) = mpsc::unbounded_channel::<WireFrame>();
    let peer_for_task = peer_addr.clone();
    let task = tokio::spawn(run(stream, peer_for_task, writer_rx, inbound_tx, frame_codec, connection_loss_reporter));
    Self { peer_addr, writer_tx, task: Some(task) }
  }

  /// Returns the peer address this client is connected to.
  #[must_use]
  pub fn peer_addr(&self) -> &str {
    &self.peer_addr
  }

  /// Enqueues a frame for writing without blocking the caller.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::ConnectionClosed`] if the writer task has
  /// already exited.
  pub fn send(&self, frame: WireFrame) -> Result<(), TransportError> {
    self.writer_tx.send(frame).map_err(|_| TransportError::ConnectionClosed)
  }

  /// Aborts the reader / writer task.
  pub fn shutdown(&mut self) {
    if let Some(handle) = self.task.take() {
      handle.abort();
    }
  }
}

fn connected_tokio_stream(peer_addr: &str) -> Result<TcpStream, TransportError> {
  let stream = StdTcpStream::connect(peer_addr).map_err(|_| TransportError::SendFailed)?;
  stream.set_nonblocking(true).map_err(|_| TransportError::SendFailed)?;
  TcpStream::from_std(stream).map_err(|_| TransportError::SendFailed)
}

async fn run(
  stream: TcpStream,
  peer_addr: String,
  mut writer_rx: UnboundedReceiver<WireFrame>,
  inbound_tx: UnboundedSender<InboundFrameEvent>,
  frame_codec: WireFrameCodec,
  connection_loss_reporter: Option<ConnectionLossReporter>,
) {
  let mut framed = Framed::new(stream, frame_codec);
  let mut authority = None;
  let exit_cause = loop {
    tokio::select! {
      next = framed.next() => match next {
        | Some(Ok(decoded)) => {
          if let Some(frame_authority) = authority_for_frame(&decoded) {
            authority = Some(frame_authority);
          }
          if inbound_tx.send(InboundFrameEvent {
            peer: peer_addr.clone(),
            authority: authority.clone(),
            frame: decoded,
          }).is_err() {
            break None;
          }
        }
        | Some(Err(err)) => {
          tracing::warn!(?err, peer = %peer_addr, "tcp client decode error");
          break Some(TransportError::SendFailed);
        }
        | None => break Some(TransportError::ConnectionClosed),
      },
      next = writer_rx.recv() => match next {
        | Some(frame) => {
          if let Err(err) = framed.send(frame).await {
            tracing::warn!(?err, peer = %peer_addr, "tcp client write error");
            break Some(TransportError::SendFailed);
          }
        }
        | None => break None,
      },
    }
  };
  if let (Some(cause), Some(reporter)) = (exit_cause, connection_loss_reporter) {
    reporter.report(cause).await;
  }
  if let Err(err) = framed.close().await {
    tracing::debug!(?err, "tcp client framed close failed during shutdown");
  }
}
