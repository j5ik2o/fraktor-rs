//! Single outbound TCP connection with its reader / writer tasks.

use alloc::string::String;
use core::fmt::{Debug, Formatter, Result as FmtResult};

use fraktor_remote_core_rs::core::transport::TransportError;
use futures::{SinkExt as _, StreamExt as _};
use tokio::{
  net::TcpStream,
  sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
  task::JoinHandle,
};
use tokio_util::codec::Framed;

use crate::std::tcp_transport::{
  frame_codec::WireFrameCodec, inbound_frame_event::InboundFrameEvent, wire_frame::WireFrame,
};

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
    let (writer_tx, writer_rx) = mpsc::unbounded_channel::<WireFrame>();
    let peer_for_task = peer_addr.clone();
    let task = tokio::spawn(run(stream, peer_for_task, writer_rx, inbound_tx));
    Ok(Self { peer_addr, writer_tx, task: Some(task) })
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

async fn run(
  stream: TcpStream,
  peer_addr: String,
  mut writer_rx: UnboundedReceiver<WireFrame>,
  inbound_tx: UnboundedSender<InboundFrameEvent>,
) {
  let mut framed = Framed::new(stream, WireFrameCodec::new());
  loop {
    tokio::select! {
      next = framed.next() => match next {
        | Some(Ok(frame)) => {
          if inbound_tx.send(InboundFrameEvent { peer: peer_addr.clone(), frame }).is_err() {
            break;
          }
        }
        | Some(Err(err)) => {
          tracing::warn!(?err, peer = %peer_addr, "tcp client decode error");
          break;
        }
        | None => break,
      },
      next = writer_rx.recv() => match next {
        | Some(frame) => {
          if let Err(err) = framed.send(frame).await {
            tracing::warn!(?err, peer = %peer_addr, "tcp client write error");
            break;
          }
        }
        | None => break,
      },
    }
  }
  if let Err(err) = framed.close().await {
    tracing::debug!(?err, "tcp client framed close failed during shutdown");
  }
}
