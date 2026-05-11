//! TCP accept loop.

use alloc::{string::String, sync::Arc, vec::Vec};
use core::fmt::{Debug, Formatter, Result as FmtResult};
use std::{
  net::{SocketAddr, TcpListener as StdTcpListener},
  sync::Mutex,
  time::Instant,
};

use fraktor_remote_core_rs::{extension::RemoteEvent, transport::TransportError};
use futures::{SinkExt as _, StreamExt as _};
use tokio::{
  net::{TcpListener, TcpStream},
  runtime::Handle,
  sync::mpsc::{Sender, UnboundedSender},
  task::JoinHandle,
};
use tokio_util::codec::Framed;

use super::{
  client::inbound_lane_index, connection_loss_reporter::ConnectionLossReporter, frame_codec::WireFrameCodec,
  inbound_frame_event::InboundFrameEvent,
};
use crate::association::authority_for_frame;

type ConnectionTasks = Arc<Mutex<Vec<JoinHandle<()>>>>;

/// Owns a `tokio::net::TcpListener` and drives an accept loop that spawns a
/// reader task for every accepted connection.
///
/// Each reader task reads [`crate::transport::tcp::WireFrame`]s through a
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
  /// Creates a new [`TcpServer`] with the given frame codec.
  #[must_use]
  pub(crate) fn with_frame_codec(bind_addr: String, frame_codec: WireFrameCodec) -> Self {
    Self { bind_addr, frame_codec, accept_task: None, connection_tasks: Arc::new(Mutex::new(Vec::new())) }
  }

  pub(crate) fn start_with_remote_events(
    &mut self,
    inbound_txs: Vec<UnboundedSender<InboundFrameEvent>>,
    remote_event_tx: Option<Sender<RemoteEvent>>,
    monotonic_epoch: Instant,
  ) -> Result<SocketAddr, TransportError> {
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
            let inbound_txs = inbound_txs.clone();
            let remote_event_tx = remote_event_tx.clone();
            let peer_addr = peer.to_string();
            let connection =
              tokio::spawn(read_loop(stream, peer_addr, inbound_txs, frame_codec, remote_event_tx, monotonic_epoch));
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
  inbound_txs: Vec<UnboundedSender<InboundFrameEvent>>,
  frame_codec: WireFrameCodec,
  remote_event_tx: Option<Sender<RemoteEvent>>,
  monotonic_epoch: Instant,
) {
  let mut framed = Framed::new(stream, frame_codec);
  let mut authority = None;
  let exit_cause = loop {
    match framed.next().await {
      | Some(Ok(decoded)) => {
        if let Some(frame_authority) = authority_for_frame(&decoded) {
          authority = Some(frame_authority);
        }
        let lane_index = inbound_lane_index(&peer, authority.as_ref(), &decoded, inbound_txs.len());
        let inbound_tx =
          inbound_txs.get(lane_index).expect("inbound_lane_index returns an index within the inbound_txs lane count");
        if inbound_tx
          .send(InboundFrameEvent { peer: peer.clone(), authority: authority.clone(), frame: decoded })
          .is_err()
        {
          // Receiver dropped — the transport is shutting down.
          break None;
        }
      },
      | Some(Err(err)) => {
        tracing::warn!(?err, peer = %peer, "tcp frame decode error");
        break Some(TransportError::SendFailed);
      },
      | None => break Some(TransportError::ConnectionClosed),
    }
  };
  if let (Some(cause), Some(authority), Some(sender)) = (exit_cause, authority, remote_event_tx) {
    ConnectionLossReporter::new(sender, authority, monotonic_epoch).report(cause).await;
  }
  // write buffer を flush し、peer が end-of-stream を観測できるよう half-close を明示する。
  // shutdown 経路なので close 失敗は非致命として debug ログに留める。
  if let Err(err) = framed.close().await {
    tracing::debug!(?err, peer = %peer, "tcp server framed close failed during shutdown");
  }
}
