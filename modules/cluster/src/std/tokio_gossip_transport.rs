//! Tokio-based gossip transport.

#[cfg(test)]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::net::SocketAddr;
use std::sync::Arc;

use tokio::{net::UdpSocket, sync::mpsc};

use crate::{
  core::{GossipOutbound, GossipTransport, GossipTransportError, MembershipDelta},
  std::{gossip_wire_delta_v1::GossipWireDeltaV1, tokio_gossip_transport_config::TokioGossipTransportConfig},
};

struct OutboundPacket {
  target:  SocketAddr,
  payload: Vec<u8>,
}

/// Tokio-based gossip transport.
pub struct TokioGossipTransport {
  outbound_tx: mpsc::Sender<OutboundPacket>,
  inbound_rx:  mpsc::Receiver<(String, MembershipDelta)>,
  _tasks:      Vec<tokio::task::JoinHandle<()>>,
}

impl TokioGossipTransport {
  /// Binds a new transport.
  pub fn bind(
    config: TokioGossipTransportConfig,
    runtime: tokio::runtime::Handle,
  ) -> Result<Self, GossipTransportError> {
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
    let _local_addr = socket
      .local_addr()
      .map_err(|error| GossipTransportError::SendFailed { reason: format!("local addr failed: {error}") })?;
    let socket = Arc::new(socket);

    let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundPacket>(config.outbound_capacity);
    let (inbound_tx, inbound_rx) = mpsc::channel::<(String, MembershipDelta)>(config.outbound_capacity);

    let recv_socket = Arc::clone(&socket);
    let recv_task = runtime.spawn(async move {
      let mut buffer = vec![0u8; config.max_datagram_bytes];
      loop {
        let result = recv_socket.recv_from(&mut buffer).await;
        let (bytes, addr) = match result {
          | Ok((len, addr)) => (&buffer[..len], addr),
          | Err(_) => break,
        };
        if let Ok(delta) = decode_delta(bytes) {
          let _ = inbound_tx.try_send((addr.to_string(), delta));
        }
      }
    });

    let send_socket = Arc::clone(&socket);
    let send_task = runtime.spawn(async move {
      while let Some(packet) = outbound_rx.recv().await {
        let _ = send_socket.send_to(&packet.payload, packet.target).await;
      }
    });

    Ok(Self { outbound_tx, inbound_rx, _tasks: vec![recv_task, send_task] })
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

  fn poll_deltas(&mut self) -> Vec<(String, MembershipDelta)> {
    let mut deltas = Vec::new();
    loop {
      match self.inbound_rx.try_recv() {
        | Ok(delta) => deltas.push(delta),
        | Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
        | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
      }
    }
    deltas
  }
}

fn decode_delta(bytes: &[u8]) -> Result<MembershipDelta, GossipTransportError> {
  let wire: GossipWireDeltaV1 = postcard::from_bytes(bytes)
    .map_err(|error| GossipTransportError::SendFailed { reason: format!("decode failed: {error}") })?;
  wire.to_delta().ok_or_else(|| GossipTransportError::SendFailed { reason: String::from("invalid status value") })
}
