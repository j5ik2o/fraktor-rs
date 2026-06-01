//! Tokio-based gossip transport.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::net::SocketAddr;
use std::sync::Arc;

use fraktor_cluster_core_kernel_rs::membership::{
  GossipEnvelope, GossipOutbound, GossipPayloadKind, GossipTransport, GossipTransportError, GossipTransportHandoff,
  GossipTransportHandoffError, MembershipDelta,
};
use fraktor_remote_core_rs::address::UniqueAddress;
use tokio::{
  net::UdpSocket,
  runtime::Handle,
  sync::mpsc::{self, Receiver, Sender, error::TryRecvError},
  task::JoinHandle,
};

use super::{gossip_wire_delta_v1::GossipWireDeltaV1, tokio_gossip_transport_config::TokioGossipTransportConfig};

#[cfg(test)]
#[path = "tokio_gossip_transport_test.rs"]
mod tests;

struct OutboundPacket {
  target:  SocketAddr,
  payload: Vec<u8>,
}

/// Tokio-based gossip transport.
pub struct TokioGossipTransport {
  local_addr:          SocketAddr,
  outbound_tx:         Sender<OutboundPacket>,
  outbound_handoff_tx: Sender<GossipTransportHandoff>,
  outbound_handoff_rx: Receiver<GossipTransportHandoff>,
  inbound_envelope_tx: Sender<Result<GossipEnvelope, GossipTransportError>>,
  inbound_delta_rx:    Receiver<(String, MembershipDelta)>,
  inbound_envelope_rx: Receiver<Result<GossipEnvelope, GossipTransportError>>,
  local_identity:      Option<UniqueAddress>,
  peer_identities:     Vec<UniqueAddress>,
  _tasks:              Vec<JoinHandle<()>>,
}

impl TokioGossipTransport {
  /// Binds a new transport.
  pub fn bind(config: TokioGossipTransportConfig, tokio_handle: Handle) -> Result<Self, GossipTransportError> {
    if config.max_datagram_bytes == 0 {
      return Err(GossipTransportError::SendFailed { reason: String::from("max_datagram_bytes must be > 0") });
    }
    if config.outbound_capacity == 0 {
      return Err(GossipTransportError::SendFailed { reason: String::from("outbound_capacity must be > 0") });
    }

    let std_socket = std::net::UdpSocket::bind(&config.bind_addr)
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("bind failed: {error}") })?;
    std_socket
      .set_nonblocking(true)
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("set_nonblocking failed: {error}") })?;
    let socket = UdpSocket::from_std(std_socket)
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("from_std failed: {error}") })?;
    let local_addr = socket
      .local_addr()
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("local addr failed: {error}") })?;
    let socket = Arc::new(socket);

    let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundPacket>(config.outbound_capacity);
    let (outbound_handoff_tx, outbound_handoff_rx) = mpsc::channel::<GossipTransportHandoff>(config.outbound_capacity);
    let (inbound_tx, inbound_delta_rx) = mpsc::channel::<(String, MembershipDelta)>(config.outbound_capacity);
    let (inbound_envelope_tx, inbound_envelope_rx) =
      mpsc::channel::<Result<GossipEnvelope, GossipTransportError>>(config.outbound_capacity);
    let local_identity = config.local_identity;
    let peer_identities = config.allowed_peer_identities;
    let max_datagram_bytes = config.max_datagram_bytes;
    let allowed_peers = config
      .allowed_peers
      .iter()
      .map(|peer| {
        peer.parse::<SocketAddr>().map_err(|error| GossipTransportError::SendFailed {
          reason: format!("invalid allowed peer '{peer}': {error}"),
        })
      })
      .collect::<Result<Vec<_>, _>>()?;

    let recv_socket = Arc::clone(&socket);
    let recv_task = tokio_handle.spawn(async move {
      let mut buffer = vec![0u8; max_datagram_bytes];
      loop {
        let result = recv_socket.recv_from(&mut buffer).await;
        let (bytes, addr) = match result {
          | Ok((len, addr)) => (&buffer[..len], addr),
          | Err(_) => break,
        };
        if !allowed_peers.contains(&addr) {
          continue;
        }
        if let Ok(delta) = decode_delta(bytes)
          && let Err(err) = inbound_tx.try_send((addr.to_string(), delta))
        {
          tracing::warn!(from = %addr, "failed to enqueue inbound gossip delta: {err}");
        }
      }
    });

    let send_socket = Arc::clone(&socket);
    let send_task = tokio_handle.spawn(async move {
      while let Some(packet) = outbound_rx.recv().await {
        if let Err(err) = send_socket.send_to(&packet.payload, packet.target).await {
          tracing::warn!(target = %packet.target, "failed to send outbound gossip packet: {err}");
        }
      }
    });

    Ok(Self {
      local_addr,
      outbound_tx,
      outbound_handoff_tx,
      outbound_handoff_rx,
      inbound_envelope_tx,
      inbound_delta_rx,
      inbound_envelope_rx,
      local_identity,
      peer_identities,
      _tasks: vec![recv_task, send_task],
    })
  }

  /// Returns the bound local address.
  #[must_use]
  pub const fn local_addr(&self) -> SocketAddr {
    self.local_addr
  }

  /// Replaces the peer identity mapping used for logical envelope handoff.
  pub fn update_peer_identities(&mut self, peer_identities: Vec<UniqueAddress>) {
    self.peer_identities = peer_identities;
  }

  /// Replaces the local identity used for inbound logical envelope handoff validation.
  pub fn update_local_identity(&mut self, local_identity: UniqueAddress) {
    self.local_identity = Some(local_identity);
  }

  /// Validates an envelope and returns its logical transport handoff.
  ///
  /// # Errors
  ///
  /// Returns a handoff error when the envelope expired or the peer identity is unknown.
  pub fn handoff_envelope(
    &self,
    envelope: GossipEnvelope,
    now_tick: u64,
  ) -> Result<GossipTransportHandoff, GossipTransportHandoffError> {
    GossipTransportHandoff::try_new(envelope, &self.peer_identities, now_tick)
  }

  /// Polls logical handoffs prepared by [`GossipTransport::send_envelope`].
  pub fn poll_outbound_handoffs(&mut self) -> Vec<GossipTransportHandoff> {
    let mut handoffs = Vec::new();
    loop {
      match self.outbound_handoff_rx.try_recv() {
        | Ok(handoff) => handoffs.push(handoff),
        | Err(TryRecvError::Empty) => break,
        | Err(TryRecvError::Disconnected) => break,
      }
    }
    handoffs
  }

  /// Accepts a logical handoff from another std transport boundary.
  ///
  /// # Errors
  ///
  /// Returns a transport error when the source identity is not allowed or the target endpoint does
  /// not match this transport's advertised identity.
  pub fn accept_handoff(&mut self, handoff: GossipTransportHandoff) -> Result<(), GossipTransportError> {
    if !self.peer_identities.iter().any(|peer| peer == handoff.from()) {
      return Err(GossipTransportError::Handoff(GossipTransportHandoffError::UnknownPeer {
        peer: handoff.from().clone(),
      }));
    }
    let Some(local_identity) = &self.local_identity else {
      return Err(GossipTransportError::ReceiveFailed { reason: String::from("local identity is not configured") });
    };
    if local_identity != handoff.to() {
      return Err(GossipTransportError::Handoff(GossipTransportHandoffError::InvalidIdentity {
        expected: Box::new(local_identity.clone()),
        actual:   Box::new(handoff.to().clone()),
      }));
    }
    if handoff.target_endpoint() != GossipTransportHandoff::endpoint_for_identity(local_identity).as_str() {
      return Err(GossipTransportError::ReceiveFailed {
        reason: format!("target endpoint mismatch: {}", handoff.target_endpoint()),
      });
    }
    self.inbound_envelope_tx.try_send(Ok(handoff.envelope().clone())).map_err(|error| {
      GossipTransportError::ReceiveFailed { reason: format!("inbound envelope enqueue failed: {error}") }
    })
  }

  /// Converts a logical payload kind tag into a transport-visible result.
  ///
  /// # Errors
  ///
  /// Returns a transport error when `tag` is not a known payload kind.
  pub fn receive_payload_kind_tag(&self, tag: u8) -> Result<GossipPayloadKind, GossipTransportError> {
    GossipTransportHandoff::payload_kind_from_tag(tag).map_err(GossipTransportError::from)
  }

  fn encode_delta(&self, delta: &MembershipDelta) -> Result<Vec<u8>, GossipTransportError> {
    let wire = GossipWireDeltaV1::from_delta(delta);
    postcard::to_allocvec(&wire)
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("encode failed: {error}") })
  }
}

impl GossipTransport for TokioGossipTransport {
  fn send(&mut self, outbound: GossipOutbound) -> Result<(), GossipTransportError> {
    let target = outbound
      .target
      .parse::<SocketAddr>()
      .map_err(|error| GossipTransportError::SendFailed { reason: error.to_string() })?;
    let payload = self.encode_delta(&outbound.delta)?;
    let packet = OutboundPacket { target, payload };
    self
      .outbound_tx
      .try_send(packet)
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("outbound enqueue failed: {error}") })
  }

  fn send_envelope(&mut self, envelope: GossipEnvelope, now_tick: u64) -> Result<(), GossipTransportError> {
    let handoff = self.handoff_envelope(envelope, now_tick).map_err(GossipTransportError::from)?;
    self
      .outbound_handoff_tx
      .try_send(handoff)
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("outbound handoff enqueue failed: {error}") })
  }

  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)> {
    let mut deltas = Vec::new();
    loop {
      match self.inbound_delta_rx.try_recv() {
        | Ok(delta) => deltas.push(delta),
        | Err(TryRecvError::Empty) => break,
        | Err(TryRecvError::Disconnected) => break,
      }
    }
    deltas
  }

  fn poll_envelopes(&mut self) -> Vec<Result<GossipEnvelope, GossipTransportError>> {
    let mut envelopes = Vec::new();
    loop {
      match self.inbound_envelope_rx.try_recv() {
        | Ok(envelope) => envelopes.push(envelope),
        | Err(TryRecvError::Empty) => break,
        | Err(TryRecvError::Disconnected) => break,
      }
    }
    envelopes
  }
}

fn decode_delta(bytes: &[u8]) -> Result<MembershipDelta, GossipTransportError> {
  const MAX_ENTRIES: usize = 1024;
  let wire: GossipWireDeltaV1 = postcard::from_bytes(bytes)
    .map_err(|error| GossipTransportError::SendFailed { reason: format!("decode failed: {error}") })?;
  if wire.entries.len() > MAX_ENTRIES {
    return Err(GossipTransportError::SendFailed {
      reason: format!("too many entries: {} (max {MAX_ENTRIES})", wire.entries.len()),
    });
  }
  wire.to_delta().ok_or_else(|| GossipTransportError::SendFailed { reason: String::from("invalid status value") })
}
