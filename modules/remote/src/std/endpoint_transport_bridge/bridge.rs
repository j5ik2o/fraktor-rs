use alloc::{
  boxed::{Box as AllocBox, Box},
  collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
  format,
  string::String,
  sync::Arc,
  vec::Vec,
};
use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};
use std::time::{SystemTime, UNIX_EPOCH};

use fraktor_actor_rs::core::{
  event::{
    logging::LogLevel,
    stream::{CorrelationId, RemotingLifecycleEvent},
  },
  system::ActorSystemWeakGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, SharedAccess},
};
use tokio::{
  sync::{Mutex as TokioMutex, mpsc},
  time::{Instant, sleep},
};

use super::{EndpointTransportBridgeConfig, EndpointTransportBridgeHandle};
use crate::core::{
  EventPublisherGeneric, FLUSH_ACK_FRAME_KIND, FLUSH_FRAME_KIND, Flush, FlushAck, RemoteInstruments, RemoteNodeId,
  WireError,
  endpoint_association::{
    AssociationState, EndpointAssociationCommand, EndpointAssociationCoordinatorSharedGeneric,
    EndpointAssociationEffect,
  },
  endpoint_reader::EndpointReaderGeneric,
  endpoint_writer::EndpointWriterSharedGeneric,
  envelope::{
    ACKED_DELIVERY_ACK_FRAME_KIND, ACKED_DELIVERY_NACK_FRAME_KIND, AckedDelivery, DeferredEnvelope, RemotingEnvelope,
    SYSTEM_MESSAGE_FRAME_KIND, SystemMessageEnvelope,
  },
  handshake::{HandshakeFrame, HandshakeKind},
  remoting_extension::RemotingControlHandle,
  transport::{
    RemoteTransportShared, TransportBind, TransportChannel, TransportEndpoint, TransportError, TransportHandle,
    inbound::{InboundFrame, TransportInbound, TransportInboundShared},
  },
  watcher::{HEARTBEAT_FRAME_KIND, HEARTBEAT_RSP_FRAME_KIND, Heartbeat, HeartbeatRsp, RemoteWatcherCommand},
};

const HANDSHAKE_INIT_FRAME_KIND: u8 = 0x01;
const HANDSHAKE_ACK_FRAME_KIND: u8 = 0x02;
const REMOTING_MESSAGE_FRAME_KIND: u8 = 0x10;
const OUTBOUND_IDLE_DELAY: Duration = Duration::from_millis(5);
const INBOUND_FRAME_MAX_CONCURRENCY: usize = 32;
const WATCHER_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(100);
const WATCHER_REAP_INTERVAL: Duration = Duration::from_millis(200);
const REMOTE_INSTRUMENT_TRAILER_MARKER: [u8; 2] = [0xA5, 0x7C];
static LOCAL_NODE_UID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

enum InboundSystemSequenceResult {
  Deliver,
  Duplicate { ack_sequence_no: u64 },
  Missing { highest_acked_sequence_no: u64 },
}

pub(crate) struct EndpointTransportBridge<TB: RuntimeToolbox + 'static> {
  system:                 ActorSystemWeakGeneric<TB>,
  control:                RemotingControlHandle<TB>,
  event_publisher:        EventPublisherGeneric<TB>,
  writer:                 EndpointWriterSharedGeneric<TB>,
  reader:                 ArcShared<EndpointReaderGeneric<TB>>,
  transport:              RemoteTransportShared<TB>,
  host:                   String,
  port:                   u16,
  system_name:            String,
  handshake_timeout:      Duration,
  shutdown_flush_timeout: Duration,
  local_uid:              u64,
  listener:               TokioMutex<Option<TransportHandle>>,
  channels:               TokioMutex<BTreeMap<String, TransportChannel>>,
  watchdog_versions:      Arc<TokioMutex<BTreeMap<String, u64>>>,
  system_sequences:       TokioMutex<BTreeMap<String, u64>>,
  inbound_sequences:      TokioMutex<BTreeMap<String, u64>>,
  pending_system:         TokioMutex<BTreeMap<String, BTreeMap<u64, SystemMessageEnvelope>>>,
  peer_authorities:       TokioMutex<BTreeMap<String, String>>,
  system_correlations:    TokioMutex<BTreeMap<u128, String>>,
  flush_ack_observations: TokioMutex<BTreeMap<String, u32>>,
  remote_instruments:     RemoteInstruments,
  pub(super) coordinator: EndpointAssociationCoordinatorSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> EndpointTransportBridge<TB> {
  pub(super) fn new(config: EndpointTransportBridgeConfig<TB>) -> Arc<Self> {
    Arc::new(Self {
      system:                 config.system,
      control:                config.control,
      event_publisher:        config.event_publisher,
      writer:                 config.writer,
      reader:                 config.reader,
      transport:              config.transport,
      host:                   config.canonical_host,
      port:                   config.canonical_port,
      system_name:            config.system_name,
      handshake_timeout:      config.handshake_timeout,
      shutdown_flush_timeout: config.shutdown_flush_timeout,
      local_uid:              Self::allocate_local_uid(),
      listener:               TokioMutex::new(None),
      channels:               TokioMutex::new(BTreeMap::<String, TransportChannel>::new()),
      watchdog_versions:      Arc::new(TokioMutex::new(BTreeMap::<String, u64>::new())),
      system_sequences:       TokioMutex::new(BTreeMap::<String, u64>::new()),
      inbound_sequences:      TokioMutex::new(BTreeMap::<String, u64>::new()),
      pending_system:         TokioMutex::new(BTreeMap::<String, BTreeMap<u64, SystemMessageEnvelope>>::new()),
      peer_authorities:       TokioMutex::new(BTreeMap::<String, String>::new()),
      system_correlations:    TokioMutex::new(BTreeMap::<u128, String>::new()),
      flush_ack_observations: TokioMutex::new(BTreeMap::<String, u32>::new()),
      remote_instruments:     RemoteInstruments::new(config.remote_instruments),
      coordinator:            EndpointAssociationCoordinatorSharedGeneric::new(),
    })
  }

  pub(crate) fn spawn(
    config: EndpointTransportBridgeConfig<TB>,
  ) -> Result<EndpointTransportBridgeHandle, TransportError> {
    let bridge = Self::new(config);
    let bind = TransportBind::new(bridge.host.clone(), Some(bridge.port));
    let handle = bridge.transport.with_write(|t| t.spawn_listener(&bind))?;
    bridge.event_publisher.publish_listen_started(bind.authority(), CorrelationId::from_u128(0));
    *bridge.listener.try_lock().expect("listener mutex uncontended") = Some(handle);
    let handler: TransportInboundShared<TB> =
      TransportInboundShared::new(Box::new(InboundHandler::new(bridge.clone(), INBOUND_FRAME_MAX_CONCURRENCY)));
    bridge.transport.with_write(|t| t.install_inbound_handler(handler));
    let send_task = tokio::spawn(Self::drive_outbound(bridge.clone()));
    Ok(EndpointTransportBridgeHandle { send_task })
  }

  async fn drive_outbound(self: Arc<Self>) {
    let mut last_reap_at = Instant::now();
    let mut last_heartbeat_at = Instant::now();
    let mut observed_running = false;
    loop {
      if self.control.is_running() {
        observed_running = true;
      } else if observed_running {
        self.run_shutdown_flush().await;
        break;
      }
      if last_heartbeat_at.elapsed() >= WATCHER_HEARTBEAT_INTERVAL {
        self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat_tick());
        last_heartbeat_at = Instant::now();
      }
      if last_reap_at.elapsed() >= WATCHER_REAP_INTERVAL {
        self.dispatch_remote_watcher(RemoteWatcherCommand::reap_unreachable());
        last_reap_at = Instant::now();
      }
      let next = self.writer.with_write(|w| w.try_next());
      match next {
        | Ok(Some(envelope)) => {
          if let Err(error) = self.handle_outbound_envelope(envelope).await {
            self.emit_error(format!("failed to process outbound envelope: {error:?}"));
          }
        },
        | Ok(None) => sleep(OUTBOUND_IDLE_DELAY).await,
        | Err(error) => {
          self.emit_error(format!("endpoint writer error: {error:?}"));
          sleep(OUTBOUND_IDLE_DELAY).await;
        },
      }
    }
  }

  async fn handle_outbound_envelope(&self, envelope: RemotingEnvelope) -> Result<(), TransportError> {
    let authority = Self::target_authority(envelope.remote_node())
      .ok_or_else(|| TransportError::AuthorityNotBound("missing remote authority".into()))?;
    let deferred = DeferredEnvelope::new(envelope);
    let enqueue = self.coordinator.with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.clone(),
        envelope:  alloc::boxed::Box::new(deferred),
      })
    });
    self.process_effects(enqueue.effects).await?;

    match self.coordinator.with_read(|m| m.state(&authority)) {
      | Some(AssociationState::Connected { .. })
      | Some(AssociationState::Associating { .. })
      | Some(AssociationState::Quarantined { .. }) => {},
      | Some(AssociationState::Gated { resume_at }) => {
        let now = self.now_millis();
        let should_recover = match resume_at {
          | Some(deadline) => now >= deadline,
          | None => true,
        };
        if should_recover {
          let endpoint = TransportEndpoint::new(authority.clone());
          let recover = self.coordinator.with_write(|m| {
            m.handle(EndpointAssociationCommand::Recover {
              authority: authority.clone(),
              endpoint: Some(endpoint),
              now,
            })
          });
          self.process_effects(recover.effects).await?;
        }
      },
      | Some(AssociationState::Unassociated) | None => {
        let endpoint = TransportEndpoint::new(authority.clone());
        let associate = self.coordinator.with_write(|m| {
          m.handle(EndpointAssociationCommand::Associate {
            authority: authority.clone(),
            endpoint,
            now: self.now_millis(),
          })
        });
        self.process_effects(associate.effects).await?;
      },
    }
    Ok(())
  }

  pub(super) async fn process_effects(&self, effects: Vec<EndpointAssociationEffect>) -> Result<(), TransportError> {
    let mut queue: VecDeque<EndpointAssociationEffect> = VecDeque::from(effects);
    while let Some(effect) = queue.pop_front() {
      match effect {
        | EndpointAssociationEffect::StartHandshake { authority, endpoint } => {
          let additional = self.handle_start_handshake(&authority, &endpoint).await?;
          queue.extend(additional);
        },
        | EndpointAssociationEffect::DeliverEnvelopes { authority, envelopes } => {
          for envelope in envelopes {
            self.flush_envelope(&authority, envelope).await?;
          }
        },
        | EndpointAssociationEffect::DiscardDeferred { authority, .. } => {
          self.emit_error(format!("discarded deferred envelopes for {authority}"));
        },
        | EndpointAssociationEffect::Lifecycle(event) => self.event_publisher.publish_lifecycle(event),
      }
    }
    Ok(())
  }

  async fn handle_start_handshake(
    &self,
    authority: &str,
    endpoint: &TransportEndpoint,
  ) -> Result<Vec<EndpointAssociationEffect>, TransportError> {
    let channel = if let Some(existing) = self.channels.lock().await.get(authority).copied() {
      existing
    } else {
      let channel = self.transport.with_write(|t| t.open_channel(endpoint))?;
      self.channels.lock().await.insert(authority.to_string(), channel);
      channel
    };
    let watchdog_version = self.next_watchdog_version(authority).await;
    self.spawn_handshake_timeout_watchdog(authority.to_string(), watchdog_version);

    let handshake =
      HandshakeFrame::new(HandshakeKind::Offer, &self.system_name, &self.host, Some(self.port), self.local_uid);
    let payload = handshake.encode();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))?;
    Ok(Vec::new())
  }

  fn allocate_local_uid() -> u64 {
    // as_nanos() returns u128; the u64 cast is safe for practical use (covers ~584 years from epoch).
    let timestamp_nanos =
      SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock should be after unix epoch").as_nanos() as u64;
    let sequence = LOCAL_NODE_UID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    timestamp_nanos.saturating_add(sequence)
  }

  async fn next_watchdog_version(&self, authority: &str) -> u64 {
    let mut versions = self.watchdog_versions.lock().await;
    let next = versions.get(authority).copied().unwrap_or(0).saturating_add(1);
    versions.insert(authority.to_string(), next);
    next
  }

  fn spawn_handshake_timeout_watchdog(&self, authority: String, watchdog_version: u64) {
    let timeout = self.handshake_timeout;
    let coordinator = self.coordinator.clone();
    let event_publisher = self.event_publisher.clone();
    let system = self.system.clone();
    let versions = Arc::clone(&self.watchdog_versions);
    tokio::spawn(async move {
      sleep(timeout).await;
      let is_latest = versions.lock().await.get(&authority).copied() == Some(watchdog_version);
      if !is_latest {
        return;
      }
      let now = system.upgrade().map(|s| s.state().monotonic_now().as_millis() as u64).unwrap_or(0);
      let resume_at = None;
      let result = coordinator.with_write(|m| {
        m.handle(EndpointAssociationCommand::HandshakeTimedOut { authority: authority.clone(), resume_at, now })
      });
      for effect in result.effects {
        match effect {
          | EndpointAssociationEffect::DiscardDeferred { authority, .. } => {
            if let Some(system) = system.upgrade() {
              system.emit_log(LogLevel::Error, format!("discarded deferred envelopes for {authority}"), None);
            }
          },
          | EndpointAssociationEffect::Lifecycle(event) => event_publisher.publish_lifecycle(event),
          | EndpointAssociationEffect::StartHandshake { .. } | EndpointAssociationEffect::DeliverEnvelopes { .. } => {},
        }
      }
    });
  }

  async fn flush_envelope(&self, authority: &str, deferred: DeferredEnvelope) -> Result<(), TransportError> {
    let envelope = deferred.into_envelope();
    let channel = self.ensure_channel(authority).await?;
    if envelope.is_system() {
      let sequence_no = self.next_system_sequence(authority).await;
      let system_envelope = SystemMessageEnvelope::from_remoting_envelope(envelope, sequence_no, self.local_node());
      self.register_pending_system_envelope(authority, system_envelope.clone()).await;
      let payload = AckedDelivery::SystemMessage(AllocBox::new(system_envelope.clone())).encode_frame();
      self.transport.with_write(|t| t.send(&channel, &payload, system_envelope.correlation_id()))?;
      return Ok(());
    }

    let start = if self.remote_instruments.serialization_timing_enabled() { Some(Instant::now()) } else { None };
    let metadata = self.remote_instruments.write_metadata();
    let mut payload = envelope.encode_frame();
    if !metadata.is_empty() {
      Self::append_remote_instrument_metadata(&mut payload, &metadata);
    }
    let serialization_nanos = start.map_or(0, |started| started.elapsed().as_nanos() as u64);
    self.transport.with_write(|t| t.send(&channel, &payload, envelope.correlation_id()))?;
    self.remote_instruments.message_sent(payload.len(), serialization_nanos);
    Ok(())
  }

  async fn ensure_channel(&self, authority: &str) -> Result<TransportChannel, TransportError> {
    let mut channels = self.channels.lock().await;
    let channel = match channels.entry(authority.to_string()) {
      | Entry::Occupied(entry) => *entry.get(),
      | Entry::Vacant(entry) => {
        let endpoint = TransportEndpoint::new(authority.to_string());
        let channel = self.transport.with_write(|t| t.open_channel(&endpoint))?;
        entry.insert(channel);
        channel
      },
    };
    Ok(channel)
  }

  fn emit_error(&self, message: String) {
    if let Some(system) = self.system.upgrade() {
      system.emit_log(LogLevel::Error, message, None);
    }
  }

  fn dispatch_remote_watcher(&self, command: RemoteWatcherCommand) {
    if let Err(error) = self.control.dispatch_remote_watcher_command(command) {
      self.emit_error(format!("failed to dispatch remote watcher command: {error}"));
    }
  }

  fn append_remote_instrument_metadata(payload: &mut Vec<u8>, metadata: &[u8]) {
    payload.extend_from_slice(metadata);
    payload.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    payload.extend_from_slice(&REMOTE_INSTRUMENT_TRAILER_MARKER);
  }

  fn split_remote_instrument_metadata(payload: &[u8]) -> Result<(&[u8], &[u8]), WireError> {
    if payload.len() < 6 {
      return Ok((payload, &[]));
    }
    let marker_start = payload.len() - REMOTE_INSTRUMENT_TRAILER_MARKER.len();
    if payload[marker_start..] != REMOTE_INSTRUMENT_TRAILER_MARKER {
      return Ok((payload, &[]));
    }
    if marker_start < 4 {
      return Err(WireError::InvalidFormat);
    }
    let len_start = marker_start - 4;
    let metadata_len =
      u32::from_le_bytes(payload[len_start..marker_start].try_into().map_err(|_| WireError::InvalidFormat)?) as usize;
    let metadata_start = len_start.checked_sub(metadata_len).ok_or(WireError::InvalidFormat)?;
    let frame_payload = &payload[..metadata_start];
    let metadata = &payload[metadata_start..len_start];
    Ok((frame_payload, metadata))
  }

  pub(super) fn now_millis(&self) -> u64 {
    self.system.upgrade().map(|s| s.state().monotonic_now().as_millis() as u64).unwrap_or(0)
  }

  fn target_authority(node: &RemoteNodeId) -> Option<String> {
    node.port().map(|port| format!("{}:{port}", node.host()))
  }

  async fn register_peer_authority(&self, remote_address: &str, authority: &str) {
    let mut peers = self.peer_authorities.lock().await;
    peers.insert(remote_address.to_string(), authority.to_string());
  }

  async fn resolve_peer_authority(&self, remote_address: &str) -> Option<String> {
    let peers = self.peer_authorities.lock().await;
    peers.get(remote_address).cloned()
  }

  async fn register_system_correlation(&self, correlation_id: CorrelationId, authority: &str) {
    let mut correlations = self.system_correlations.lock().await;
    correlations.insert(correlation_id.to_u128(), authority.to_string());
  }

  async fn resolve_system_correlation_authority(&self, correlation_id: CorrelationId) -> Option<String> {
    let correlations = self.system_correlations.lock().await;
    correlations.get(&correlation_id.to_u128()).cloned()
  }

  pub(super) async fn handle_inbound_frame(&self, frame: InboundFrame) {
    if frame.payload().len() < 2 {
      return;
    }
    match frame.payload()[1] {
      | HANDSHAKE_INIT_FRAME_KIND | HANDSHAKE_ACK_FRAME_KIND => {
        if let Err(error) = self
          .process_handshake_payload_with_remote(frame.payload().to_vec(), Some(frame.remote_address().to_string()))
          .await
        {
          self.emit_error(format!("failed to decode handshake: {error:?}"));
        }
      },
      | REMOTING_MESSAGE_FRAME_KIND => {
        let start = if self.remote_instruments.serialization_timing_enabled() { Some(Instant::now()) } else { None };
        let (frame_payload, metadata) = match Self::split_remote_instrument_metadata(frame.payload()) {
          | Ok(result) => result,
          | Err(error) => {
            self.emit_error(format!("failed to parse remote instrument metadata: {error:?}"));
            return;
          },
        };
        if let Err(error) = self.remote_instruments.read_metadata(metadata) {
          self.emit_error(format!("failed to decode remote instrument metadata: {error:?}"));
          return;
        }
        match RemotingEnvelope::decode_frame(frame_payload, frame.correlation_id()) {
          | Ok(envelope) => {
            let deserialization_nanos = start.map_or(0, |started| started.elapsed().as_nanos() as u64);
            self.remote_instruments.message_received(frame.payload().len(), deserialization_nanos);
            self.deliver_inbound(envelope).await;
          },
          | Err(error) => self.emit_error(format!("failed to decode envelope: {error:?}")),
        }
      },
      | SYSTEM_MESSAGE_FRAME_KIND | ACKED_DELIVERY_ACK_FRAME_KIND | ACKED_DELIVERY_NACK_FRAME_KIND => {
        self.handle_acked_delivery_frame(frame).await
      },
      | FLUSH_FRAME_KIND => self.handle_flush_frame(frame).await,
      | FLUSH_ACK_FRAME_KIND => self.handle_flush_ack_frame(frame).await,
      | HEARTBEAT_FRAME_KIND => self.handle_heartbeat_frame(frame).await,
      | HEARTBEAT_RSP_FRAME_KIND => self.handle_heartbeat_rsp_frame(frame).await,
      | _ => {},
    }
  }

  fn local_node(&self) -> RemoteNodeId {
    RemoteNodeId::new(self.system_name.clone(), self.host.clone(), Some(self.port), self.local_uid)
  }

  async fn next_system_sequence(&self, authority: &str) -> u64 {
    let mut sequences = self.system_sequences.lock().await;
    let next = sequences.get(authority).copied().unwrap_or(0).saturating_add(1);
    sequences.insert(authority.to_string(), next);
    next
  }

  async fn register_pending_system_envelope(&self, authority: &str, envelope: SystemMessageEnvelope) {
    let correlation_id = envelope.correlation_id();
    let mut pending = self.pending_system.lock().await;
    let entry = pending.entry(authority.to_string()).or_insert_with(BTreeMap::new);
    entry.insert(envelope.sequence_no(), envelope);
    drop(pending);
    self.register_system_correlation(correlation_id, authority).await;
  }

  async fn clear_pending_system_envelopes(&self, authority: &str, ack_seq_no: u64) {
    let removed_correlations = {
      let mut pending = self.pending_system.lock().await;
      let mut removed = Vec::new();
      if let Some(entries) = pending.get_mut(authority) {
        for envelope in entries.values() {
          if envelope.sequence_no() <= ack_seq_no {
            removed.push(envelope.correlation_id().to_u128());
          }
        }
        entries.retain(|seq, _| *seq > ack_seq_no);
        if entries.is_empty() {
          pending.remove(authority);
        }
      }
      removed
    };
    if removed_correlations.is_empty() {
      return;
    }
    let mut correlations = self.system_correlations.lock().await;
    for correlation in removed_correlations {
      correlations.remove(&correlation);
    }
  }

  // NOTE: CQS 例外 — authority 解決と heartbeat ディスパッチは受信パスで常に
  // ペアになるため、分離するとすべての呼び出し元に副作用責務が漏れてしまう。
  async fn resolve_control_authority(&self, frame: &InboundFrame) -> Option<String> {
    if !frame.correlation_id().is_nil()
      && let Some(authority) = self.resolve_system_correlation_authority(frame.correlation_id()).await
    {
      self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat(authority.clone()));
      return Some(authority);
    }
    let authority = self.resolve_peer_authority(frame.remote_address()).await?;
    self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat(authority.clone()));
    Some(authority)
  }

  async fn resolve_system_message_reply_authority(
    &self,
    frame: &InboundFrame,
    envelope: &SystemMessageEnvelope,
  ) -> Option<String> {
    let authority = Self::target_authority(envelope.ack_reply_to())?;
    self.register_peer_authority(frame.remote_address(), &authority).await;
    self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat_rsp(authority.clone(), envelope.ack_reply_to().uid()));
    Some(authority)
  }

  async fn resolve_flush_authority(&self, frame: &InboundFrame) -> Option<String> {
    let authority = self.resolve_peer_authority(frame.remote_address()).await?;
    self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat(authority.clone()));
    Some(authority)
  }

  async fn resolve_flush_ack_authority(&self, frame: &InboundFrame) -> Option<String> {
    let authority = self.resolve_peer_authority(frame.remote_address()).await?;
    self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat(authority.clone()));
    Some(authority)
  }

  async fn resolve_heartbeat_authority(&self, frame: &InboundFrame) -> String {
    self.resolve_peer_authority(frame.remote_address()).await.unwrap_or_else(|| frame.remote_address().to_string())
  }

  async fn classify_inbound_sequence(&self, authority: &str, sequence_no: u64) -> InboundSystemSequenceResult {
    let mut sequences = self.inbound_sequences.lock().await;
    let expected = sequences.entry(authority.to_string()).or_insert(1);
    let current_expected = *expected;
    if sequence_no == current_expected {
      *expected = current_expected.saturating_add(1);
      return InboundSystemSequenceResult::Deliver;
    }
    if sequence_no < current_expected {
      return InboundSystemSequenceResult::Duplicate { ack_sequence_no: current_expected.saturating_sub(1) };
    }
    InboundSystemSequenceResult::Missing { highest_acked_sequence_no: current_expected.saturating_sub(1) }
  }

  async fn pending_system_count(&self, authority: &str) -> u32 {
    let pending = self.pending_system.lock().await;
    pending.get(authority).map(|entries| entries.len() as u32).unwrap_or(0)
  }

  async fn resend_pending_system_messages(&self, authority: &str) -> Result<(), TransportError> {
    let pending_messages = {
      let pending = self.pending_system.lock().await;
      pending
        .get(authority)
        .map(|entries| entries.values().cloned().collect::<Vec<SystemMessageEnvelope>>())
        .unwrap_or_default()
    };
    if pending_messages.is_empty() {
      return Ok(());
    }
    let channel = self.ensure_channel(authority).await?;
    for envelope in pending_messages {
      let payload = AckedDelivery::SystemMessage(AllocBox::new(envelope.clone())).encode_frame();
      self.transport.with_write(|t| t.send(&channel, &payload, envelope.correlation_id()))?;
    }
    Ok(())
  }

  async fn send_acked_delivery(
    &self,
    authority: &str,
    payload: AckedDelivery,
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    let channel = self.ensure_channel(authority).await?;
    let frame = payload.encode_frame();
    self.transport.with_write(|t| t.send(&channel, &frame, correlation_id))
  }

  async fn send_flush_ack(&self, authority: &str, expected_acks: u32) -> Result<(), TransportError> {
    let channel = self.ensure_channel(authority).await?;
    let payload = FlushAck::new(expected_acks).encode_frame();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))
  }

  async fn send_flush(&self, authority: &str) -> Result<(), TransportError> {
    let channel = self.ensure_channel(authority).await?;
    let payload = Flush::new().encode_frame();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))
  }

  async fn send_heartbeat_rsp(&self, authority: &str, uid: u64) -> Result<(), TransportError> {
    let channel = self.ensure_channel(authority).await?;
    let payload = HeartbeatRsp::new(authority, uid).encode_frame();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))
  }

  async fn flush_target_authorities(&self) -> Vec<String> {
    let mut targets = BTreeSet::new();
    {
      let channels = self.channels.lock().await;
      targets.extend(channels.keys().cloned());
    }
    {
      let pending = self.pending_system.lock().await;
      targets.extend(pending.keys().cloned());
    }
    targets.into_iter().collect()
  }

  pub(super) async fn run_shutdown_flush(&self) {
    let targets = self.flush_target_authorities().await;
    if targets.is_empty() {
      return;
    }
    {
      let mut observations = self.flush_ack_observations.lock().await;
      for authority in &targets {
        observations.insert(authority.clone(), u32::MAX);
      }
    }
    let started = Instant::now();
    let timeout = self.shutdown_flush_timeout;
    loop {
      let awaiting = {
        let observations = self.flush_ack_observations.lock().await;
        targets
          .iter()
          .filter(|authority| observations.get(*authority).copied().unwrap_or(u32::MAX) > 0)
          .cloned()
          .collect::<Vec<String>>()
      };
      if awaiting.is_empty() {
        break;
      }
      if started.elapsed() >= timeout {
        self.emit_error(format!(
          "shutdown flush timed out after {:?}; remaining authorities: {}",
          timeout,
          awaiting.join(",")
        ));
        break;
      }
      for authority in awaiting {
        if let Err(error) = self.send_flush(&authority).await {
          self.emit_error(format!("failed to send shutdown flush for {authority}: {error:?}"));
        }
      }
      sleep(OUTBOUND_IDLE_DELAY).await;
    }
  }

  async fn handle_acked_delivery_frame(&self, frame: InboundFrame) {
    match AckedDelivery::decode_frame(frame.payload(), frame.correlation_id()) {
      | Ok(AckedDelivery::SystemMessage(envelope)) => {
        let Some(authority) = self.resolve_system_message_reply_authority(&frame, &envelope).await else {
          self.emit_error("failed to resolve system-message ack authority".to_string());
          return;
        };
        let sequence_no = envelope.sequence_no();
        let reply = match self.classify_inbound_sequence(&authority, sequence_no).await {
          | InboundSystemSequenceResult::Deliver => {
            self.deliver_inbound((*envelope).into_remoting_envelope()).await;
            AckedDelivery::ack(sequence_no)
          },
          | InboundSystemSequenceResult::Duplicate { ack_sequence_no } => AckedDelivery::ack(ack_sequence_no),
          | InboundSystemSequenceResult::Missing { highest_acked_sequence_no } => {
            AckedDelivery::nack(highest_acked_sequence_no)
          },
        };
        if let Err(error) = self.send_acked_delivery(&authority, reply, frame.correlation_id()).await {
          self.emit_error(format!("failed to send system-message control reply: {error:?}"));
        }
      },
      | Ok(AckedDelivery::Ack { sequence_no }) => {
        let Some(authority) = self.resolve_control_authority(&frame).await else {
          self.emit_error("failed to resolve ack authority".to_string());
          return;
        };
        self.clear_pending_system_envelopes(&authority, sequence_no).await;
      },
      | Ok(AckedDelivery::Nack { sequence_no }) => {
        let Some(authority) = self.resolve_control_authority(&frame).await else {
          self.emit_error("failed to resolve nack authority".to_string());
          return;
        };
        self.clear_pending_system_envelopes(&authority, sequence_no).await;
        if let Err(error) = self.resend_pending_system_messages(&authority).await {
          self.emit_error(format!("failed to resend system message: {error:?}"));
        }
      },
      | Err(error) => self.emit_error(format!("failed to decode acked-delivery frame: {error:?}")),
    }
  }

  async fn handle_heartbeat_frame(&self, frame: InboundFrame) {
    let authority = self.resolve_heartbeat_authority(&frame).await;
    self.register_peer_authority(frame.remote_address(), &authority).await;
    match Heartbeat::decode_frame(frame.payload(), authority.clone()) {
      | Ok(heartbeat) => {
        self.dispatch_remote_watcher(RemoteWatcherCommand::Heartbeat { heartbeat });
        if let Err(error) = self.send_heartbeat_rsp(&authority, self.local_uid).await {
          self.emit_error(format!("failed to send heartbeat-rsp frame: {error:?}"));
        }
      },
      | Err(error) => self.emit_error(format!("failed to decode heartbeat frame: {error:?}")),
    }
  }

  async fn handle_heartbeat_rsp_frame(&self, frame: InboundFrame) {
    let authority = self.resolve_heartbeat_authority(&frame).await;
    self.register_peer_authority(frame.remote_address(), &authority).await;
    match HeartbeatRsp::decode_frame(frame.payload(), authority.clone()) {
      | Ok(heartbeat_rsp) => self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat_rsp(
        heartbeat_rsp.authority().to_string(),
        heartbeat_rsp.uid(),
      )),
      | Err(error) => self.emit_error(format!("failed to decode heartbeat-rsp frame: {error:?}")),
    }
  }

  async fn handle_flush_frame(&self, frame: InboundFrame) {
    if let Err(error) = Flush::decode_frame(frame.payload()) {
      self.emit_error(format!("failed to decode flush frame: {error:?}"));
      return;
    }
    let Some(authority) = self.resolve_flush_authority(&frame).await else {
      self.emit_error("failed to resolve flush authority".to_string());
      return;
    };
    let expected_acks = self.pending_system_count(&authority).await;
    if let Err(error) = self.send_flush_ack(&authority, expected_acks).await {
      self.emit_error(format!("failed to send flush ack: {error:?}"));
    }
  }

  async fn handle_flush_ack_frame(&self, frame: InboundFrame) {
    let flush_ack = match FlushAck::decode_frame(frame.payload()) {
      | Ok(flush_ack) => flush_ack,
      | Err(error) => {
        self.emit_error(format!("failed to decode flush-ack frame: {error:?}"));
        return;
      },
    };
    let Some(authority) = self.resolve_flush_ack_authority(&frame).await else {
      self.emit_error("failed to resolve flush-ack authority".to_string());
      return;
    };
    self.flush_ack_observations.lock().await.insert(authority, flush_ack.expected_acks());
  }

  pub(super) async fn process_handshake_payload_with_remote(
    &self,
    payload: Vec<u8>,
    remote_address: Option<String>,
  ) -> Result<(), WireError> {
    let frame = HandshakeFrame::decode(&payload)?;
    if let Some(port) = frame.port() {
      let authority = format!("{}:{port}", frame.host());
      if let Some(remote_address) = remote_address {
        self.register_peer_authority(&remote_address, &authority).await;
      }
      self.dispatch_remote_watcher(RemoteWatcherCommand::heartbeat_rsp(authority.clone(), frame.uid()));
      let remote =
        RemoteNodeId::new(frame.system_name().to_string(), frame.host().to_string(), frame.port(), frame.uid());
      let accept = self.coordinator.with_write(|m| {
        m.handle(EndpointAssociationCommand::HandshakeAccepted {
          authority:   authority.clone(),
          remote_node: remote,
          now:         self.now_millis(),
        })
      });
      let should_send_ack = matches!(frame.kind(), HandshakeKind::Offer)
        && accept.effects.iter().any(|effect| {
          matches!(effect, EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Connected { .. }))
        });
      if should_send_ack && let Err(error) = self.send_handshake_ack(&authority).await {
        self.emit_error(format!("failed to send handshake ack: {error:?}"));
      }
      if let Err(error) = self.process_effects(accept.effects).await {
        self.emit_error(format!("failed to process effects after handshake accept: {error:?}"));
      }
    }
    Ok(())
  }

  async fn send_handshake_ack(&self, authority: &str) -> Result<(), TransportError> {
    let channel = self.ensure_channel(authority).await?;
    let ack = HandshakeFrame::new(HandshakeKind::Ack, &self.system_name, &self.host, Some(self.port), self.local_uid);
    let payload = ack.encode();
    self.transport.with_write(|t| t.send(&channel, &payload, CorrelationId::nil()))
  }

  async fn deliver_inbound(&self, envelope: RemotingEnvelope) {
    match self.reader.decode(envelope) {
      | Ok(inbound) => {
        if let Err(error) = self.reader.deliver(inbound) {
          self.emit_error(format!("failed to deliver inbound envelope: {error:?}"));
        }
      },
      | Err(error) => self.emit_error(format!("failed to deserialize inbound envelope: {error:?}")),
    }
  }
}

struct InboundHandler {
  frame_sender: mpsc::Sender<InboundFrame>,
}

impl InboundHandler {
  fn new<TB: RuntimeToolbox + 'static>(bridge: Arc<EndpointTransportBridge<TB>>, max_concurrency: usize) -> Self {
    let (frame_sender, frame_receiver) = mpsc::channel::<InboundFrame>(max_concurrency);
    let frame_receiver = Arc::new(TokioMutex::new(frame_receiver));
    for _ in 0..max_concurrency {
      let bridge = bridge.clone();
      let frame_receiver = Arc::clone(&frame_receiver);
      tokio::spawn(async move {
        loop {
          let frame = {
            let mut receiver = frame_receiver.lock().await;
            receiver.recv().await
          };
          match frame {
            | Some(frame) => bridge.handle_inbound_frame(frame).await,
            | None => break,
          }
        }
      });
    }
    Self { frame_sender }
  }
}

impl TransportInbound for InboundHandler {
  fn on_frame(&mut self, frame: InboundFrame) {
    let _ = self.frame_sender.try_send(frame);
  }
}
