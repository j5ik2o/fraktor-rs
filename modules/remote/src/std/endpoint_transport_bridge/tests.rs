use alloc::{
  boxed::Box as AllocBox,
  collections::BTreeMap,
  string::{String, ToString},
  sync::Arc,
  vec::Vec,
};
use core::{
  convert::TryFrom,
  sync::atomic::{AtomicU64, AtomicUsize, Ordering},
  time::Duration,
};
use std::sync::Mutex;

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorContextGeneric,
    actor_path::{ActorPath, ActorPathParts, GuardianKind},
  },
  error::ActorError,
  event::{
    logging::LogLevel,
    stream::{
      CorrelationId, EventStreamEvent, EventStreamSubscriber, EventStreamSubscriptionGeneric, subscriber_handle,
    },
  },
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  serialization::{
    SerializationCallScope, SerializationExtensionGeneric, SerializationExtensionSharedGeneric, SerializationSetup,
    SerializationSetupBuilder, SerializedMessage, Serializer, SerializerId, builtin::StringSerializer,
  },
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::{
  core::sync::{ArcShared, SharedAccess},
  std::runtime_toolbox::StdToolbox,
};

use super::{EndpointTransportBridge, EndpointTransportBridgeConfig, EndpointTransportBridgeHandle};
use crate::core::{
  EventPublisherGeneric, FLUSH_ACK_FRAME_KIND, Flush, FlushAck, RemoteInstrument, RemoteNodeId, WireError,
  endpoint_association::{AssociationState, EndpointAssociationCommand, QuarantineReason},
  endpoint_reader::EndpointReaderGeneric,
  endpoint_writer::{EndpointWriterGeneric, EndpointWriterSharedGeneric},
  envelope::{
    ACKED_DELIVERY_ACK_FRAME_KIND, ACKED_DELIVERY_NACK_FRAME_KIND, AckedDelivery, OutboundPriority, RemotingEnvelope,
    SYSTEM_MESSAGE_FRAME_KIND, SystemMessageEnvelope,
  },
  handshake::{HandshakeFrame, HandshakeKind},
  remoting_extension::{RemotingControlHandle, RemotingExtensionConfig},
  transport::{
    RemoteTransport, RemoteTransportShared, TransportBind, TransportChannel, TransportEndpoint, TransportError,
    TransportHandle,
    inbound::{InboundFrame, TransportInboundShared},
  },
  watcher::{HEARTBEAT_RSP_FRAME_KIND, Heartbeat, HeartbeatRsp, RemoteWatcherCommand},
};

struct NoopActor;

impl Actor<StdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, StdToolbox>,
    _message: AnyMessageViewGeneric<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
struct WatcherCommandRecorder {
  commands: Arc<Mutex<Vec<RemoteWatcherCommand>>>,
}

impl WatcherCommandRecorder {
  fn new() -> Self {
    Self { commands: Arc::new(Mutex::new(Vec::new())) }
  }

  fn snapshot(&self) -> Vec<RemoteWatcherCommand> {
    self.commands.lock().expect("watcher recorder lock").clone()
  }
}

struct WatcherCommandRecorderActor {
  recorder: WatcherCommandRecorder,
}

impl Actor<StdToolbox> for WatcherCommandRecorderActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, StdToolbox>,
    message: AnyMessageViewGeneric<'_, StdToolbox>,
  ) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<RemoteWatcherCommand>() {
      self.recorder.commands.lock().expect("watcher recorder lock").push(command.clone());
    }
    Ok(())
  }
}

#[derive(Default)]
struct InstrumentCaptureState {
  outbound_metadata_calls: usize,
  inbound_metadata_calls:  usize,
  sent_calls:              usize,
  received_calls:          usize,
}

struct CaptureInstrument {
  state: Arc<Mutex<InstrumentCaptureState>>,
}

impl CaptureInstrument {
  fn new(state: Arc<Mutex<InstrumentCaptureState>>) -> Self {
    Self { state }
  }
}

impl RemoteInstrument for CaptureInstrument {
  fn identifier(&self) -> u8 {
    1
  }

  fn remote_write_metadata(&self, buffer: &mut Vec<u8>) {
    buffer.extend_from_slice(&[0xDE, 0xAD]);
    let mut guard = self.state.lock().expect("instrument state lock");
    guard.outbound_metadata_calls += 1;
  }

  fn remote_message_sent(&self, _size: usize, _serialization_nanos: u64) {
    let mut guard = self.state.lock().expect("instrument state lock");
    guard.sent_calls += 1;
  }

  fn remote_read_metadata(&self, buffer: &[u8]) -> Result<(), WireError> {
    if buffer != [0xDE, 0xAD] {
      return Err(WireError::InvalidFormat);
    }
    let mut guard = self.state.lock().expect("instrument state lock");
    guard.inbound_metadata_calls += 1;
    Ok(())
  }

  fn remote_message_received(&self, _size: usize, _deserialization_nanos: u64) {
    let mut guard = self.state.lock().expect("instrument state lock");
    guard.received_calls += 1;
  }
}

#[derive(Clone)]
struct SentFrame {
  authority:      String,
  payload:        Vec<u8>,
  correlation_id: CorrelationId,
}

#[derive(Clone)]
struct TestTransportProbe {
  sent_frames:        Arc<Mutex<Vec<SentFrame>>>,
  open_calls:         Arc<AtomicUsize>,
  open_channel_delay: Arc<AtomicU64>,
  send_failures_left: Arc<AtomicUsize>,
  send_delay:         Arc<AtomicU64>,
  inbound_handler:    Arc<Mutex<Option<TransportInboundShared<StdToolbox>>>>,
}

impl TestTransportProbe {
  fn open_calls(&self) -> usize {
    self.open_calls.load(Ordering::Acquire)
  }

  fn set_open_delay(&self, delay: Duration) {
    self.open_channel_delay.store(delay.as_millis() as u64, Ordering::SeqCst);
  }

  fn set_send_failures_left(&self, failures_left: usize) {
    self.send_failures_left.store(failures_left, Ordering::SeqCst);
  }

  fn set_send_delay(&self, delay: Duration) {
    self.send_delay.store(delay.as_millis() as u64, Ordering::SeqCst);
  }

  fn set_inbound_handler(&self, handler: TransportInboundShared<StdToolbox>) {
    *self.inbound_handler.lock().expect("probe lock") = Some(handler);
  }

  fn emit_inbound_frame(&self, frame: InboundFrame) {
    let handler = self.inbound_handler.lock().expect("probe lock").clone();
    if let Some(handler) = handler {
      handler.with_write(|handler| handler.on_frame(frame));
    }
  }

  fn push_sent(&self, authority: String, payload: &[u8], correlation_id: CorrelationId) {
    self.sent_frames.lock().expect("probe lock").push(SentFrame {
      authority,
      payload: payload.to_vec(),
      correlation_id,
    });
  }

  fn sent_handshake_kinds_for(&self, authority: &str) -> Vec<HandshakeKind> {
    self
      .sent_frames
      .lock()
      .expect("probe lock")
      .iter()
      .filter(|frame| frame.authority == authority && frame.correlation_id == CorrelationId::nil())
      .filter_map(|frame| HandshakeFrame::decode(&frame.payload).ok().map(|decoded| decoded.kind()))
      .collect()
  }

  fn sent_handshake_kinds(&self) -> Vec<HandshakeKind> {
    self
      .sent_frames
      .lock()
      .expect("probe lock")
      .iter()
      .filter_map(|frame| HandshakeFrame::decode(&frame.payload).ok().map(|decoded| decoded.kind()))
      .collect()
  }

  fn sent_frames_for(&self, authority: &str) -> Vec<SentFrame> {
    self.sent_frames.lock().expect("probe lock").iter().filter(|frame| frame.authority == authority).cloned().collect()
  }
}

impl Default for TestTransportProbe {
  fn default() -> Self {
    Self {
      sent_frames:        Arc::new(Mutex::new(Vec::new())),
      open_calls:         Arc::new(AtomicUsize::new(0)),
      open_channel_delay: Arc::new(AtomicU64::new(0)),
      send_failures_left: Arc::new(AtomicUsize::new(0)),
      send_delay:         Arc::new(AtomicU64::new(0)),
      inbound_handler:    Arc::new(Mutex::new(None)),
    }
  }
}

struct TestTransport {
  probe:         TestTransportProbe,
  channels:      BTreeMap<u64, String>,
  next_channel:  u64,
  inbound:       Option<TransportInboundShared<StdToolbox>>,
  next_listener: u64,
}

impl TestTransport {
  fn new() -> (Self, TestTransportProbe) {
    let probe = TestTransportProbe::default();
    (
      Self {
        probe:         probe.clone(),
        channels:      BTreeMap::new(),
        next_channel:  1,
        inbound:       None,
        next_listener: 1,
      },
      probe,
    )
  }
}

impl RemoteTransport<StdToolbox> for TestTransport {
  fn scheme(&self) -> &str {
    "fraktor.test"
  }

  fn spawn_listener(&mut self, _bind: &TransportBind) -> Result<TransportHandle, TransportError> {
    let authority = format!("test-listener-{}", self.next_listener);
    self.next_listener += 1;
    Ok(TransportHandle::new(authority))
  }

  fn open_channel(&mut self, endpoint: &TransportEndpoint) -> Result<TransportChannel, TransportError> {
    self.probe.open_calls.fetch_add(1, Ordering::SeqCst);
    let delay_millis = self.probe.open_channel_delay.load(Ordering::Acquire);
    if delay_millis > 0 {
      std::thread::sleep(std::time::Duration::from_millis(delay_millis));
    }
    let id = self.next_channel;
    self.next_channel += 1;
    self.channels.insert(id, endpoint.authority().to_string());
    Ok(TransportChannel::new(id))
  }

  fn send(
    &mut self,
    channel: &TransportChannel,
    payload: &[u8],
    correlation_id: CorrelationId,
  ) -> Result<(), TransportError> {
    let failures_left = self.probe.send_failures_left.load(Ordering::Acquire);
    if failures_left > 0 {
      self.probe.send_failures_left.fetch_sub(1, Ordering::SeqCst);
      return Err(TransportError::AuthorityNotBound("injected transport send failure".into()));
    }

    let delay_millis = self.probe.send_delay.load(Ordering::Acquire);
    if delay_millis > 0 {
      std::thread::sleep(std::time::Duration::from_millis(delay_millis));
    }

    let authority =
      self.channels.get(&channel.id()).cloned().ok_or(TransportError::ChannelUnavailable(channel.id()))?;
    self.probe.push_sent(authority, payload, correlation_id);
    Ok(())
  }

  fn close(&mut self, channel: &TransportChannel) {
    self.channels.remove(&channel.id());
  }

  fn install_backpressure_hook(&mut self, _hook: crate::core::transport::TransportBackpressureHookShared) {}

  fn install_inbound_handler(&mut self, handler: TransportInboundShared<StdToolbox>) {
    self.inbound = Some(handler);
    self.probe.set_inbound_handler(self.inbound.clone().expect("inbound handler"));
  }
}

#[derive(Clone)]
struct EventRecorder {
  events: Arc<Mutex<Vec<EventStreamEvent<StdToolbox>>>>,
}

impl EventRecorder {
  fn new() -> Self {
    Self { events: Arc::new(Mutex::new(Vec::new())) }
  }

  fn snapshot(&self) -> Vec<EventStreamEvent<StdToolbox>> {
    self.events.lock().expect("recorder lock").clone()
  }
}

impl EventStreamSubscriber<StdToolbox> for EventRecorder {
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    self.events.lock().expect("recorder lock").push(event.clone());
  }
}

fn build_system() -> ActorSystemGeneric<StdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("endpoint-bridge-tests");
  let config = ActorSystemConfigGeneric::<StdToolbox>::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::<StdToolbox>::new()));
  ActorSystemGeneric::new_with_config(&props, &config).expect("actor system")
}

fn subscribe_events(
  system: &ActorSystemGeneric<StdToolbox>,
) -> (EventRecorder, EventStreamSubscriptionGeneric<StdToolbox>) {
  let recorder = EventRecorder::new();
  let subscriber = subscriber_handle(recorder.clone());
  let subscription = system.subscribe_event_stream(&subscriber);
  (recorder, subscription)
}

fn serialization_setup() -> SerializationSetup {
  let serializer_id = SerializerId::try_from(91).expect("serializer id");
  let serializer: ArcShared<dyn Serializer> = ArcShared::new(StringSerializer::new(serializer_id));
  SerializationSetupBuilder::new()
    .register_serializer("string", serializer_id, serializer)
    .expect("register serializer")
    .bind::<String>("string")
    .expect("bind string")
    .bind_remote_manifest::<String>("bridge.String")
    .expect("bind manifest")
    .set_fallback("string")
    .expect("set fallback")
    .require_manifest_for_scope(SerializationCallScope::Remote)
    .build()
    .expect("serialization setup")
}

fn serialization_extension(system: &ActorSystemGeneric<StdToolbox>) -> SerializationExtensionSharedGeneric<StdToolbox> {
  SerializationExtensionSharedGeneric::new(SerializationExtensionGeneric::new(system, serialization_setup()))
}

fn build_bridge(
  handshake_timeout: Duration,
) -> (Arc<EndpointTransportBridge<StdToolbox>>, TestTransportProbe, ActorSystemGeneric<StdToolbox>) {
  let (bridge, probe, system, _control) = build_bridge_with_control(handshake_timeout);
  (bridge, probe, system)
}

fn build_bridge_with_control(
  handshake_timeout: Duration,
) -> (
  Arc<EndpointTransportBridge<StdToolbox>>,
  TestTransportProbe,
  ActorSystemGeneric<StdToolbox>,
  RemotingControlHandle<StdToolbox>,
) {
  let system = build_system();
  let serialization = serialization_extension(&system);
  let writer = EndpointWriterSharedGeneric::new(EndpointWriterGeneric::new(system.downgrade(), serialization.clone()));
  let reader = ArcShared::new(EndpointReaderGeneric::new(system.downgrade(), serialization));
  let (transport, probe) = TestTransport::new();
  let control = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  let config = EndpointTransportBridgeConfig {
    system: system.downgrade(),
    control: control.clone(),
    writer,
    reader,
    transport: RemoteTransportShared::new(Box::new(transport)),
    event_publisher: EventPublisherGeneric::new(system.downgrade()),
    canonical_host: "127.0.0.1".to_string(),
    canonical_port: 2552,
    system_name: "local-system".to_string(),
    remote_instruments: Vec::new(),
    handshake_timeout,
    shutdown_flush_timeout: handshake_timeout,
  };
  (EndpointTransportBridge::new(config), probe, system, control)
}

fn build_bridge_with_instruments(
  handshake_timeout: Duration,
  remote_instruments: Vec<Arc<dyn RemoteInstrument>>,
) -> (Arc<EndpointTransportBridge<StdToolbox>>, TestTransportProbe, ActorSystemGeneric<StdToolbox>) {
  let system = build_system();
  let serialization = serialization_extension(&system);
  let writer = EndpointWriterSharedGeneric::new(EndpointWriterGeneric::new(system.downgrade(), serialization.clone()));
  let reader = ArcShared::new(EndpointReaderGeneric::new(system.downgrade(), serialization));
  let (transport, probe) = TestTransport::new();
  let control = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  let config = EndpointTransportBridgeConfig {
    system: system.downgrade(),
    control: control.clone(),
    writer,
    reader,
    transport: RemoteTransportShared::new(Box::new(transport)),
    event_publisher: EventPublisherGeneric::new(system.downgrade()),
    canonical_host: "127.0.0.1".to_string(),
    canonical_port: 2552,
    system_name: "local-system".to_string(),
    remote_instruments,
    handshake_timeout,
    shutdown_flush_timeout: handshake_timeout,
  };
  (EndpointTransportBridge::new(config), probe, system)
}

fn spawn_bridge(
  handshake_timeout: Duration,
) -> (EndpointTransportBridgeHandle, TestTransportProbe, ActorSystemGeneric<StdToolbox>) {
  let (handle, probe, system, _control) = spawn_bridge_with_control(handshake_timeout);
  (handle, probe, system)
}

fn spawn_bridge_with_control(
  handshake_timeout: Duration,
) -> (
  EndpointTransportBridgeHandle,
  TestTransportProbe,
  ActorSystemGeneric<StdToolbox>,
  RemotingControlHandle<StdToolbox>,
) {
  let system = build_system();
  let serialization = serialization_extension(&system);
  let writer = EndpointWriterSharedGeneric::new(EndpointWriterGeneric::new(system.downgrade(), serialization.clone()));
  let reader = ArcShared::new(EndpointReaderGeneric::new(system.downgrade(), serialization));
  let (transport, probe) = TestTransport::new();
  let control = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  let config = EndpointTransportBridgeConfig {
    system: system.downgrade(),
    control: control.clone(),
    writer,
    reader,
    transport: RemoteTransportShared::new(Box::new(transport)),
    event_publisher: EventPublisherGeneric::new(system.downgrade()),
    canonical_host: "127.0.0.1".to_string(),
    canonical_port: 2552,
    system_name: "local-system".to_string(),
    remote_instruments: Vec::new(),
    handshake_timeout,
    shutdown_flush_timeout: handshake_timeout,
  };
  (EndpointTransportBridge::spawn(config).expect("spawn bridge"), probe, system, control)
}

fn register_remote_watcher_daemon(
  system: &ActorSystemGeneric<StdToolbox>,
  control: &RemotingControlHandle<StdToolbox>,
) -> WatcherCommandRecorder {
  let recorder = WatcherCommandRecorder::new();
  let props = PropsGeneric::from_fn({
    let recorder = recorder.clone();
    move || WatcherCommandRecorderActor { recorder: recorder.clone() }
  })
  .with_name("remote-watcher-command-recorder");
  let child = system.extended().spawn_system_actor(&props).expect("spawn watcher recorder");
  control.register_remote_watcher_daemon(child.actor_ref().clone());
  recorder
}

fn association_state(
  bridge: &EndpointTransportBridge<StdToolbox>,
  authority: &str,
) -> Option<crate::core::endpoint_association::AssociationState> {
  bridge.coordinator.with_read(|m| m.state(authority))
}

fn deferred_envelope(label: &str) -> crate::core::envelope::DeferredEnvelope {
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let recipient = ActorPath::from_parts(parts).child("svc");
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 0);
  let serializer = SerializerId::try_from(41).expect("serializer id");
  let serialized = SerializedMessage::new(serializer, None, label.as_bytes().to_vec());
  let envelope =
    RemotingEnvelope::new(recipient, remote, None, serialized, CorrelationId::nil(), OutboundPriority::User);
  crate::core::envelope::DeferredEnvelope::new(envelope)
}

fn deferred_system_envelope(label: &str) -> crate::core::envelope::DeferredEnvelope {
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let recipient = ActorPath::from_parts(parts).child("sys");
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 0);
  let serializer = SerializerId::try_from(42).expect("serializer id");
  let serialized = SerializedMessage::new(serializer, None, label.as_bytes().to_vec());
  let envelope =
    RemotingEnvelope::new(recipient, remote, None, serialized, CorrelationId::from_u128(9), OutboundPriority::System);
  crate::core::envelope::DeferredEnvelope::new(envelope)
}

fn sample_system_message_envelope(
  sequence_no: u64,
  correlation_id: CorrelationId,
  ack_reply_to: RemoteNodeId,
) -> SystemMessageEnvelope {
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let recipient = ActorPath::from_parts(parts).child("sys");
  let remote_node = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 0);
  let serializer = SerializerId::try_from(99).expect("serializer id");
  let serialized = SerializedMessage::new(serializer, None, b"system".to_vec());
  SystemMessageEnvelope::new(recipient, remote_node, None, serialized, correlation_id, sequence_no, ack_reply_to)
}

async fn associate(
  bridge: &EndpointTransportBridge<StdToolbox>,
  authority: &str,
  endpoint: TransportEndpoint,
  now: u64,
) {
  let result = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::Associate { authority: authority.to_string(), endpoint, now })
  });
  bridge.process_effects(result.effects).await.expect("associate effects");
}

#[tokio::test(flavor = "current_thread")]
async fn start_handshake_keeps_associating_until_ack_arrives() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:4101";
  let endpoint = TransportEndpoint::new(authority.to_string());

  associate(&bridge, authority, endpoint.clone(), bridge.now_millis()).await;
  assert_eq!(association_state(&bridge, authority), Some(AssociationState::Associating { endpoint }));
  assert_eq!(probe.sent_handshake_kinds_for(authority), vec![HandshakeKind::Offer]);
  let sent_offer = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter(|frame| frame.correlation_id == CorrelationId::nil())
    .find_map(|frame| HandshakeFrame::decode(&frame.payload).ok())
    .expect("sent offer");
  assert_eq!(sent_offer.kind(), HandshakeKind::Offer);
  assert_ne!(sent_offer.uid(), 0);

  tokio::time::sleep(Duration::from_millis(40)).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Associating { .. })));
}

#[tokio::test(flavor = "current_thread")]
async fn receiving_offer_replies_ack_and_marks_connected() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(500));
  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(4201), 42);

  bridge.process_handshake_payload_with_remote(offer.encode(), None).await.expect("offer processing");

  assert_eq!(probe.sent_handshake_kinds_for("127.0.0.1:4201"), vec![HandshakeKind::Ack]);
  let sent_ack = probe
    .sent_frames_for("127.0.0.1:4201")
    .into_iter()
    .filter(|frame| frame.correlation_id == CorrelationId::nil())
    .find_map(|frame| HandshakeFrame::decode(&frame.payload).ok())
    .expect("sent ack");
  assert_eq!(sent_ack.kind(), HandshakeKind::Ack);
  assert_ne!(sent_ack.uid(), 0);
  let state = association_state(&bridge, "127.0.0.1:4201").expect("state");
  match state {
    | AssociationState::Connected { remote } => {
      assert_eq!(remote, RemoteNodeId::new("remote-system", "127.0.0.1", Some(4201), 42));
    },
    | other => panic!("expected connected state, got {other:?}"),
  }
}

#[tokio::test(flavor = "current_thread")]
async fn receiving_offer_does_not_reply_ack_while_gated() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(500));
  let authority = "127.0.0.1:4202";

  bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::Gate { authority: authority.to_string(), resume_at: Some(999), now: 1 })
  });

  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(4202), 42);
  bridge.process_handshake_payload_with_remote(offer.encode(), None).await.expect("offer processing");

  assert_eq!(probe.sent_handshake_kinds_for(authority), Vec::new());
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Gated { .. })));
}

#[tokio::test(flavor = "current_thread")]
async fn receiving_offer_does_not_reply_ack_while_quarantined() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(500));
  let authority = "127.0.0.1:4203";

  bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::Quarantine {
      authority: authority.to_string(),
      reason:    QuarantineReason::new("uid mismatch"),
      resume_at: Some(999),
      now:       1,
    })
  });

  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(4203), 42);
  bridge.process_handshake_payload_with_remote(offer.encode(), None).await.expect("offer processing");

  assert_eq!(probe.sent_handshake_kinds_for(authority), Vec::new());
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Quarantined { .. })));
}

#[tokio::test(flavor = "current_thread")]
async fn handshake_timeout_moves_to_gated_and_recover_plus_ack_connects_again() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(20));
  let authority = "127.0.0.1:4301";
  let endpoint = TransportEndpoint::new(authority.to_string());

  associate(&bridge, authority, endpoint.clone(), bridge.now_millis()).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Associating { .. })));

  tokio::time::sleep(Duration::from_millis(80)).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Gated { .. })));

  let recover = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::Recover {
      authority: authority.to_string(),
      endpoint:  Some(endpoint),
      now:       bridge.now_millis(),
    })
  });
  bridge.process_effects(recover.effects).await.expect("recover effects");

  let offer_count =
    probe.sent_handshake_kinds_for(authority).iter().filter(|kind| matches!(kind, HandshakeKind::Offer)).count();
  assert!(offer_count >= 2);

  let ack = HandshakeFrame::new(HandshakeKind::Ack, "remote-system", "127.0.0.1", Some(4301), 99);
  bridge.process_handshake_payload_with_remote(ack.encode(), None).await.expect("ack processing");
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Connected { .. })));
}

#[tokio::test(flavor = "current_thread")]
async fn handshake_timeout_gate_has_no_resume_deadline() {
  let (bridge, _probe, _system) = build_bridge(Duration::from_millis(20));
  let authority = "127.0.0.1:4302";
  let endpoint = TransportEndpoint::new(authority.to_string());

  associate(&bridge, authority, endpoint, bridge.now_millis()).await;
  tokio::time::sleep(Duration::from_millis(80)).await;

  match association_state(&bridge, authority) {
    | Some(AssociationState::Gated { resume_at }) => assert!(resume_at.is_none()),
    | other => panic!("expected gated state, got {other:?}"),
  }
}

#[tokio::test(flavor = "current_thread")]
async fn handshake_timeout_emits_error_log_when_discarding_deferred_envelopes() {
  let (bridge, _probe, system) = build_bridge(Duration::from_millis(20));
  let (recorder, _subscription) = subscribe_events(&system);
  let authority = "127.0.0.1:4303";
  let endpoint = TransportEndpoint::new(authority.to_string());

  associate(&bridge, authority, endpoint, bridge.now_millis()).await;
  bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::EnqueueDeferred {
      authority: authority.to_string(),
      envelope:  Box::new(deferred_envelope("pending-timeout-message")),
    })
  });

  tokio::time::sleep(Duration::from_millis(80)).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Gated { .. })));

  let expected = format!("discarded deferred envelopes for {authority}");
  let events = recorder.snapshot();
  assert!(events.iter().any(
    |event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Error && log.message() == expected)
  ));
}

#[tokio::test(flavor = "current_thread")]
async fn stale_handshake_timeout_does_not_gate_new_attempt() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(100));
  let authority = "127.0.0.1:4401";
  let endpoint = TransportEndpoint::new(authority.to_string());

  associate(&bridge, authority, endpoint.clone(), bridge.now_millis()).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Associating { .. })));

  tokio::time::sleep(Duration::from_millis(20)).await;
  let recover = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::Recover {
      authority: authority.to_string(),
      endpoint:  Some(endpoint),
      now:       bridge.now_millis(),
    })
  });
  bridge.process_effects(recover.effects).await.expect("recover effects");

  let offer_count =
    probe.sent_handshake_kinds_for(authority).iter().filter(|kind| matches!(kind, HandshakeKind::Offer)).count();
  assert!(offer_count >= 2);

  tokio::time::sleep(Duration::from_millis(90)).await;
  assert!(
    matches!(association_state(&bridge, authority), Some(AssociationState::Associating { .. })),
    "stale timeout from the previous attempt must not gate the active handshake"
  );

  tokio::time::sleep(Duration::from_millis(40)).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Gated { .. })));
}

#[tokio::test]
async fn ensure_channel_open_is_atomic_under_concurrent_flushes() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:4601";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(4601), 9);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  probe.set_open_delay(Duration::from_millis(20));
  let effect1 = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_envelope("first")),
      })
    })
    .effects;
  let effect2 = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_envelope("second")),
      })
    })
    .effects;

  let bridge1 = bridge.clone();
  let bridge2 = bridge.clone();
  let t1 = tokio::spawn(async move { bridge1.process_effects(effect1).await });
  let t2 = tokio::spawn(async move { bridge2.process_effects(effect2).await });

  let (r1, r2) = tokio::join!(t1, t2);
  r1.expect("first deliver task").expect("first deliver");
  r2.expect("second deliver task").expect("second deliver");
  assert_eq!(probe.open_calls(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn process_handshake_payload_logs_process_effects_error() {
  let (bridge, probe, system) = build_bridge(Duration::from_millis(500));
  let (recorder, _subscription) = subscribe_events(&system);
  let authority = "127.0.0.1:4701";

  probe.set_send_failures_left(2);
  bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::EnqueueDeferred {
      authority: authority.to_string(),
      envelope:  Box::new(deferred_envelope("pending")),
    })
  });

  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(4701), 12);
  bridge.process_handshake_payload_with_remote(offer.encode(), None).await.expect("offer processing");

  let events = recorder.snapshot();
  assert!(events.iter().any(|event| {
    matches!(event, EventStreamEvent::Log(log) if log.message().contains("failed to process effects after handshake accept"))
  }));
}

#[tokio::test(flavor = "current_thread")]
async fn outbound_system_message_is_wrapped_as_acked_delivery_frame() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:25520";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  let deliver = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_system_envelope("system-payload")),
      })
    })
    .effects;
  bridge.process_effects(deliver).await.expect("deliver effects");

  let system_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .find(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .expect("system-message frame");
  let decoded = AckedDelivery::decode_frame(&system_frame.payload, system_frame.correlation_id).expect("decode");
  match decoded {
    | AckedDelivery::SystemMessage(envelope) => assert_eq!(envelope.sequence_no(), 1),
    | other => panic!("unexpected frame payload: {other:?}"),
  }
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_system_message_replies_with_ack_frame() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:4902";
  let peer_address = "127.0.0.1:59999";
  let sequence_no = 1;
  let ack_reply_to = RemoteNodeId::new("local-system", "127.0.0.1", Some(4902), 0);
  let payload = AckedDelivery::SystemMessage(AllocBox::new(sample_system_message_envelope(
    sequence_no,
    CorrelationId::from_u128(7),
    ack_reply_to,
  )))
  .encode_frame();

  bridge
    .handle_inbound_frame(InboundFrame::new("test-listener", peer_address, payload, CorrelationId::from_u128(7)))
    .await;

  let ack_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .find(|frame| frame.payload.get(1) == Some(&ACKED_DELIVERY_ACK_FRAME_KIND))
    .expect("ack frame");
  let ack = AckedDelivery::decode_frame(&ack_frame.payload, ack_frame.correlation_id).expect("ack decode");
  assert!(matches!(ack, AckedDelivery::Ack { sequence_no: n } if n == sequence_no));
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_system_message_gap_then_recovery_emits_nack_then_acks() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:4904";
  let peer_address = "127.0.0.1:61200";
  let ack_reply_to = RemoteNodeId::new("local-system", "127.0.0.1", Some(4904), 88);

  for (sequence_no, correlation_id) in
    [(2, CorrelationId::from_u128(21)), (1, CorrelationId::from_u128(22)), (2, CorrelationId::from_u128(23))]
  {
    let payload = AckedDelivery::SystemMessage(AllocBox::new(sample_system_message_envelope(
      sequence_no,
      correlation_id,
      ack_reply_to.clone(),
    )))
    .encode_frame();
    bridge.handle_inbound_frame(InboundFrame::new("test-listener", peer_address, payload, correlation_id)).await;
  }

  let control_replies: Vec<AckedDelivery> = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter_map(|frame| match frame.payload.get(1) {
      | Some(kind) if *kind == ACKED_DELIVERY_ACK_FRAME_KIND || *kind == ACKED_DELIVERY_NACK_FRAME_KIND => {
        Some(AckedDelivery::decode_frame(&frame.payload, frame.correlation_id).expect("decode control reply"))
      },
      | _ => None,
    })
    .collect();
  assert!(matches!(control_replies.as_slice(), [
    AckedDelivery::Nack { sequence_no: 0 },
    AckedDelivery::Ack { sequence_no: 1 },
    AckedDelivery::Ack { sequence_no: 2 }
  ]));
}

#[tokio::test(flavor = "current_thread")]
async fn duplicate_inbound_system_message_replies_with_latest_contiguous_ack() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:4905";
  let peer_address = "127.0.0.1:61201";
  let ack_reply_to = RemoteNodeId::new("local-system", "127.0.0.1", Some(4905), 91);

  for (sequence_no, correlation_id) in
    [(1, CorrelationId::from_u128(31)), (2, CorrelationId::from_u128(32)), (1, CorrelationId::from_u128(33))]
  {
    let payload = AckedDelivery::SystemMessage(AllocBox::new(sample_system_message_envelope(
      sequence_no,
      correlation_id,
      ack_reply_to.clone(),
    )))
    .encode_frame();
    bridge.handle_inbound_frame(InboundFrame::new("test-listener", peer_address, payload, correlation_id)).await;
  }

  let ack_replies: Vec<AckedDelivery> = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter(|frame| frame.payload.get(1) == Some(&ACKED_DELIVERY_ACK_FRAME_KIND))
    .map(|frame| AckedDelivery::decode_frame(&frame.payload, frame.correlation_id).expect("decode ack reply"))
    .collect();
  assert!(matches!(ack_replies.as_slice(), [
    AckedDelivery::Ack { sequence_no: 1 },
    AckedDelivery::Ack { sequence_no: 2 },
    AckedDelivery::Ack { sequence_no: 2 }
  ]));
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_system_message_emits_watcher_heartbeat_rsp() {
  let (bridge, _probe, system, control) = build_bridge_with_control(Duration::from_millis(200));
  let recorder = register_remote_watcher_daemon(&system, &control);
  let authority = "127.0.0.1:4903";
  let peer_address = "127.0.0.1:62234";
  let payload = AckedDelivery::SystemMessage(AllocBox::new(sample_system_message_envelope(
    3,
    CorrelationId::from_u128(11),
    RemoteNodeId::new("local-system", "127.0.0.1", Some(4903), 77),
  )))
  .encode_frame();

  bridge
    .handle_inbound_frame(InboundFrame::new("test-listener", peer_address, payload, CorrelationId::from_u128(11)))
    .await;

  let commands = recorder.snapshot();
  assert!(commands.iter().any(|command| {
    matches!(
      command,
      RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp }
      if heartbeat_rsp.authority == authority && heartbeat_rsp.uid == 77
    )
  }));
}

#[tokio::test(flavor = "current_thread")]
async fn flush_ack_reports_pending_system_message_count() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:25520";
  let peer_address = "127.0.0.1:60123";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  let deliver = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_system_envelope("needs-ack")),
      })
    })
    .effects;
  bridge.process_effects(deliver).await.expect("deliver effects");

  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(25520), 42);
  bridge
    .handle_inbound_frame(InboundFrame::new("test-listener", peer_address, offer.encode(), CorrelationId::nil()))
    .await;

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      Flush::new().encode_frame(),
      CorrelationId::nil(),
    ))
    .await;

  let flush_ack_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .rfind(|frame| frame.payload.get(1) == Some(&FLUSH_ACK_FRAME_KIND))
    .expect("flush-ack frame");
  let flush_ack = FlushAck::decode_frame(&flush_ack_frame.payload).expect("flush-ack decode");
  assert_eq!(flush_ack.expected_acks(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_flush_waits_for_flush_ack_observation() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(400));
  let authority = "127.0.0.1:25520";
  let peer_address = "127.0.0.1:60124";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  let deliver = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_system_envelope("shutdown-flush")),
      })
    })
    .effects;
  bridge.process_effects(deliver).await.expect("deliver effects");

  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(25520), 43);
  bridge
    .handle_inbound_frame(InboundFrame::new("test-listener", peer_address, offer.encode(), CorrelationId::nil()))
    .await;

  let bridge_for_flush = bridge.clone();
  let flush_task = tokio::spawn(async move {
    bridge_for_flush.run_shutdown_flush().await;
  });
  tokio::time::sleep(Duration::from_millis(30)).await;

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      FlushAck::new(0).encode_frame(),
      CorrelationId::nil(),
    ))
    .await;

  tokio::time::timeout(Duration::from_millis(300), flush_task)
    .await
    .expect("shutdown flush should complete")
    .expect("flush task");

  let flush_sent =
    probe.sent_frames_for(authority).into_iter().filter(|frame| Flush::decode_frame(&frame.payload).is_ok()).count();
  assert!(flush_sent >= 1);
}

#[tokio::test(flavor = "current_thread")]
async fn remote_instrument_hooks_are_invoked_for_outbound_and_inbound_messages() {
  let instrument_state = Arc::new(Mutex::new(InstrumentCaptureState::default()));
  let instrument: Arc<dyn RemoteInstrument> = Arc::new(CaptureInstrument::new(instrument_state.clone()));
  let (bridge, probe, _system) = build_bridge_with_instruments(Duration::from_millis(200), vec![instrument]);
  let authority = "127.0.0.1:25520";
  let peer_address = "127.0.0.1:62222";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  let deliver = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_envelope("instrumented-message")),
      })
    })
    .effects;
  bridge.process_effects(deliver).await.expect("deliver effects");

  let sent_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .find(|frame| frame.payload.get(1) == Some(&0x10))
    .expect("remoting message frame");
  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      sent_frame.payload,
      sent_frame.correlation_id,
    ))
    .await;

  let state = instrument_state.lock().expect("instrument state");
  assert_eq!(state.outbound_metadata_calls, 1);
  assert_eq!(state.inbound_metadata_calls, 1);
  assert_eq!(state.sent_calls, 1);
  assert_eq!(state.received_calls, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn ack_frame_clears_pending_system_messages() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:25520";
  let peer_address = "127.0.0.1:61234";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  let deliver = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_system_envelope("to-clear")),
      })
    })
    .effects;
  bridge.process_effects(deliver).await.expect("deliver effects");

  let system_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .find(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .expect("system frame");
  let sequence_no =
    match AckedDelivery::decode_frame(&system_frame.payload, system_frame.correlation_id).expect("decode") {
      | AckedDelivery::SystemMessage(envelope) => envelope.sequence_no(),
      | other => panic!("unexpected frame payload: {other:?}"),
    };

  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(25520), 77);
  bridge
    .handle_inbound_frame(InboundFrame::new("test-listener", peer_address, offer.encode(), CorrelationId::nil()))
    .await;

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      AckedDelivery::ack(sequence_no).encode_frame(),
      system_frame.correlation_id,
    ))
    .await;
  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      Flush::new().encode_frame(),
      CorrelationId::nil(),
    ))
    .await;

  let flush_ack_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .rfind(|frame| frame.payload.get(1) == Some(&FLUSH_ACK_FRAME_KIND))
    .expect("flush-ack frame");
  let flush_ack = FlushAck::decode_frame(&flush_ack_frame.payload).expect("flush-ack decode");
  assert_eq!(flush_ack.expected_acks(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn nack_frame_resends_pending_system_message() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:25520";
  let peer_address = "127.0.0.1:61235";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25520), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  let deliver = bridge
    .coordinator
    .with_write(|m| {
      m.handle(EndpointAssociationCommand::EnqueueDeferred {
        authority: authority.to_string(),
        envelope:  Box::new(deferred_system_envelope("resent-on-nack")),
      })
    })
    .effects;
  bridge.process_effects(deliver).await.expect("deliver effects");

  let first_system_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .find(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .expect("first system frame");
  let sequence_no = match AckedDelivery::decode_frame(&first_system_frame.payload, first_system_frame.correlation_id)
    .expect("decode")
  {
    | AckedDelivery::SystemMessage(envelope) => envelope.sequence_no(),
    | other => panic!("unexpected frame payload: {other:?}"),
  };

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      AckedDelivery::nack(sequence_no.saturating_sub(1)).encode_frame(),
      first_system_frame.correlation_id,
    ))
    .await;

  let resent_sequences: Vec<u64> = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .map(|frame| {
      match AckedDelivery::decode_frame(&frame.payload, frame.correlation_id).expect("decode resent system") {
        | AckedDelivery::SystemMessage(envelope) => envelope.sequence_no(),
        | other => panic!("unexpected frame payload: {other:?}"),
      }
    })
    .collect();
  assert_eq!(resent_sequences, vec![sequence_no, sequence_no]);
}

#[tokio::test(flavor = "current_thread")]
async fn nack_frame_clears_acked_and_resends_unacknowledged_messages() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(200));
  let authority = "127.0.0.1:25521";
  let peer_address = "127.0.0.1:61236";
  let remote = RemoteNodeId::new("remote-system", "127.0.0.1", Some(25521), 1);

  let accept = bridge.coordinator.with_write(|m| {
    m.handle(EndpointAssociationCommand::HandshakeAccepted {
      authority:   authority.to_string(),
      remote_node: remote,
      now:         bridge.now_millis(),
    })
  });
  bridge.process_effects(accept.effects).await.expect("accept effects");

  for label in ["nack-clear-1", "nack-clear-2"] {
    let deliver = bridge
      .coordinator
      .with_write(|m| {
        m.handle(EndpointAssociationCommand::EnqueueDeferred {
          authority: authority.to_string(),
          envelope:  Box::new(deferred_system_envelope(label)),
        })
      })
      .effects;
    bridge.process_effects(deliver).await.expect("deliver effects");
  }

  let sent_sequences_before_nack: Vec<u64> = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .map(|frame| {
      match AckedDelivery::decode_frame(&frame.payload, frame.correlation_id).expect("decode initial system") {
        | AckedDelivery::SystemMessage(envelope) => envelope.sequence_no(),
        | other => panic!("unexpected frame payload: {other:?}"),
      }
    })
    .collect();
  assert_eq!(sent_sequences_before_nack, vec![1, 2]);

  let first_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .find(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .expect("first system frame");

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      peer_address,
      AckedDelivery::nack(1).encode_frame(),
      first_frame.correlation_id,
    ))
    .await;

  let sent_sequences_after_nack: Vec<u64> = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter(|frame| frame.payload.get(1) == Some(&SYSTEM_MESSAGE_FRAME_KIND))
    .map(|frame| {
      match AckedDelivery::decode_frame(&frame.payload, frame.correlation_id).expect("decode resent system") {
        | AckedDelivery::SystemMessage(envelope) => envelope.sequence_no(),
        | other => panic!("unexpected frame payload: {other:?}"),
      }
    })
    .collect();
  assert_eq!(sent_sequences_after_nack, vec![1, 2, 2]);
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_heartbeat_frame_emits_watcher_command_and_replies() {
  let (bridge, probe, system, control) = build_bridge_with_control(Duration::from_millis(200));
  let recorder = register_remote_watcher_daemon(&system, &control);
  let authority = "127.0.0.1:4906";
  associate(&bridge, authority, TransportEndpoint::new(authority.to_string()), bridge.now_millis()).await;
  let offer_uid = probe
    .sent_frames_for(authority)
    .into_iter()
    .filter(|frame| frame.correlation_id == CorrelationId::nil())
    .find_map(|frame| HandshakeFrame::decode(&frame.payload).ok())
    .map(|frame| frame.uid())
    .expect("offer uid");

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      authority,
      Heartbeat::new(authority).encode_frame(),
      CorrelationId::nil(),
    ))
    .await;

  let commands = recorder.snapshot();
  assert!(commands.iter().any(|command| {
    matches!(command, RemoteWatcherCommand::Heartbeat { heartbeat } if heartbeat.authority == authority)
  }));

  let rsp_frame = probe
    .sent_frames_for(authority)
    .into_iter()
    .rfind(|frame| frame.payload.get(1) == Some(&HEARTBEAT_RSP_FRAME_KIND))
    .expect("heartbeat-rsp frame");
  let rsp = HeartbeatRsp::decode_frame(&rsp_frame.payload, authority).expect("decode heartbeat-rsp");
  assert_eq!(rsp.authority(), authority);
  assert_eq!(rsp.uid(), offer_uid);
  assert_ne!(rsp.uid(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_heartbeat_rsp_frame_emits_watcher_command() {
  let (bridge, _probe, system, control) = build_bridge_with_control(Duration::from_millis(200));
  let recorder = register_remote_watcher_daemon(&system, &control);
  let authority = "127.0.0.1:4907";

  bridge
    .handle_inbound_frame(InboundFrame::new(
      "test-listener",
      authority,
      HeartbeatRsp::new(authority, 77).encode_frame(),
      CorrelationId::nil(),
    ))
    .await;

  let commands = recorder.snapshot();
  assert!(commands.iter().any(|command| {
    matches!(
      command,
      RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp }
      if heartbeat_rsp.authority == authority && heartbeat_rsp.uid == 77
    )
  }));
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_handler_rejects_frames_when_queue_is_full() {
  let (handle, probe, _system) = spawn_bridge(Duration::from_millis(500));
  probe.set_send_delay(Duration::from_millis(20));

  let total_frames = 96usize;
  for index in 0..total_frames {
    let port = 4800 + index as u16;
    let frame = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(port), index as u64);
    probe.emit_inbound_frame(InboundFrame::new(
      "test-listener",
      format!("127.0.0.1:{port}"),
      frame.encode(),
      CorrelationId::nil(),
    ));
  }

  tokio::time::sleep(Duration::from_millis(200)).await;
  let ack_count = probe.sent_handshake_kinds().iter().filter(|kind| matches!(kind, HandshakeKind::Ack)).count();
  assert!(ack_count > 0);
  assert!(ack_count < total_frames);

  let _ = handle.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn outbound_loop_emits_periodic_reap_unreachable() {
  let (handle, _probe, system, control) = spawn_bridge_with_control(Duration::from_millis(500));
  let recorder = register_remote_watcher_daemon(&system, &control);

  tokio::time::sleep(Duration::from_millis(260)).await;

  let commands = recorder.snapshot();
  assert!(commands.iter().any(|command| matches!(command, RemoteWatcherCommand::ReapUnreachable)));

  let _ = handle.shutdown().await;
}

#[tokio::test(flavor = "current_thread")]
async fn outbound_loop_emits_periodic_heartbeat_tick() {
  let (handle, _probe, system, control) = spawn_bridge_with_control(Duration::from_millis(500));
  let recorder = register_remote_watcher_daemon(&system, &control);

  tokio::time::sleep(Duration::from_millis(260)).await;

  let commands = recorder.snapshot();
  assert!(commands.iter().any(|command| matches!(command, RemoteWatcherCommand::HeartbeatTick)));

  let _ = handle.shutdown().await;
}
