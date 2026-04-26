use alloc::{string::String, vec::Vec};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_path::{ActorPath, ActorPathParser},
    messaging::AnyMessage,
  },
  event::{logging::ActorLogMarker, stream::CorrelationId},
};

use crate::domain::{
  address::{Address, RemoteNodeId},
  envelope::{InboundEnvelope, OutboundEnvelope, OutboundPriority},
  instrument::{
    FlightRecorderEvent, HandshakePhase, RemoteInstrument, RemoteLogMarker, RemotingFlightRecorder,
    RemotingFlightRecorderSnapshot,
  },
  transport::BackpressureSignal,
};

const REMOTE_ADDRESS: &str = "sys@host:2552";
const REMOTE_ADDRESS_UID: u64 = 42;

fn marker_property<'a>(marker: &'a ActorLogMarker, key: &str) -> Option<&'a str> {
  marker.properties().get(key).map(String::as_str)
}

fn sample_path() -> ActorPath {
  ActorPathParser::parse("fraktor.tcp://sys@host:2552/user/worker").expect("parse")
}

fn sample_remote_node() -> RemoteNodeId {
  RemoteNodeId::new("sys", "host", Some(2552), 1)
}

fn sample_address() -> Address {
  Address::new("sys", "host", 2552)
}

fn sample_outbound() -> OutboundEnvelope {
  OutboundEnvelope::new(
    sample_path(),
    None,
    AnyMessage::new(String::from("hi")),
    OutboundPriority::User,
    sample_remote_node(),
    CorrelationId::nil(),
  )
}

fn sample_inbound() -> InboundEnvelope {
  InboundEnvelope::new(
    sample_path(),
    sample_remote_node(),
    AnyMessage::new(String::from("hi")),
    None,
    CorrelationId::nil(),
    OutboundPriority::User,
  )
}

// ---------------------------------------------------------------------------
// RemotingFlightRecorder
// ---------------------------------------------------------------------------

#[test]
fn new_recorder_is_empty() {
  let r = RemotingFlightRecorder::new(10);
  assert_eq!(r.capacity(), 10);
  assert_eq!(r.len(), 0);
  assert!(r.is_empty());
  assert!(r.snapshot().is_empty());
}

#[test]
fn record_send_captures_all_fields() {
  let mut r = RemotingFlightRecorder::new(10);
  r.record_send("sys@host:2552", CorrelationId::new(1, 2), 0, 42, 100);
  let snap = r.snapshot();
  assert_eq!(snap.len(), 1);
  let Some(FlightRecorderEvent::Send { authority, correlation_id, priority, size, now_ms }) =
    snap.events().first().cloned()
  else {
    panic!("expected Send event");
  };
  assert_eq!(authority, "sys@host:2552");
  assert_eq!(correlation_id, CorrelationId::new(1, 2));
  assert_eq!(priority, 0);
  assert_eq!(size, 42);
  assert_eq!(now_ms, 100);
}

#[test]
fn record_receive_captures_all_fields() {
  let mut r = RemotingFlightRecorder::new(10);
  r.record_receive("sys@host:2552", CorrelationId::nil(), 5, 200);
  let snap = r.snapshot();
  assert!(matches!(snap.events().first(), Some(FlightRecorderEvent::Receive { .. })));
}

#[test]
fn record_handshake_captures_phase() {
  let mut r = RemotingFlightRecorder::new(10);
  r.record_handshake("sys@host:2552", HandshakePhase::Started, 0);
  r.record_handshake("sys@host:2552", HandshakePhase::Accepted, 100);
  let snap = r.snapshot();
  assert_eq!(snap.len(), 2);
  assert!(matches!(snap.events()[0], FlightRecorderEvent::Handshake { phase: HandshakePhase::Started, .. }));
  assert!(matches!(snap.events()[1], FlightRecorderEvent::Handshake { phase: HandshakePhase::Accepted, .. }));
}

#[test]
fn record_quarantine_captures_reason() {
  let mut r = RemotingFlightRecorder::new(10);
  r.record_quarantine("sys@host:2552", "handshake timed out", 500);
  let snap = r.snapshot();
  let Some(FlightRecorderEvent::Quarantine { reason, .. }) = snap.events().first().cloned() else {
    panic!("expected Quarantine event");
  };
  assert_eq!(reason, "handshake timed out");
}

#[test]
fn record_backpressure_is_captured() {
  let mut r = RemotingFlightRecorder::new(10);
  r.record_backpressure("sys@host:2552", BackpressureSignal::Apply, CorrelationId::new(1, 0), 10);
  r.record_backpressure("sys@host:2552", BackpressureSignal::Release, CorrelationId::new(1, 0), 50);
  let snap = r.snapshot();
  assert_eq!(snap.len(), 2);
  assert!(matches!(snap.events()[0], FlightRecorderEvent::Backpressure { signal: BackpressureSignal::Apply, .. }));
  assert!(matches!(snap.events()[1], FlightRecorderEvent::Backpressure { signal: BackpressureSignal::Release, .. }));
}

#[test]
fn ring_buffer_drops_oldest_events_when_capacity_reached() {
  let mut r = RemotingFlightRecorder::new(3);
  for i in 0..5 {
    r.record_send("sys@host:2552", CorrelationId::nil(), 0, 1, i);
  }
  let snap = r.snapshot();
  assert_eq!(snap.len(), 3);
  // Oldest two (now_ms 0, 1) should have been evicted; retained = 2, 3, 4.
  let times: Vec<u64> = snap
    .events()
    .iter()
    .filter_map(|e| match e {
      | FlightRecorderEvent::Send { now_ms, .. } => Some(*now_ms),
      | _ => None,
    })
    .collect();
  assert_eq!(times, [2, 3, 4]);
}

#[test]
fn zero_capacity_recorder_discards_everything() {
  let mut r = RemotingFlightRecorder::new(0);
  r.record_send("sys@host:2552", CorrelationId::nil(), 0, 1, 0);
  r.record_receive("sys@host:2552", CorrelationId::nil(), 1, 0);
  assert!(r.is_empty());
  assert!(r.snapshot().is_empty());
}

#[test]
fn snapshot_preserves_order() {
  let mut r = RemotingFlightRecorder::new(100);
  r.record_send("a", CorrelationId::nil(), 0, 1, 10);
  r.record_receive("a", CorrelationId::nil(), 1, 20);
  r.record_handshake("a", HandshakePhase::Accepted, 30);
  let snap = r.snapshot();
  assert_eq!(snap.len(), 3);
  assert!(matches!(snap.events()[0], FlightRecorderEvent::Send { .. }));
  assert!(matches!(snap.events()[1], FlightRecorderEvent::Receive { .. }));
  assert!(matches!(snap.events()[2], FlightRecorderEvent::Handshake { .. }));
}

#[test]
fn snapshot_is_immutable_after_production() {
  let mut r = RemotingFlightRecorder::new(10);
  r.record_send("a", CorrelationId::nil(), 0, 1, 10);
  let snap = r.snapshot();
  // Mutating the recorder after taking a snapshot does not affect the snapshot.
  r.record_send("b", CorrelationId::nil(), 0, 1, 20);
  assert_eq!(snap.len(), 1);
}

// ---------------------------------------------------------------------------
// RemoteInstrument trait
// ---------------------------------------------------------------------------

struct CountingInstrument {
  sends:    usize,
  receives: usize,
}

impl CountingInstrument {
  const fn new() -> Self {
    Self { sends: 0, receives: 0 }
  }
}

impl RemoteInstrument for CountingInstrument {
  fn on_send(&mut self, _envelope: &OutboundEnvelope) {
    self.sends += 1;
  }

  fn on_receive(&mut self, _envelope: &InboundEnvelope) {
    self.receives += 1;
  }
}

#[test]
fn remote_instrument_trait_can_be_implemented() {
  let mut inst = CountingInstrument::new();
  let out = sample_outbound();
  let inb = sample_inbound();
  inst.on_send(&out);
  inst.on_receive(&inb);
  inst.on_send(&out);
  assert_eq!(inst.sends, 2);
  assert_eq!(inst.receives, 1);
}

#[test]
fn snapshot_new_and_accessors() {
  let events = alloc::vec![FlightRecorderEvent::Send {
    authority:      "x".into(),
    correlation_id: CorrelationId::nil(),
    priority:       0,
    size:           1,
    now_ms:         0,
  }];
  let snap = RemotingFlightRecorderSnapshot::new(events);
  assert_eq!(snap.len(), 1);
  assert!(!snap.is_empty());
}

// ---------------------------------------------------------------------------
// RemoteLogMarker
// ---------------------------------------------------------------------------

#[test]
fn failure_detector_growing_marker_uses_pekko_name_and_remote_address() {
  let address = sample_address();
  let marker = RemoteLogMarker::failure_detector_growing(&address);

  assert_eq!(marker.name(), "pekkoFailureDetectorGrowing");
  assert_eq!(marker_property(&marker, "pekkoRemoteAddress"), Some(REMOTE_ADDRESS));
  assert_eq!(marker.properties().len(), 1);
}

#[test]
fn quarantine_marker_uses_pekko_name_remote_address_and_uid() {
  let address = sample_address();
  let marker = RemoteLogMarker::quarantine(&address, Some(REMOTE_ADDRESS_UID));

  assert_eq!(marker.name(), "pekkoQuarantine");
  assert_eq!(marker_property(&marker, "pekkoRemoteAddress"), Some(REMOTE_ADDRESS));
  assert_eq!(marker_property(&marker, "pekkoRemoteAddressUid"), Some("42"));
}

#[test]
fn quarantine_marker_uses_empty_uid_property_when_uid_is_absent() {
  let address = sample_address();
  let marker = RemoteLogMarker::quarantine(&address, None);

  assert_eq!(marker.name(), "pekkoQuarantine");
  assert_eq!(marker_property(&marker, "pekkoRemoteAddress"), Some(REMOTE_ADDRESS));
  assert_eq!(marker_property(&marker, "pekkoRemoteAddressUid"), Some(""));
}

#[test]
fn connect_marker_uses_pekko_name_remote_address_and_uid() {
  let address = sample_address();
  let marker = RemoteLogMarker::connect(&address, Some(REMOTE_ADDRESS_UID));

  assert_eq!(marker.name(), "pekkoConnect");
  assert_eq!(marker_property(&marker, "pekkoRemoteAddress"), Some(REMOTE_ADDRESS));
  assert_eq!(marker_property(&marker, "pekkoRemoteAddressUid"), Some("42"));
}

#[test]
fn disconnected_marker_uses_pekko_name_remote_address_and_uid() {
  let address = sample_address();
  let marker = RemoteLogMarker::disconnected(&address, Some(REMOTE_ADDRESS_UID));

  assert_eq!(marker.name(), "pekkoDisconnected");
  assert_eq!(marker_property(&marker, "pekkoRemoteAddress"), Some(REMOTE_ADDRESS));
  assert_eq!(marker_property(&marker, "pekkoRemoteAddressUid"), Some("42"));
}
