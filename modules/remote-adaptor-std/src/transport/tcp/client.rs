//! Single outbound TCP connection with its reader / writer tasks.

#[cfg(test)]
#[path = "client_test.rs"]
mod tests;

use alloc::{string::String, vec::Vec};
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  task::Poll,
};
use std::time::Instant;

use fraktor_remote_core_rs::{
  config::RemoteCompressionConfig,
  extension::RemoteEvent,
  transport::{TransportEndpoint, TransportError},
  wire::CompressionTableKind,
};
use futures::{SinkExt as _, StreamExt as _, future::poll_fn};
use tokio::{
  net::TcpStream,
  runtime::Handle,
  sync::mpsc::{self, Receiver, Sender, UnboundedSender, error::TrySendError},
  task::JoinHandle,
};
use tokio_util::codec::Framed;

use super::{
  WireFrame,
  compression::{
    InboundCompressionAction, TcpCompressionTables, compression_advertisement_interval,
    next_compression_advertisement_tick,
  },
  connection_loss_reporter::ConnectionLossReporter,
  frame_codec::WireFrameCodec,
  inbound_frame_event::InboundFrameEvent,
};
use crate::association::authority_for_frame;

const WRITER_QUEUE_CAPACITY: usize = 1024;

/// Single outbound TCP connection towards a remote authority.
///
/// Owns a writer channel used by the synchronous
/// `fraktor_remote_core_rs::transport::RemoteTransport::send` entry point,
/// and a background tokio task that drains the channel and writes frames to
/// the socket. The same task also reads inbound frames and forwards them to
/// the shared inbound channel owned by the transport.
pub struct TcpClient {
  peer_addr:  String,
  writer_txs: Vec<Sender<WireFrame>>,
  task:       Option<JoinHandle<()>>,
}

pub(crate) struct TcpClientConnectOptions {
  frame_codec:        WireFrameCodec,
  outbound_lanes:     usize,
  compression_config: RemoteCompressionConfig,
  local_authority:    String,
  reporter:           Option<TcpClientConnectionLossReporterOptions>,
}

struct TcpClientConnectionLossReporterOptions {
  event_sender:    Sender<RemoteEvent>,
  authority:       TransportEndpoint,
  monotonic_epoch: Instant,
}

struct TcpClientRunOptions {
  frame_codec:              WireFrameCodec,
  compression_config:       RemoteCompressionConfig,
  local_authority:          String,
  connection_loss_reporter: Option<ConnectionLossReporter>,
}

impl TcpClientConnectOptions {
  pub(crate) const fn new(frame_codec: WireFrameCodec) -> Self {
    Self {
      frame_codec,
      outbound_lanes: 1,
      compression_config: RemoteCompressionConfig::new(),
      local_authority: String::new(),
      reporter: None,
    }
  }

  pub(crate) const fn with_outbound_lanes(mut self, outbound_lanes: usize) -> Self {
    assert!(outbound_lanes > 0, "outbound lanes must be greater than zero");
    self.outbound_lanes = outbound_lanes;
    self
  }

  pub(crate) fn with_connection_loss_reporter(
    mut self,
    event_sender: Sender<RemoteEvent>,
    authority: TransportEndpoint,
    monotonic_epoch: Instant,
  ) -> Self {
    self.reporter = Some(TcpClientConnectionLossReporterOptions { event_sender, authority, monotonic_epoch });
    self
  }

  pub(crate) fn with_compression_config(
    mut self,
    compression_config: RemoteCompressionConfig,
    local_authority: String,
  ) -> Self {
    self.compression_config = compression_config;
    self.local_authority = local_authority;
    self
  }

  fn into_run_options(self) -> TcpClientRunOptions {
    let connection_loss_reporter = self
      .reporter
      .map(|options| ConnectionLossReporter::new(options.event_sender, options.authority, options.monotonic_epoch));
    TcpClientRunOptions {
      frame_codec: self.frame_codec,
      compression_config: self.compression_config,
      local_authority: self.local_authority,
      connection_loss_reporter,
    }
  }
}

impl Debug for TcpClient {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("TcpClient")
      .field("peer_addr", &self.peer_addr)
      .field("alive", &self.task.as_ref().is_some_and(|t| !t.is_finished()))
      .field("writer_lanes", &self.writer_txs.len())
      .finish()
  }
}

impl TcpClient {
  /// Creates a client whose background task establishes the TCP connection
  /// asynchronously before draining queued outbound frames.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotAvailable`] when no Tokio runtime is
  /// available to drive the connection task.
  pub(crate) fn connect(
    peer_addr: String,
    inbound_txs: Vec<UnboundedSender<InboundFrameEvent>>,
    options: TcpClientConnectOptions,
  ) -> Result<Self, TransportError> {
    let handle = Handle::try_current().map_err(|_| TransportError::NotAvailable)?;
    let outbound_lanes = options.outbound_lanes;
    let mut writer_txs = Vec::with_capacity(outbound_lanes);
    let mut writer_rxs = Vec::with_capacity(outbound_lanes);
    for _ in 0..outbound_lanes {
      let (writer_tx, writer_rx) = mpsc::channel::<WireFrame>(WRITER_QUEUE_CAPACITY);
      writer_txs.push(writer_tx);
      writer_rxs.push(writer_rx);
    }
    let peer_for_task = peer_addr.clone();
    let task = handle.spawn(connect_and_run(peer_for_task, writer_rxs, inbound_txs, options));
    Ok(Self { peer_addr, writer_txs, task: Some(task) })
  }

  /// Enqueues a frame for writing without blocking the caller.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::Backpressure`] if the bounded writer queue is full,
  /// or [`TransportError::ConnectionClosed`] if the writer task has exited.
  pub fn send(&self, frame: WireFrame) -> Result<(), TransportError> {
    self.send_to_lane(0, frame)
  }

  /// Enqueues a frame into the lane selected by `lane_key`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::Backpressure`] if the selected lane queue is
  /// full, or [`TransportError::ConnectionClosed`] if the writer task has exited.
  pub(crate) fn send_with_lane_key(&self, lane_key: &[u8], frame: WireFrame) -> Result<(), TransportError> {
    self.send_to_lane(writer_lane_index(lane_key, self.writer_txs.len()), frame)
  }

  /// Enqueues a frame into the given writer lane.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotAvailable`] when the lane id does not exist,
  /// [`TransportError::Backpressure`] when the selected queue is full,
  /// or [`TransportError::ConnectionClosed`] if the writer task has exited.
  pub(crate) fn send_to_lane_id(&self, lane_id: u32, frame: WireFrame) -> Result<(), TransportError> {
    let lane_index = lane_id as usize;
    if lane_index >= self.writer_txs.len() {
      return Err(TransportError::NotAvailable);
    }
    self.send_to_lane(lane_index, frame)
  }

  fn send_to_lane(&self, lane_index: usize, frame: WireFrame) -> Result<(), TransportError> {
    let Some(writer_tx) = self.writer_txs.get(lane_index) else {
      return Err(TransportError::ConnectionClosed);
    };
    writer_tx.try_send(frame).map_err(|error| match error {
      | TrySendError::Full(_) => TransportError::Backpressure,
      | TrySendError::Closed(_) => TransportError::ConnectionClosed,
    })
  }

  pub(crate) fn is_alive(&self) -> bool {
    self.task.as_ref().is_some_and(|handle| !handle.is_finished())
  }

  /// Aborts the reader / writer task.
  pub fn shutdown(&mut self) {
    if let Some(handle) = self.task.take() {
      handle.abort();
    }
  }
}

async fn connect_and_run(
  peer_addr: String,
  writer_rxs: Vec<Receiver<WireFrame>>,
  inbound_txs: Vec<UnboundedSender<InboundFrameEvent>>,
  options: TcpClientConnectOptions,
) {
  let run_options = options.into_run_options();
  match TcpStream::connect(&peer_addr).await {
    | Ok(stream) => {
      run(stream, peer_addr, writer_rxs, inbound_txs, run_options).await;
    },
    | Err(err) => {
      tracing::warn!(?err, peer = %peer_addr, "tcp client connect error");
      if let Some(reporter) = run_options.connection_loss_reporter {
        reporter.report(TransportError::SendFailed).await;
      }
    },
  }
}

async fn run(
  stream: TcpStream,
  peer_addr: String,
  mut writer_rxs: Vec<Receiver<WireFrame>>,
  inbound_txs: Vec<UnboundedSender<InboundFrameEvent>>,
  options: TcpClientRunOptions,
) {
  let frame_codec = options.frame_codec;
  let compression_config = options.compression_config;
  let local_authority = options.local_authority;
  let connection_loss_reporter = options.connection_loss_reporter;
  let mut framed = Framed::new(stream, frame_codec);
  let mut authority = None;
  let mut next_writer_lane = 0;
  let mut compression_tables = TcpCompressionTables::new(compression_config);
  let mut actor_ref_advertisement_interval = compression_advertisement_interval(
    compression_config.actor_ref_max(),
    compression_config.actor_ref_advertisement_interval(),
  );
  let mut manifest_advertisement_interval = compression_advertisement_interval(
    compression_config.manifest_max(),
    compression_config.manifest_advertisement_interval(),
  );
  let exit_cause = loop {
    tokio::select! {
      next = framed.next() => match next {
        | Some(Ok(decoded)) => {
          let decoded = match compression_tables.handle_inbound_frame(decoded, &local_authority) {
            | Ok(InboundCompressionAction::Forward(frame)) => frame,
            | Ok(InboundCompressionAction::Reply { pdu, authority: frame_authority }) => {
              authority = Some(frame_authority);
              if let Err(err) = framed.send(WireFrame::Control(pdu)).await {
                tracing::warn!(?err, peer = %peer_addr, "tcp client compression ack write error");
                break Some(TransportError::SendFailed);
              }
              continue;
            },
            | Ok(InboundCompressionAction::Consumed { authority: frame_authority }) => {
              authority = Some(frame_authority);
              continue;
            },
            | Err(err) => {
              tracing::warn!(?err, peer = %peer_addr, "tcp client compression frame error");
              break Some(TransportError::SendFailed);
            },
          };
          if let Some(frame_authority) = authority_for_frame(&decoded) {
            authority = Some(frame_authority);
          }
          let lane_index = inbound_lane_index(&peer_addr, authority.as_ref(), &decoded, inbound_txs.len());
          let Some(inbound_tx) = inbound_txs.get(lane_index) else {
            break Some(TransportError::NotAvailable);
          };
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
      next = next_writer_frame(&mut writer_rxs, &mut next_writer_lane) => match next {
        | Some(frame) => {
          let frame = compression_tables.apply_outbound_frame(frame);
          if let Err(err) = framed.send(frame).await {
            tracing::warn!(?err, peer = %peer_addr, "tcp client write error");
            break Some(TransportError::SendFailed);
          }
        }
        | None => break None,
      },
      _ = next_compression_advertisement_tick(&mut actor_ref_advertisement_interval) => {
        if let Some(frame) = compression_tables.create_advertisement(CompressionTableKind::ActorRef, &local_authority)
          && let Err(err) = framed.send(frame).await
        {
          tracing::warn!(?err, peer = %peer_addr, "tcp client actor-ref compression advertisement write error");
          break Some(TransportError::SendFailed);
        }
      },
      _ = next_compression_advertisement_tick(&mut manifest_advertisement_interval) => {
        if let Some(frame) = compression_tables.create_advertisement(CompressionTableKind::Manifest, &local_authority)
          && let Err(err) = framed.send(frame).await
        {
          tracing::warn!(?err, peer = %peer_addr, "tcp client manifest compression advertisement write error");
          break Some(TransportError::SendFailed);
        }
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

async fn next_writer_frame(writer_rxs: &mut [Receiver<WireFrame>], next_writer_lane: &mut usize) -> Option<WireFrame> {
  poll_fn(|cx| {
    if writer_rxs.is_empty() {
      return Poll::Ready(None);
    }
    let mut has_open_idle_lane = false;
    for offset in 0..writer_rxs.len() {
      let lane_index = (*next_writer_lane + offset) % writer_rxs.len();
      match writer_rxs[lane_index].poll_recv(cx) {
        | Poll::Ready(Some(frame)) => {
          *next_writer_lane = (lane_index + 1) % writer_rxs.len();
          return Poll::Ready(Some(frame));
        },
        | Poll::Ready(None) => {},
        | Poll::Pending => has_open_idle_lane = true,
      }
    }
    if has_open_idle_lane { Poll::Pending } else { Poll::Ready(None) }
  })
  .await
}

pub(crate) fn writer_lane_index(lane_key: &[u8], lane_count: usize) -> usize {
  lane_index(lane_key, lane_count)
}

pub(crate) fn inbound_lane_index(
  peer: &str,
  authority: Option<&TransportEndpoint>,
  frame: &WireFrame,
  lane_count: usize,
) -> usize {
  if let Some(authority) = authority {
    return lane_index(authority.authority().as_bytes(), lane_count);
  }
  if let Some(authority) = authority_for_frame(frame) {
    return lane_index(authority.authority().as_bytes(), lane_count);
  }
  lane_index(peer.as_bytes(), lane_count)
}

fn lane_index(key: &[u8], lane_count: usize) -> usize {
  if lane_count <= 1 {
    return 0;
  }
  let mut hash = 14_695_981_039_346_656_037_u64;
  for byte in key {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(1_099_511_628_211);
  }
  (hash % lane_count as u64) as usize
}
