//! `TcpRemoteTransport` — `std::net`-backed implementation of the core
//! [`RemoteTransport`] port.

use alloc::{string::String, vec::Vec};
use core::{
  fmt::{Debug, Formatter, Result as FmtResult},
  time::Duration,
};
use std::{collections::BTreeMap, time::Instant};

use fraktor_remote_core_rs::core::{
  address::Address,
  association::QuarantineReason,
  config::RemoteConfig,
  envelope::OutboundEnvelope,
  extension::RemoteEvent,
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  wire::HandshakePdu,
};
use tokio::{
  sync::mpsc::{self, Sender, UnboundedReceiver, UnboundedSender},
  task::JoinHandle,
  time::sleep,
};

use super::{
  client::TcpClient, frame_codec::WireFrameCodec, inbound_frame_event::InboundFrameEvent, server::TcpServer,
  wire_frame::WireFrame,
};
use crate::std::association::{run_inbound_dispatch, std_instant_elapsed_millis};

/// TCP-backed implementation of [`RemoteTransport`].
///
/// This struct aggregates a [`TcpServer`] (inbound) and a [`BTreeMap`] of
/// [`TcpClient`]s (outbound, one per remote authority). Inbound frames land
/// in an `mpsc::UnboundedReceiver` that callers (typically the
/// `association` module added in Section 19) poll to feed the pure
/// `Association` state machines.
///
/// Note: the trait [`RemoteTransport::send`] is **synchronous**; because
/// establishing a brand-new outbound TCP connection is asynchronous, callers
/// must call [`TcpRemoteTransport::connect_peer`] from an async context
/// *before* calling `send` for a given peer. This mirrors Pekko Artery's
/// explicit association lifecycle. User envelope delivery still requires the
/// Phase 3 serialization driver, so `send` fails fast instead of emitting an
/// empty payload frame.
pub struct TcpRemoteTransport {
  configured_local_addresses: Vec<Address>,
  local_addresses:            Vec<Address>,
  default_address:            Option<Address>,
  bind_addr:                  String,
  frame_codec:                WireFrameCodec,
  server:                     TcpServer,
  clients:                    BTreeMap<String, TcpClient>,
  inbound_tx:                 UnboundedSender<InboundFrameEvent>,
  inbound_rx:                 Option<UnboundedReceiver<InboundFrameEvent>>,
  remote_event_tx:            Option<Sender<RemoteEvent>>,
  monotonic_epoch:            Instant,
  inbound_restart_timeout:    Duration,
  inbound_max_restarts:       u32,
  inbound_worker:             Option<JoinHandle<Result<(), TransportError>>>,
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
    Self::with_frame_codec(bind_addr, local_addresses, frame_codec)
      .with_inbound_restart_budget(config.inbound_max_restarts(), config.inbound_restart_timeout())
  }

  fn with_frame_codec(bind_addr: String, local_addresses: Vec<Address>, frame_codec: WireFrameCodec) -> Self {
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<InboundFrameEvent>();
    let default_address = local_addresses.first().cloned();
    let default_config = RemoteConfig::new("localhost");
    Self {
      configured_local_addresses: local_addresses.clone(),
      local_addresses,
      default_address,
      server: TcpServer::with_frame_codec(bind_addr.clone(), frame_codec),
      bind_addr,
      frame_codec,
      clients: BTreeMap::new(),
      inbound_tx,
      inbound_rx: Some(inbound_rx),
      remote_event_tx: None,
      monotonic_epoch: Instant::now(),
      inbound_restart_timeout: default_config.inbound_restart_timeout(),
      inbound_max_restarts: default_config.inbound_max_restarts(),
      inbound_worker: None,
      running: false,
    }
  }

  fn reset_inbound_channel(&mut self) {
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<InboundFrameEvent>();
    self.inbound_tx = inbound_tx;
    self.inbound_rx = Some(inbound_rx);
  }

  /// Returns a copy that emits scheduled remote events through `sender`.
  #[must_use]
  pub fn with_remote_event_sender(mut self, sender: Sender<RemoteEvent>) -> Self {
    self.remote_event_tx = Some(sender);
    self
  }

  /// Returns a copy that uses the given monotonic epoch for all emitted remote event timestamps.
  #[must_use]
  pub fn with_monotonic_epoch(mut self, monotonic_epoch: Instant) -> Self {
    self.monotonic_epoch = monotonic_epoch;
    self
  }

  /// Configures the inbound worker restart budget.
  #[must_use]
  pub fn with_inbound_restart_budget(mut self, max_restarts: u32, restart_timeout: Duration) -> Self {
    self.inbound_max_restarts = max_restarts;
    self.inbound_restart_timeout = restart_timeout;
    self
  }

  /// Returns the monotonic epoch used to calculate remote event timestamps.
  #[must_use]
  pub fn monotonic_epoch(&self) -> Instant {
    self.monotonic_epoch
  }

  /// Takes ownership of the inbound receiver.
  ///
  /// Exactly one consumer (typically the `association` task) should
  /// take the receiver to start processing inbound frames. Subsequent calls
  /// return `None`.
  #[must_use]
  pub fn take_inbound_receiver(&mut self) -> Option<UnboundedReceiver<InboundFrameEvent>> {
    self.inbound_rx.take()
  }

  fn spawn_inbound_worker(&mut self) -> Result<(), TransportError> {
    let Some(event_sender) = self.remote_event_tx.clone() else {
      tracing::debug!("with_remote_event_sender was not called; inbound worker not spawned");
      return Ok(());
    };
    let Some(inbound_rx) = self.inbound_rx.take() else {
      tracing::debug!("inbound receiver was already consumed; inbound worker not spawned");
      return Err(TransportError::NotAvailable);
    };
    let monotonic_epoch = self.monotonic_epoch;
    let inbound_max_restarts = self.inbound_max_restarts;
    let inbound_restart_timeout = self.inbound_restart_timeout;
    let handle = tokio::spawn(async move {
      run_inbound_dispatch(
        inbound_rx,
        event_sender,
        move || std_instant_elapsed_millis(monotonic_epoch),
        inbound_max_restarts,
        inbound_restart_timeout,
      )
      .await
    });
    self.inbound_worker = Some(handle);
    Ok(())
  }

  /// Establishes an outbound connection to `remote` and stores the client.
  ///
  /// This method is async because `TcpStream::connect` is async. It must be
  /// invoked from an async context (e.g. `tokio::spawn`) prior to calling
  /// the synchronous [`RemoteTransport::send`].
  ///
  /// `remote` is hashed via the same [`Self::peer_key_for_address`] formatter
  /// used by [`RemoteTransport::send`] / [`Self::send_handshake`], so the
  /// stored client is guaranteed to match subsequent send/quarantine lookups.
  /// (Earlier revisions accepted an arbitrary `String` here, which silently
  /// caused `ConnectionClosed` errors when callers formatted the key
  /// differently from the internal `host:port` convention.)
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport has not yet been
  /// started, or [`TransportError::SendFailed`] if the outbound connection
  /// cannot be established.
  pub async fn connect_peer(&mut self, remote: &Address) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_key = Self::peer_key_for_address(remote);
    if self.clients.contains_key(&peer_key) {
      return Ok(());
    }
    let connect_addr = alloc::format!("{}:{}", remote.host(), remote.port());
    let client = TcpClient::connect_with_frame_codec(connect_addr, self.inbound_tx.clone(), self.frame_codec).await?;
    self.clients.insert(peer_key, client);
    Ok(())
  }

  /// Returns an immutable reference to the client registry.
  #[must_use]
  pub fn clients(&self) -> &BTreeMap<String, TcpClient> {
    &self.clients
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

  fn peer_key_for_address(address: &Address) -> String {
    alloc::format!("{}:{}", address.host(), address.port())
  }

  /// Sends a handshake PDU to an already connected peer.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] when the transport has not been started, or
  /// [`TransportError::ConnectionClosed`] when no TCP client is registered for `remote`.
  pub fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError> {
    if !self.running {
      return Err(TransportError::NotStarted);
    }
    let peer_key = Self::peer_key_for_address(remote);
    let Some(client) = self.clients.get(&peer_key) else {
      return Err(TransportError::ConnectionClosed);
    };
    client.send(WireFrame::Handshake(pdu))
  }
}

impl RemoteTransport for TcpRemoteTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    if self.running {
      return Err(TransportError::AlreadyRunning);
    }
    let bound_addr = self.server.start(self.inbound_tx.clone())?;
    self.apply_bound_port_to_advertised_addresses(bound_addr.port());
    self.running = true;
    if let Err(error) = self.spawn_inbound_worker() {
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
    if let Some(handle) = self.inbound_worker.take() {
      handle.abort();
    }
    if self.remote_event_tx.is_some() && self.inbound_rx.is_none() {
      self.reset_inbound_channel();
    }
    self.clients.clear();
    self.running = false;
    Ok(())
  }

  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)> {
    if !self.running {
      return Err((TransportError::NotStarted, Box::new(envelope)));
    }
    Err((TransportError::SendFailed, Box::new(envelope)))
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
