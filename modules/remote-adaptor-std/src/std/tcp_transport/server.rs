//! TCP accept loop.

use alloc::string::String;
use core::fmt::{Debug, Formatter, Result as FmtResult};
use std::net::{SocketAddr, TcpListener as StdTcpListener};

use fraktor_remote_core_rs::core::transport::TransportError;
use futures::{SinkExt as _, StreamExt as _};
use tokio::{
  net::{TcpListener, TcpStream},
  runtime::Handle,
  sync::mpsc::UnboundedSender,
  task::JoinHandle,
};
use tokio_util::codec::Framed;

use crate::std::tcp_transport::{frame_codec::WireFrameCodec, inbound_frame_event::InboundFrameEvent};

/// Owns a `tokio::net::TcpListener` and drives an accept loop that spawns a
/// reader task for every accepted connection.
///
/// Each reader task reads [`crate::std::tcp_transport::WireFrame`]s through a
/// `Framed` stream and forwards them to the shared inbound channel owned
/// by the transport.
pub struct TcpServer {
  bind_addr:   String,
  frame_codec: WireFrameCodec,
  accept_task: Option<JoinHandle<()>>,
}

impl Debug for TcpServer {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("TcpServer")
      .field("bind_addr", &self.bind_addr)
      .field("running", &self.accept_task.is_some())
      .finish()
  }
}

impl TcpServer {
  /// Creates a new [`TcpServer`] that will bind to `bind_addr` on `start`.
  #[must_use]
  pub const fn new(bind_addr: String) -> Self {
    Self { bind_addr, frame_codec: WireFrameCodec::new(), accept_task: None }
  }

  /// Creates a new [`TcpServer`] with the given frame codec.
  #[must_use]
  pub(crate) const fn with_frame_codec(bind_addr: String, frame_codec: WireFrameCodec) -> Self {
    Self { bind_addr, frame_codec, accept_task: None }
  }

  /// Returns `true` when the server is currently running.
  #[must_use]
  pub const fn is_running(&self) -> bool {
    self.accept_task.is_some()
  }

  /// Binds the listener and spawns the accept loop task.
  ///
  /// Inbound frames are forwarded to `inbound_tx`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotAvailable`] if no Tokio runtime is available,
  /// or [`TransportError::SendFailed`] if the listener cannot be bound.
  pub fn start(&mut self, inbound_tx: UnboundedSender<InboundFrameEvent>) -> Result<SocketAddr, TransportError> {
    if self.accept_task.is_some() {
      return Err(TransportError::AlreadyRunning);
    }
    let handle = Handle::try_current().map_err(|_| TransportError::NotAvailable)?;
    let listener = StdTcpListener::bind(&self.bind_addr).map_err(|_| TransportError::SendFailed)?;
    let bound_addr = listener.local_addr().map_err(|_| TransportError::SendFailed)?;
    listener.set_nonblocking(true).map_err(|_| TransportError::SendFailed)?;
    let listener = TcpListener::from_std(listener).map_err(|_| TransportError::SendFailed)?;
    let frame_codec = self.frame_codec;
    let task = handle.spawn(async move {
      loop {
        match listener.accept().await {
          | Ok((stream, peer)) => {
            let inbound_tx = inbound_tx.clone();
            let peer_addr = peer.to_string();
            tokio::spawn(read_loop(stream, peer_addr, inbound_tx, frame_codec));
          },
          | Err(err) => {
            tracing::warn!(?err, "tcp accept loop failed");
            break;
          },
        }
      }
    });
    self.accept_task = Some(task);
    Ok(bound_addr)
  }

  /// Stops the accept loop task, aborting any in-flight accept.
  pub fn shutdown(&mut self) {
    if let Some(handle) = self.accept_task.take() {
      handle.abort();
    }
  }
}

async fn read_loop(
  stream: TcpStream,
  peer: String,
  inbound_tx: UnboundedSender<InboundFrameEvent>,
  frame_codec: WireFrameCodec,
) {
  let mut framed = Framed::new(stream, frame_codec);
  while let Some(next) = framed.next().await {
    match next {
      | Ok(frame) => {
        if inbound_tx.send(InboundFrameEvent { peer: peer.clone(), frame }).is_err() {
          // Receiver dropped — the transport is shutting down.
          break;
        }
      },
      | Err(err) => {
        tracing::warn!(?err, peer = %peer, "tcp frame decode error");
        break;
      },
    }
  }
  // write buffer を flush し、peer が end-of-stream を観測できるよう half-close を明示する。
  // shutdown 経路なので close 失敗は非致命として debug ログに留める。
  if let Err(err) = framed.close().await {
    tracing::debug!(?err, peer = %peer, "tcp server framed close failed during shutdown");
  }
}
