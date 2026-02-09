use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  sync::Arc,
  vec::Vec,
};
use core::time::Duration;
use std::sync::Mutex;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric},
  error::ActorError,
  event::stream::CorrelationId,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::{ManualTestDriver, TickDriverConfig},
  serialization::{
    SerializationCallScope, SerializationExtensionGeneric, SerializationExtensionSharedGeneric, SerializationSetup,
    SerializationSetupBuilder, Serializer, SerializerId, StringSerializer,
  },
  system::{ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::{
  core::sync::{ArcShared, SharedAccess},
  std::runtime_toolbox::StdToolbox,
};

use super::{EndpointTransportBridge, EndpointTransportBridgeConfig};
use crate::core::{
  AssociationState, EndpointAssociationCommand, EndpointReaderGeneric, EndpointWriterGeneric,
  EndpointWriterSharedGeneric, EventPublisherGeneric, HandshakeFrame, HandshakeKind, QuarantineReason, RemoteNodeId,
  RemoteTransport, RemoteTransportShared, TransportBind, TransportChannel, TransportEndpoint, TransportError,
  TransportHandle, TransportInboundShared,
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
struct SentFrame {
  authority:      String,
  payload:        Vec<u8>,
  correlation_id: CorrelationId,
}

#[derive(Clone, Default)]
struct TestTransportProbe {
  sent_frames: Arc<Mutex<Vec<SentFrame>>>,
}

impl TestTransportProbe {
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
    let authority =
      self.channels.get(&channel.id()).cloned().ok_or(TransportError::ChannelUnavailable(channel.id()))?;
    self.probe.push_sent(authority, payload, correlation_id);
    Ok(())
  }

  fn close(&mut self, channel: &TransportChannel) {
    self.channels.remove(&channel.id());
  }

  fn install_backpressure_hook(&mut self, _hook: crate::core::TransportBackpressureHookShared) {}

  fn install_inbound_handler(&mut self, handler: TransportInboundShared<StdToolbox>) {
    self.inbound = Some(handler);
  }
}

fn build_system() -> ActorSystemGeneric<StdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("endpoint-bridge-tests");
  let config = ActorSystemConfigGeneric::<StdToolbox>::default()
    .with_tick_driver(TickDriverConfig::manual(ManualTestDriver::<StdToolbox>::new()));
  ActorSystemGeneric::new_with_config(&props, &config).expect("actor system")
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
  let system = build_system();
  let serialization = serialization_extension(&system);
  let writer = EndpointWriterSharedGeneric::new(EndpointWriterGeneric::new(system.downgrade(), serialization.clone()));
  let reader = ArcShared::new(EndpointReaderGeneric::new(system.downgrade(), serialization));
  let (transport, probe) = TestTransport::new();
  let config = EndpointTransportBridgeConfig {
    system: system.downgrade(),
    writer,
    reader,
    transport: RemoteTransportShared::new(Box::new(transport)),
    event_publisher: EventPublisherGeneric::new(system.downgrade()),
    canonical_host: "127.0.0.1".to_string(),
    canonical_port: 2552,
    system_name: "local-system".to_string(),
    handshake_timeout,
  };
  (EndpointTransportBridge::new(config), probe, system)
}

fn association_state(
  bridge: &EndpointTransportBridge<StdToolbox>,
  authority: &str,
) -> Option<crate::core::AssociationState> {
  bridge.coordinator.with_read(|m| m.state(authority))
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

  tokio::time::sleep(Duration::from_millis(40)).await;
  assert!(matches!(association_state(&bridge, authority), Some(AssociationState::Associating { .. })));
}

#[tokio::test(flavor = "current_thread")]
async fn receiving_offer_replies_ack_and_marks_connected() {
  let (bridge, probe, _system) = build_bridge(Duration::from_millis(500));
  let offer = HandshakeFrame::new(HandshakeKind::Offer, "remote-system", "127.0.0.1", Some(4201), 42);

  bridge.process_handshake_payload(offer.encode()).await.expect("offer processing");

  assert_eq!(probe.sent_handshake_kinds_for("127.0.0.1:4201"), vec![HandshakeKind::Ack]);
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
  bridge.process_handshake_payload(offer.encode()).await.expect("offer processing");

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
  bridge.process_handshake_payload(offer.encode()).await.expect("offer processing");

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
  bridge.process_handshake_payload(ack.encode()).await.expect("ack processing");
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
