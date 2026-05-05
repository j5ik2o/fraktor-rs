//! TCP accept loop.

use alloc::{string::String, sync::Arc, vec::Vec};
use core::fmt::{Debug, Formatter, Result as FmtResult};
use std::{
  net::{SocketAddr, TcpListener as StdTcpListener},
  sync::Mutex,
};

use fraktor_remote_core_rs::core::transport::TransportError;
use futures::{SinkExt as _, StreamExt as _};
use tokio::{
  net::{TcpListener, TcpStream},
  runtime::Handle,
  sync::mpsc::UnboundedSender,
  task::JoinHandle,
};
use tokio_util::codec::Framed;

use super::{frame_codec::WireFrameCodec, inbound_frame_event::InboundFrameEvent};

type ConnectionTasks = Arc<Mutex<Vec<JoinHandle<()>>>>;

/// Owns a `tokio::net::TcpListener` and drives an accept loop that spawns a
/// reader task for every accepted connection.
///
/// Each reader task reads [`crate::std::transport::tcp::WireFrame`]s through a
/// `Framed` stream and forwards them to the shared inbound channel owned
/// by the transport.
pub struct TcpServer {
  bind_addr:        String,
  frame_codec:      WireFrameCodec,
  accept_task:      Option<JoinHandle<()>>,
  connection_tasks: ConnectionTasks,
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
  pub fn new(bind_addr: String) -> Self {
    Self {
      bind_addr,
      frame_codec: WireFrameCodec::new(),
      accept_task: None,
      connection_tasks: Arc::new(Mutex::new(Vec::new())),
    }
  }

  /// Creates a new [`TcpServer`] with the given frame codec.
  #[must_use]
  pub(crate) fn with_frame_codec(bind_addr: String, frame_codec: WireFrameCodec) -> Self {
    Self { bind_addr, frame_codec, accept_task: None, connection_tasks: Arc::new(Mutex::new(Vec::new())) }
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
    let connection_tasks = self.connection_tasks.clone();
    let task = handle.spawn(async move {
      loop {
        match listener.accept().await {
          | Ok((stream, peer)) => {
            let inbound_tx = inbound_tx.clone();
            let peer_addr = peer.to_string();
            let connection = tokio::spawn(read_loop(stream, peer_addr, inbound_tx, frame_codec));
            // 接続ごとの read_loop ハンドルを共有 Vec に蓄積し、 shutdown() から abort できるようにする。
            // 終了済みハンドルはここでまとめて掃除し、長時間 accept を続けても無制限には膨れないようにする。
            match connection_tasks.lock() {
              | Ok(mut tasks) => {
                tasks.retain(|task| !task.is_finished());
                tasks.push(connection);
              },
              | Err(err) => {
                tracing::warn!(?err, "tcp accept loop could not register connection task");
              },
            }
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

  /// Stops the accept loop task and aborts every accepted connection.
  pub fn shutdown(&mut self) {
    if let Some(handle) = self.accept_task.take() {
      handle.abort();
    }
    if let Ok(mut tasks) = self.connection_tasks.lock() {
      for task in tasks.drain(..) {
        task.abort();
      }
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
      | Ok(decoded) => {
        if inbound_tx.send(InboundFrameEvent { peer: peer.clone(), frame: decoded }).is_err() {
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
