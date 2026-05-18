//! `TcpRemoteTransport` — `std::net`-backed implementation of the core
//! [`RemoteTransport`] port.

#[cfg(test)]
#[path = "base_test.rs"]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  time::Duration,
};
use std::{collections::BTreeMap, time::Instant};

use bytes::Bytes;
use fraktor_actor_core_kernel_rs::serialization::{SerializationCallScope, SerializationExtensionShared};
use fraktor_remote_core_rs::{
  address::Address,
  association::QuarantineReason,
  config::{RemoteCompressionConfig, RemoteConfig},
  envelope::OutboundEnvelope,
  extension::RemoteEvent,
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  wire::{AckPdu, ControlPdu, EnvelopePayload, EnvelopePdu, HandshakePdu, RemoteDeploymentPdu},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess};
use tokio::{
  sync::mpsc::{self, Sender, UnboundedReceiver, UnboundedSender},
  task::JoinHandle,
  time::sleep,
};

use super::{
  WireFrame,
  client::{TcpClient, TcpClientConnectOptions},
  frame_codec::WireFrameCodec,
  inbound_frame_event::InboundFrameEvent,
  server::TcpServer,
};
use crate::association::{run_inbound_dispatch, std_instant_elapsed_millis};

/// TCP-backed implementation of [`RemoteTransport`].
///
/// This struct aggregates the adapter-owned TCP listener and outbound
/// connections. Inbound frames are driven by crate-internal workers and fed
/// into `remote-core` through its event port.
///
/// Note: the trait [`RemoteTransport::send`] is **synchronous**. Peer
/// connection setup registers a writer immediately and drives the actual TCP
/// connect on a Tokio task, so the core event loop never performs blocking
/// socket I/O. User envelope delivery uses actor-core serialization and fails
/// visibly instead of silently encoding unsupported payloads as empty bytes.
pub struct TcpRemoteTransport {
  configured_local_addresses: Vec<Address>,
  local_addresses:            Vec<Address>,
  default_address:            Option<Address>,
  bind_addr:                  String,
  frame_codec:                WireFrameCodec,
  server:                     TcpServer,
  clients:                    BTreeMap<String, TcpClient>,
  inbound_txs:                Vec<UnboundedSender<InboundFrameEvent>>,
  inbound_rxs:                Option<Vec<UnboundedReceiver<InboundFrameEvent>>>,
  remote_event_tx:            Option<Sender<RemoteEvent>>,
  monotonic_epoch:            Instant,
  inbound_workers:            Vec<JoinHandle<Result<(), TransportError>>>,
  inbound_lanes:              usize,
  outbound_lanes:             usize,
  compression_config:         RemoteCompressionConfig,
  serialization_extension:    Option<ArcShared<SerializationExtensionShared>>,
  running:                    bool,
}

impl Debug for TcpRemoteTransport {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_struct("TcpRemoteTransport")
      .field("bind_addr", &self.bind_addr)
      .field("running", &self.running)
      .field("clients", &self.clients.len())
      .finish_non_exhaustive()
  }
}

impl TcpRemoteTransport {
  /// Creates a new transport that will bind to `bind_addr` and advertise the
  /// given `local_addresses`.
  #[must_use]
  pub fn new(bind_addr: impl Into<String>, local_addresses: Vec<Address>) -> Self {
    let bind_addr = bind_addr.into();
    Self::with_frame_codec(bind_addr, local_addresses, WireFrameCodec::new())
  }

  /// Creates a new transport from [`RemoteConfig`].
  #[must_use]
  pub fn from_config(system_name: impl Into<String>, config: RemoteConfig) -> Self {
    let bind_host = match config.bind_hostname() {
      | Some(hostname) => hostname,
      | None => config.canonical_host(),
    };
    let bind_port = match config.bind_port() {
      | Some(port) => port,
      | None => match config.canonical_port() {
        | Some(port) => port,
        | None => 0,
      },
    };
    let advertised_port = match config.canonical_port() {
      | Some(port) => port,
      | None => bind_port,
    };
    let system_name = system_name.into();
    let bind_addr = alloc::format!("{bind_host}:{bind_port}");
    let local_addresses = vec![Address::new(system_name, config.canonical_host(), advertised_port)];
    let frame_codec = WireFrameCodec::with_maximum_frame_size(config.maximum_frame_size());
    let compression_config = *config.compression_config();
    Self::with_frame_codec_and_lanes(
      bind_addr,
      local_addresses,
      frame_codec,
      config.inbound_lanes(),
      config.outbound_lanes(),
      compression_config,
    )
  }

  fn with_frame_codec(bind_addr: String, local_addresses: Vec<Address>, frame_codec: WireFrameCodec) -> Self {
    Self::with_frame_codec_and_lanes(bind_addr, local_addresses, frame_codec, 1, 1, RemoteCompressionConfig::new())
  }

  fn with_frame_codec_and_lanes(
    bind_addr: String,
    local_addresses: Vec<Address>,
    frame_codec: WireFrameCodec,
    inbound_lanes: usize,
    outbound_lanes: usize,
    compression_config: RemoteCompressionConfig,
  ) -> Self {
    assert!(inbound_lanes > 0, "inbound lanes must be greater than zero");
    assert!(outbound_lanes > 0, "outbound lanes must be greater than zero");
    let (inbound_txs, inbound_rxs) = Self::inbound_channels(inbound_lanes);
    let default_address = local_addresses.first().cloned();
    Self {
      configured_local_addresses: local_addresses.clone(),
      local_addresses,
      default_address,
      server: TcpServer::with_frame_codec_and_compression_config(bind_addr.clone(), frame_codec, compression_config),
      bind_addr,
      frame_codec,
      clients: BTreeMap::new(),
      inbound_txs,
      inbound_rxs: Some(inbound_rxs),
      remote_event_tx: None,
      monotonic_epoch: Instant::now(),
      inbound_workers: Vec::new(),
      inbound_lanes,
      outbound_lanes,
      compression_config,
      serialization_extension: None,
      running: false,
    }
  }

  fn inbound_channels(
    inbound_lanes: usize,
  ) -> (Vec<UnboundedSender<InboundFrameEvent>>, Vec<UnboundedReceiver<InboundFrameEvent>>) {
    let mut inbound_txs = Vec::with_capacity(inbound_lanes);
    let mut inbound_rxs = Vec::with_capacity(inbound_lanes);
    for _ in 0..inbound_lanes {
      let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<InboundFrameEvent>();
      inbound_txs.push(inbound_tx);
      inbound_rxs.push(inbound_rx);
    }
    (inbound_txs, inbound_rxs)
  }

  fn reset_inbound_channel(&mut self) {
    let (inbound_txs, inbound_rxs) = Self::inbound_channels(self.inbound_lanes);
    self.inbound_txs = inbound_txs;
    self.inbound_rxs = Some(inbound_rxs);
  }

  /// Returns a copy that emits scheduled remote events through `sender`.
  #[must_use]
  pub(crate) fn with_remote_event_sender(mut self, sender: Sender<RemoteEvent>) -> Self {
    self.remote_event_tx = Some(sender);
    self
  }

  /// Returns a copy that uses the given monotonic epoch for all emitted remote event timestamps.
  #[must_use]
  pub(crate) fn with_monotonic_epoch(mut self, monotonic_epoch: Instant) -> Self {
    self.monotonic_epoch = monotonic_epoch;
    self
  }

  /// Returns a copy that serializes outbound payloads through `serialization_extension`.
  #[must_use]
  pub(crate) fn with_serialization_extension(
    mut self,
    serialization_extension: ArcShared<SerializationExtensionShared>,
  ) -> Self {
    self.serialization_extension = Some(serialization_extension);
    self
  }

  fn spawn_inbound_workers(&mut self) -> Result<(), TransportError> {
    let Some(event_sender) = self.remote_event_tx.clone() else {
      tracing::debug!("with_remote_event_sender was not called; inbound workers not spawned");
      return Ok(());
    };
    let Some(inbound_rxs) = self.inbound_rxs.take() else {
      tracing::debug!("inbound receivers were already consumed; inbound workers not spawned");
      return Err(TransportError::NotAvailable);
    };
    let monotonic_epoch = self.monotonic_epoch;
    self.inbound_workers = inbound_rxs
      .into_iter()
      .map(|inbound_rx| {
        let event_sender = event_sender.clone();
        tokio::spawn(async move {
          run_inbound_dispatch(inbound_rx, event_sender, move || std_instant_elapsed_millis(monotonic_epoch)).await
        })
      })
      .collect();
    Ok(())
  }

  fn connect_peer_writer(&mut self, remote: &Address) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_key = Self::peer_key_for_address(remote);
    if let Some(client) = self.clients.get_mut(&peer_key) {
      if client.is_alive() {
        return Ok(());
      }
      client.shutdown();
      self.clients.remove(&peer_key);
    }
    let connect_addr = alloc::format!("{}:{}", remote.host(), remote.port());
    let client = TcpClient::connect(connect_addr, self.inbound_txs.clone(), self.client_connect_options(remote))?;
    self.clients.insert(peer_key, client);
    Ok(())
  }

  fn client_connect_options(&self, remote: &Address) -> TcpClientConnectOptions {
    let options = TcpClientConnectOptions::new(self.frame_codec)
      .with_outbound_lanes(self.outbound_lanes)
      .with_compression_config(self.compression_config, self.local_authority());
    if let Some(event_sender) = self.remote_event_tx.clone() {
      options.with_connection_loss_reporter(
        event_sender,
        TransportEndpoint::new(remote.to_string()),
        self.monotonic_epoch,
      )
    } else {
      options
    }
  }

  fn apply_bound_port_to_advertised_addresses(&mut self, bound_port: u16) {
    self.local_addresses = self
      .configured_local_addresses
      .iter()
      .map(|address| {
        if address.port() == 0 { Address::new(address.system(), address.host(), bound_port) } else { address.clone() }
      })
      .collect();
    self.default_address = self.local_addresses.first().cloned();
  }

  fn local_authority(&self) -> String {
    self.default_address.as_ref().map(ToString::to_string).unwrap_or_default()
  }

  fn local_authority_from_addresses(addresses: &[Address], bound_port: u16) -> String {
    addresses
      .first()
      .map(|address| {
        if address.port() == 0 {
          Address::new(address.system(), address.host(), bound_port).to_string()
        } else {
          address.to_string()
        }
      })
      .unwrap_or_default()
  }

  fn peer_key_for_address(address: &Address) -> String {
    alloc::format!("{}:{}", address.host(), address.port())
  }

  /// Sends a handshake PDU to an already connected peer.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] when the transport has not been started,
  /// [`TransportError::Backpressure`] when the TCP client writer queue is full,
  /// or [`TransportError::ConnectionClosed`] when no TCP client is registered for `remote`.
  pub(crate) fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError> {
    self.send_wire_frame(remote, WireFrame::Handshake(pdu))
  }

  /// Sends a control PDU to an already connected peer.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] when the transport has not been started,
  /// [`TransportError::Backpressure`] when the TCP client writer queue is full,
  /// or [`TransportError::ConnectionClosed`] when no TCP client is registered for `remote`.
  pub(crate) fn send_control(&mut self, remote: &Address, pdu: ControlPdu) -> Result<(), TransportError> {
    self.send_wire_frame(remote, WireFrame::Control(pdu))
  }

  pub(crate) fn send_deployment(&mut self, remote: &Address, pdu: RemoteDeploymentPdu) -> Result<(), TransportError> {
    self.send_wire_frame(remote, WireFrame::Deployment(pdu))
  }

  pub(crate) fn send_flush_request(
    &mut self,
    remote: &Address,
    pdu: ControlPdu,
    lane_id: u32,
  ) -> Result<(), TransportError> {
    self.send_wire_frame_to_lane(remote, lane_id, WireFrame::Control(pdu))
  }

  pub(crate) fn send_ack(&mut self, remote: &Address, pdu: AckPdu) -> Result<(), TransportError> {
    self.send_wire_frame(remote, WireFrame::Ack(pdu))
  }

  fn send_wire_frame(&mut self, remote: &Address, frame: WireFrame) -> Result<(), TransportError> {
    self.send_wire_frame_to_client(remote, |client| client.send(frame))
  }

  fn send_wire_frame_with_lane_key(
    &mut self,
    remote: &Address,
    lane_key: &[u8],
    frame: WireFrame,
  ) -> Result<(), TransportError> {
    self.send_wire_frame_to_client(remote, |client| client.send_with_lane_key(lane_key, frame))
  }

  fn send_wire_frame_to_lane(
    &mut self,
    remote: &Address,
    lane_id: u32,
    frame: WireFrame,
  ) -> Result<(), TransportError> {
    self.send_wire_frame_to_client(remote, |client| client.send_to_lane_id(lane_id, frame))
  }

  fn send_wire_frame_to_client<F>(&mut self, remote: &Address, send: F) -> Result<(), TransportError>
  where
    F: FnOnce(&TcpClient) -> Result<(), TransportError>, {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_key = Self::peer_key_for_address(remote);
    let Some(client) = self.clients.get(&peer_key) else {
      return Err(TransportError::ConnectionClosed);
    };
    let result = send(client);
    if result.as_ref().err().is_some_and(|error| error == &TransportError::ConnectionClosed)
      && let Some(mut client) = self.clients.remove(&peer_key)
    {
      client.shutdown();
    }
    result
  }
}

pub(super) fn outbound_envelope_to_pdu(
  envelope: &OutboundEnvelope,
  serialization_extension: &SerializationExtensionShared,
) -> Result<EnvelopePdu, TransportError> {
  let serialized = serialization_extension
    .with_read(|extension| extension.serialize(envelope.message().payload(), SerializationCallScope::Remote))
    .map_err(|error| {
      tracing::debug!(?error, "outbound payload serialization failed");
      TransportError::SendFailed
    })?;
  Ok(EnvelopePdu::new(
    envelope.recipient().to_canonical_uri(),
    envelope.sender().map(|sender| sender.to_canonical_uri()),
    envelope.correlation_id().hi(),
    envelope.correlation_id().lo(),
    envelope.priority().to_wire(),
    EnvelopePayload::new(
      serialized.serializer_id().value(),
      serialized.manifest().map(ToString::to_string),
      Bytes::from(serialized.bytes().to_vec()),
    ),
  ))
  .map(|pdu| pdu.with_redelivery_sequence(envelope.redelivery_sequence()))
}

fn remote_address_from_envelope(envelope: &OutboundEnvelope) -> Result<Address, TransportError> {
  let remote_node = envelope.remote_node();
  let Some(port) = remote_node.port() else {
    return Err(TransportError::ConnectionClosed);
  };
  Ok(Address::new(remote_node.system(), remote_node.host(), port))
}

fn outbound_lane_key_for_envelope(envelope: &OutboundEnvelope) -> Vec<u8> {
  let mut key = Vec::new();
  key.extend_from_slice(envelope.recipient().to_canonical_uri().as_bytes());
  key.push(0);
  if let Some(sender) = envelope.sender() {
    key.extend_from_slice(sender.to_canonical_uri().as_bytes());
  }
  key.push(0);
  key
}

impl RemoteTransport for TcpRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    if self.running {
      return Err(TransportError::AlreadyRunning);
    }
    let configured_local_addresses = self.configured_local_addresses.clone();
    let bound_addr = self.server.start_with_remote_events(
      self.inbound_txs.clone(),
      self.remote_event_tx.clone(),
      self.monotonic_epoch,
      move |bound_port| Self::local_authority_from_addresses(&configured_local_addresses, bound_port),
    )?;
    self.apply_bound_port_to_advertised_addresses(bound_addr.port());
    self.running = true;
    if let Err(error) = self.spawn_inbound_workers() {
      self.server.shutdown();
      self.running = false;
      return Err(error);
    }
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    self.server.shutdown();
    for (_peer, client) in self.clients.iter_mut() {
      client.shutdown();
    }
    for handle in self.inbound_workers.drain(..) {
      handle.abort();
    }
    if self.remote_event_tx.is_some() && self.inbound_rxs.is_none() {
      self.reset_inbound_channel();
    }
    self.clients.clear();
    self.running = false;
    Ok(())
  }

  fn connect_peer(&mut self, remote: &Address) -> Result<(), TransportError> {
    self.connect_peer_writer(remote)
  }

  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    if !self.running {
      return Err((TransportError::NotStarted, Box::new(envelope)));
    }
    let remote = match remote_address_from_envelope(&envelope) {
      | Ok(remote) => remote,
      | Err(error) => return Err((error, Box::new(envelope))),
    };
    let peer_key = Self::peer_key_for_address(&remote);
    if !self.clients.contains_key(&peer_key) {
      return Err((TransportError::ConnectionClosed, Box::new(envelope)));
    }
    let Some(serialization_extension) = self.serialization_extension.as_ref() else {
      tracing::debug!("serialization extension is not connected to TcpRemoteTransport");
      return Err((TransportError::NotAvailable, Box::new(envelope)));
    };
    let frame = match outbound_envelope_to_pdu(&envelope, serialization_extension) {
      | Ok(pdu) => WireFrame::Envelope(pdu),
      | Err(error) => return Err((error, Box::new(envelope))),
    };
    let lane_key = outbound_lane_key_for_envelope(&envelope);
    match self.send_wire_frame_with_lane_key(&remote, &lane_key, frame) {
      | Ok(()) => Ok(()),
      | Err(error) => Err((error, Box::new(envelope))),
    }
  }

  fn send_control(&mut self, remote: &Address, pdu: ControlPdu) -> Result<(), TransportError> {
    TcpRemoteTransport::send_control(self, remote, pdu)
  }

  fn send_deployment(&mut self, remote: &Address, pdu: RemoteDeploymentPdu) -> Result<(), TransportError> {
    TcpRemoteTransport::send_deployment(self, remote, pdu)
  }

  fn send_flush_request(&mut self, remote: &Address, pdu: ControlPdu, lane_id: u32) -> Result<(), TransportError> {
    TcpRemoteTransport::send_flush_request(self, remote, pdu, lane_id)
  }

  fn send_ack(&mut self, remote: &Address, pdu: AckPdu) -> Result<(), TransportError> {
    TcpRemoteTransport::send_ack(self, remote, pdu)
  }

  fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError> {
    TcpRemoteTransport::send_handshake(self, remote, pdu)
  }

  fn schedule_handshake_timeout(
    &mut self,
    authority: &TransportEndpoint,
    timeout: Duration,
    generation: u64,
  ) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let Some(sender) = self.remote_event_tx.clone() else {
      return Err(TransportError::NotAvailable);
    };
    let authority = authority.clone();
    let monotonic_epoch = self.monotonic_epoch;
    // タイマー task は transport 停止後でも generation 判定で破棄可能な閉じた通知だけを送る。
    // JoinHandle は shutdown 契約に含めず、送信失敗は task 内で WARN に記録する。
    let _timer_task = tokio::spawn(async move {
      sleep(timeout).await;
      let now_ms = std_instant_elapsed_millis(monotonic_epoch);
      if let Err(error) = sender.send(RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }).await {
        tracing::warn!(?error, "handshake timeout event delivery failed");
      }
    });
    Ok(())
  }

  fn addresses(&self) -> &[Address] {
    &self.local_addresses
  }

  fn default_address(&self) -> Option<&Address> {
    self.default_address.as_ref()
  }

  fn local_address_for_remote(&self, _remote: &Address) -> Option<&Address> {
    // Single-listener transport: every remote is served by the default
    // advertised address.
    self.default_address.as_ref()
  }

  fn quarantine(
    &mut self,
    address: &Address,
    _uid: Option<u64>,
    _reason: QuarantineReason,
  ) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_key = Self::peer_key_for_address(address);
    if let Some(mut client) = self.clients.remove(&peer_key) {
      client.shutdown();
    }
    Ok(())
  }
}
