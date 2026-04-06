//! `TcpRemoteTransport` — `std::net`-backed implementation of the core
//! [`RemoteTransport`] port.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use std::collections::BTreeMap;

use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId},
  association::QuarantineReason,
  envelope::OutboundEnvelope,
  transport::{RemoteTransport, TransportError},
  wire::EnvelopePdu,
};
use tokio::sync::mpsc;

use crate::tcp_transport::{
  client::TcpClient, inbound_frame_event::InboundFrameEvent, server::TcpServer, wire_frame::WireFrame,
};

/// TCP-backed implementation of [`RemoteTransport`].
///
/// This struct aggregates a [`TcpServer`] (inbound) and a [`BTreeMap`] of
/// [`TcpClient`]s (outbound, one per remote authority). Inbound frames land
/// in an `mpsc::UnboundedReceiver` that callers (typically the
/// `association_runtime` module added in Section 19) poll to feed the pure
/// `Association` state machines.
///
/// Note: the trait [`RemoteTransport::send`] is **synchronous**; because
/// establishing a brand-new outbound TCP connection is asynchronous, callers
/// must call [`TcpRemoteTransport::connect_peer`] from an async context
/// *before* calling `send` for a given peer. This mirrors Pekko Artery's
/// explicit association lifecycle.
pub struct TcpRemoteTransport {
  local_addresses: Vec<Address>,
  default_address: Option<Address>,
  bind_addr:       String,
  server:          TcpServer,
  clients:         BTreeMap<String, TcpClient>,
  inbound_tx:      mpsc::UnboundedSender<InboundFrameEvent>,
  inbound_rx:      Option<mpsc::UnboundedReceiver<InboundFrameEvent>>,
  running:         bool,
}

impl core::fmt::Debug for TcpRemoteTransport {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<InboundFrameEvent>();
    let bind_addr = bind_addr.into();
    let default_address = local_addresses.first().cloned();
    Self {
      local_addresses,
      default_address,
      server: TcpServer::new(bind_addr.clone()),
      bind_addr,
      clients: BTreeMap::new(),
      inbound_tx,
      inbound_rx: Some(inbound_rx),
      running: false,
    }
  }

  /// Takes ownership of the inbound receiver.
  ///
  /// Exactly one consumer (typically the `association_runtime` task) should
  /// take the receiver to start processing inbound frames. Subsequent calls
  /// return `None`.
  #[must_use]
  pub fn take_inbound_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<InboundFrameEvent>> {
    self.inbound_rx.take()
  }

  /// Establishes an outbound connection to `peer_addr` and stores the client.
  ///
  /// This method is async because `TcpStream::connect` is async. It must be
  /// invoked from an async context (e.g. `tokio::spawn`) prior to calling
  /// the synchronous [`RemoteTransport::send`].
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport has not yet been
  /// started, or [`TransportError::SendFailed`] if the outbound connection
  /// cannot be established.
  pub async fn connect_peer(&mut self, peer_addr: impl Into<String>) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_addr = peer_addr.into();
    if self.clients.contains_key(&peer_addr) {
      return Ok(());
    }
    let client = TcpClient::connect(peer_addr.clone(), self.inbound_tx.clone()).await?;
    self.clients.insert(peer_addr, client);
    Ok(())
  }

  /// Asynchronously starts the transport. Call this from within a tokio
  /// runtime to bind the listener and spawn the accept loop.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::AlreadyRunning`] if the transport is already
  /// running, or a bind failure from the underlying listener.
  pub async fn start_async(&mut self) -> Result<(), TransportError> {
    if self.running {
      return Err(TransportError::AlreadyRunning);
    }
    self.server.start(self.inbound_tx.clone()).await?;
    self.running = true;
    Ok(())
  }

  /// Returns an immutable reference to the client registry.
  #[must_use]
  pub fn clients(&self) -> &BTreeMap<String, TcpClient> {
    &self.clients
  }

  fn peer_key_for_address(address: &Address) -> String {
    alloc::format!("{}:{}", address.host(), address.port())
  }

  fn peer_key_for_remote_node(node: &RemoteNodeId) -> String {
    alloc::format!("{}:{}", node.host(), node.port().unwrap_or(0))
  }
}

impl RemoteTransport for TcpRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    // The trait contract is synchronous — we cannot bind a TCP listener
    // without an async runtime, so callers are expected to use
    // [`Self::start_async`] instead. We still gate the state flag here so
    // that higher-level lifecycle code works correctly when it does not
    // need the listener (e.g. tests with stub clients).
    if self.running {
      return Err(TransportError::AlreadyRunning);
    }
    self.running = true;
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
    self.clients.clear();
    self.running = false;
    Ok(())
  }

  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_key = Self::peer_key_for_remote_node(envelope.remote_node());
    let Some(client) = self.clients.get(&peer_key) else {
      return Err(TransportError::ConnectionClosed);
    };
    let frame = build_envelope_frame(&envelope);
    client.send(frame)
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

fn build_envelope_frame(envelope: &OutboundEnvelope) -> WireFrame {
  let recipient_path = envelope.recipient().to_string();
  let sender_path = envelope.sender().map(ToString::to_string);
  let priority = envelope.priority().to_wire();
  let correlation = envelope.correlation_id();
  // Phase B minimum: an empty payload placeholder. The actual serialisation
  // of `AnyMessage` is a responsibility of the association_runtime layer
  // added in Section 19, which will invoke the serialization extension.
  let pdu =
    EnvelopePdu::new(recipient_path, sender_path, correlation.hi(), correlation.lo(), priority, bytes::Bytes::new());
  WireFrame::Envelope(pdu)
}
